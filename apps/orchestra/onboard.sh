#!/bin/bash
# Onboard an orchestra agent to a Diraigent API.
#
# Usage:
#   DIRAIGENT_API_TOKEN=<your-jwt> ./onboard.sh               # register new agent
#   DIRAIGENT_API_TOKEN=<your-jwt> ./onboard.sh --name my-agent
#   ./onboard.sh --key dak_... --agent-id <uuid>              # skip registration (key from web UI)
#
# Option 1: Pass a user JWT to register a new agent via the API.
# Option 2: Pass --key and --agent-id if you already created the agent
#            in the web dashboard (web.diraigent.com/agents).

set -euo pipefail
umask 077
cd "$(dirname "$0")"

# ── Defaults ──────────────────────────────────────────────────────
API="${DIRAIGENT_API_URL:-https://api.diraigent.com/v1}"
AGENT_NAME=""
EXISTING_KEY=""
EXISTING_AGENT_ID=""

# ── Parse flags ───────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --api)       API="$2"; shift 2 ;;
    --name)      AGENT_NAME="$2"; shift 2 ;;
    --key)       EXISTING_KEY="$2"; shift 2 ;;
    --agent-id)  EXISTING_AGENT_ID="$2"; shift 2 ;;
    *)           echo "Unknown flag: $1"; exit 1 ;;
  esac
done

# Determine auth header — either existing key or user JWT
if [ -n "$EXISTING_KEY" ]; then
  AUTH="Authorization: Bearer $EXISTING_KEY"
  TOKEN="$EXISTING_KEY"
else
  TOKEN="${DIRAIGENT_API_TOKEN:?Set DIRAIGENT_API_TOKEN (your user JWT) or use --key dak_...}"
  AUTH="Authorization: Bearer $TOKEN"
fi

# ── Health check ──────────────────────────────────────────────────
echo "API: $API"
HEALTH_URL="${API%/v1}/health/live"
if ! curl -sf "$HEALTH_URL" >/dev/null 2>&1; then
  echo "API not reachable at $HEALTH_URL"
  exit 1
fi
echo "API reachable"

# ── Register or reuse agent ────────────────────────────────────────
if [ -n "$EXISTING_KEY" ] && [ -n "$EXISTING_AGENT_ID" ]; then
  # Skip registration — agent was created in the web UI
  AGENT_ID="$EXISTING_AGENT_ID"
  API_KEY="$EXISTING_KEY"
  echo "Using existing agent: $AGENT_ID"
  echo "Agent credential received; value not shown."
else
  # Interactive registration
  if [ -z "$AGENT_NAME" ]; then
    read -rp "Agent name [orchestra-$(hostname -s)]: " AGENT_NAME
    AGENT_NAME="${AGENT_NAME:-orchestra-$(hostname -s)}"
  fi

  echo ""
  echo "Registering agent: $AGENT_NAME"
  RESULT=$(curl -sf -X POST "$API/agents" \
    -H 'Content-Type: application/json' -H "$AUTH" \
    -d "{
      \"name\": \"$AGENT_NAME\",
      \"capabilities\": [\"rust\",\"typescript\",\"angular\",\"sql\",\"docker\",\"code-review\"],
      \"metadata\": {\"provider\": \"opencode\", \"runtime\": \"orchestra\"}
    }")

  AGENT_ID=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
  API_KEY=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['api_key'])")
  echo "Agent ID: $AGENT_ID"
  echo "Agent credential received; value not shown."
fi

# ── Pick project (optional) ───────────────────────────────────────
PROJECT_ID=""
PROJECTS=$(curl -sf "$API/projects" -H "$AUTH" 2>/dev/null || echo "[]")
PROJECT_COUNT=$(echo "$PROJECTS" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")

if [ "$PROJECT_COUNT" -gt 0 ]; then
  echo ""
  echo "Projects:"
  echo "$PROJECTS" | python3 -c "
import sys, json
for i, p in enumerate(json.load(sys.stdin)):
    print(f\"  {i+1}) {p.get('name') or p.get('slug','?')}  ({p['id'][:8]}...)\")"

  echo ""
  read -rp "Pick project (Enter to skip): " PICK
  if [ -n "$PICK" ]; then
    PROJECT_ID=$(echo "$PROJECTS" | python3 -c "import sys,json; print(json.load(sys.stdin)[int('$PICK')-1]['id'])")
    echo "-> $PROJECT_ID"
  else
    echo "Skipped — set PROJECT_ID in .env later when you have a project."
  fi
else
  echo ""
  echo "No projects found. Set PROJECT_ID in .env later when you create one."
fi

# ── Pick role (optional, only if project was picked) ──────────────
if [ -n "$PROJECT_ID" ]; then
  ROLES=$(curl -sf "$API/roles" -H "$AUTH" 2>/dev/null || echo "[]")
  ROLE_COUNT=$(echo "$ROLES" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")

  if [ "$ROLE_COUNT" -gt 0 ]; then
    echo ""
    echo "Roles:"
    echo "$ROLES" | python3 -c "
import sys, json
for i, r in enumerate(json.load(sys.stdin)):
    auths = ', '.join(r.get('authorities', []))
    print(f\"  {i+1}) {r['name']}  [{auths}]\")"

    echo ""
    read -rp "Pick role [1]: " RPICK
    RPICK="${RPICK:-1}"
    ROLE_ID=$(echo "$ROLES" | python3 -c "import sys,json; print(json.load(sys.stdin)[int('$RPICK')-1]['id'])")

    curl -sf -X POST "$API/members" \
      -H 'Content-Type: application/json' -H "$AUTH" \
      -d "{\"agent_id\": \"$AGENT_ID\", \"role_id\": \"$ROLE_ID\"}" >/dev/null
    echo "Membership created"
  fi
fi

# ── Write .env ────────────────────────────────────────────────────
export ONBOARD_API_URL="$API" ONBOARD_AGENT_ID="$AGENT_ID" ONBOARD_API_KEY="$API_KEY"
python3 - <<'PYENV'
import os, re, shlex, tempfile
from pathlib import Path
path = Path('.env')
if path.is_symlink():
    raise SystemExit('Use a regular .env file to save credentials.')
values = {'DIRAIGENT_API_URL': os.environ['ONBOARD_API_URL'],
          'AGENT_ID': os.environ['ONBOARD_AGENT_ID'],
          'DIRAIGENT_API_TOKEN': os.environ['ONBOARD_API_KEY']}
lines = []
for line in path.read_text().splitlines() if path.exists() else []:
    match = re.match(r'\s*(?:export\s+)?(DIRAIGENT_API_URL|AGENT_ID|DIRAIGENT_API_TOKEN)\s*=', line)
    if not match:
        lines.append(line)
lines.extend(k + '=' + shlex.quote(v) for k, v in values.items())
fd, name = tempfile.mkstemp(prefix='.env-', dir='.')
try:
    with os.fdopen(fd, 'w') as f:
        f.write('\n'.join(lines) + '\n'); f.flush(); os.fsync(f.fileno())
    os.replace(name, path)
finally:
    if os.path.exists(name): os.unlink(name)
PYENV

echo ""
echo "Credentials saved privately in .env; existing provider settings preserved."
echo ""
echo "Start: cargo run -p diraigent-orchestra --bin orchestra"
