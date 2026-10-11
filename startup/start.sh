#!/usr/bin/env bash
# Local Docker Compose onboarding. OpenCode is the default task provider.
set -euo pipefail
umask 077
cd "$(dirname "$0")"

MODE=start
BUILD=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --build) BUILD=true ;;
    --login|--models)
      if [ "$MODE" != start ]; then echo 'Choose --login or --models.' >&2; exit 1; fi
      MODE=${1#--} ;;
    --help|-h)
      echo 'Usage: ./start.sh [--build] [--login|--models]'
      echo 'Copy .env.example to .env and choose OPENCODE_MODEL=provider/model.'
      echo '--build builds current images from this repository checkout.'
      echo '--login authenticates OpenCode in its persistent container volume.'
      echo '--models lists model IDs for OPENCODE_MODEL.'
      exit 0 ;;
    *) echo 'Unknown option. Use ./start.sh --help.' >&2; exit 1 ;;
  esac
  shift
done
for dependency in docker curl python3; do
  command -v "$dependency" >/dev/null || { echo "Missing dependency: $dependency" >&2; exit 1; }
done
docker compose version >/dev/null
if [ ! -f .env ]; then echo 'Copy .env.example to .env first.' >&2; exit 1; fi
if [ -L .env ]; then echo 'Use a regular .env file to persist credentials.' >&2; exit 1; fi
chmod 600 .env
set -a
source .env
set +a

if [ "$BUILD" = true ]; then
  if [ ! -f ../apps/orchestra/Containerfile ]; then
    echo 'Run --build from a full Diraigent repository checkout (startup/start.sh).' >&2
    exit 1
  fi
  if command -v git >/dev/null; then
    export DIRAIGENT_VERSION="${DIRAIGENT_VERSION:-$(git -C .. rev-parse --short HEAD 2>/dev/null || printf local)}"
  fi
  docker compose build api web orchestra
fi

# Named volumes survive container replacement. Initialize their mount roots for
# the image's unprivileged worker before running OpenCode or Orchestra.
docker compose run --rm --no-deps orchestra-init
if [ "$MODE" = login ]; then
  docker compose run --rm --no-deps --entrypoint opencode orchestra auth login --standalone
  echo 'OpenCode login saved. Set OPENCODE_MODEL in .env, then run ./start.sh.'
  exit 0
fi
if [ "$MODE" = models ]; then
  docker compose run --rm --no-deps --entrypoint opencode orchestra models --standalone
  exit 0
fi
case "${OPENCODE_MODEL:-}" in
  */?*) ;;
  *) echo 'Set OPENCODE_MODEL=provider/model in .env before starting workers.' >&2; exit 1 ;;
esac

START_TMP=$(mktemp -d "${TMPDIR:-/tmp}/diraigent-start.XXXXXX")
trap 'rm -rf "$START_TMP"' EXIT
# Never print the credential listing or include keys in command-line arguments.
if ! docker compose run --rm --no-deps --entrypoint opencode orchestra auth list --standalone --format json > "$START_TMP/provider-auth.json" 2> "$START_TMP/provider-auth.err"; then
  echo 'Could not inspect OpenCode authentication. Run ./start.sh --login.' >&2
  exit 1
fi
if ! python3 - "$START_TMP/provider-auth.json" <<'PY'
import json, os, sys
provider = os.environ['OPENCODE_MODEL'].split('/', 1)[0]
accounts = json.load(open(sys.argv[1]))
sys.exit(0 if any(a.get('id') == provider and a.get('connections') for a in accounts) else 1)
PY
then
  echo 'The selected model provider is not connected. Set its API key in .env or run ./start.sh --login.' >&2
  exit 1
fi

API="http://localhost:${API_PORT:-3000}/v1"
# Registration uses a user credential; subsequent starts use the stored agent key.
export STARTUP_AUTH_TOKEN="${DIRAIGENT_API_TOKEN:-${STARTUP_API_TOKEN:-}}"
python3 - "$START_TMP/headers" <<'PY'
import os, sys
headers = ['Content-Type: application/json']
token = os.environ.get('STARTUP_AUTH_TOKEN', '')
if token:
    headers.append('Authorization: Bearer ' + token)
elif os.environ.get('DEV_USER_ID'):
    headers.append('X-Dev-User-Id: ' + os.environ['DEV_USER_ID'])
else:
    sys.exit('Set STARTUP_API_TOKEN for initial authenticated registration, or AGENT_ID and DIRAIGENT_API_TOKEN from the web app.')
if any('\n' in h or '\r' in h for h in headers):
    sys.exit('Invalid authentication configuration.')
open(sys.argv[1], 'w').write('\n'.join(headers) + '\n')
PY

docker compose up -d postgres api
printf 'Waiting for API readiness'
ready=false
for attempt in $(seq 1 60); do
  if curl --silent --fail --max-time 2 "${API%/v1}/health/ready" >/dev/null 2>&1; then ready=true; break; fi
  printf '.'
  sleep 1
done
printf '\n'
if [ "$ready" != true ]; then echo 'API did not become ready. Inspect docker compose logs api.' >&2; exit 1; fi

request() {
  local method=$1 path=$2 status
  shift 2
  if ! status=$(curl --silent --show-error --max-time 30 --request "$method" \
    --header "@$START_TMP/headers" --output "$START_TMP/response.json" \
    --write-out '%{http_code}' "$@" "$API$path"); then
    echo 'API request failed; credentials and response contents were not printed.' >&2
    return 1
  fi
  case "$status" in
    2??) return 0 ;;
    *) echo "API request failed (HTTP $status). Check authentication and workspace access." >&2; return 1 ;;
  esac
}

if [ -n "${AGENT_ID:-}" ]; then
  if [ -z "${DIRAIGENT_API_TOKEN:-}" ]; then
    echo 'Existing AGENT_ID needs its DIRAIGENT_API_TOKEN. Copy both from the web app, or remove AGENT_ID to register a new worker.' >&2
    exit 1
  fi
  # Never silently replace an inaccessible or deleted identity: it may own data.
  request GET "/agents/$AGENT_ID"
  python3 - "$START_TMP/response.json" <<'PY'
import json, os, sys
assert json.load(open(sys.argv[1]))['id'] == os.environ['AGENT_ID'], 'Unexpected agent identity'
PY
  echo 'Existing agent verified.'
else
  if [ -n "${DIRAIGENT_API_TOKEN:-}" ]; then
    echo 'DIRAIGENT_API_TOKEN needs its AGENT_ID. Use STARTUP_API_TOKEN for initial registration.' >&2
    exit 1
  fi
  # A fresh user must own a workspace before registering agents. Do not rely
  # on development-mode auto-enrollment into the seeded shared workspace.
  request GET /tenants
  if python3 - "$START_TMP/response.json" <<'PY'
import json, sys
sys.exit(0 if json.load(open(sys.argv[1])) == [] else 1)
PY
  then
    printf '%s' '{"name":"My Workspace"}' > "$START_TMP/workspace.json"
    request POST /tenants --data-binary "@$START_TMP/workspace.json"
    echo 'Initial workspace created with owner access.'
  fi
  python3 - "$START_TMP/register.json" <<'PY'
import json, os, sys
with open(sys.argv[1], 'w') as f:
    json.dump({'name': os.environ.get('AGENT_NAME') or 'orchestra-docker',
               'metadata': {'runtime': 'orchestra', 'provider': 'opencode'}}, f)
PY
  request POST /agents --data-binary "@$START_TMP/register.json"
  # The API assigns the workspace role on registration. Persist the one-time
  # agent key immediately, preserving all unrelated provider/local settings.
  python3 - "$START_TMP/response.json" <<'PY'
import json, os, re, shlex, sys, tempfile, uuid
from pathlib import Path
response = json.load(open(sys.argv[1]))
agent = str(uuid.UUID(response['id']))
key = response['api_key']
if not isinstance(key, str) or not re.fullmatch(r'dak_[A-Za-z0-9_-]+', key):
    sys.exit('Registration did not return a valid agent key.')
path = Path('.env')
if path.is_symlink():
    sys.exit('Use a regular .env file to persist the agent credentials.')
values = {'AGENT_ID': agent, 'DIRAIGENT_API_TOKEN': key}
lines = []
for line in path.read_text().splitlines():
    match = re.match(r'\s*(?:export\s+)?(AGENT_ID|DIRAIGENT_API_TOKEN|STARTUP_API_TOKEN)\s*=', line)
    if not match:
        lines.append(line)
lines.extend(k + '=' + shlex.quote(v) for k, v in values.items())
fd, temp = tempfile.mkstemp(prefix='.env-', dir='.')
try:
    with os.fdopen(fd, 'w') as f:
        f.write('\n'.join(lines) + '\n'); f.flush(); os.fsync(f.fileno())
    os.replace(temp, path)
finally:
    if os.path.exists(temp): os.unlink(temp)
PY
  set -a
  source .env
  set +a
  echo 'Agent registered; private credentials saved in .env.'
fi

docker compose up -d web orchestra
echo "Diraigent is ready at http://localhost:${WEB_PORT:-4200}."
echo 'Create a project with its repository URL, then make a task ready.'
