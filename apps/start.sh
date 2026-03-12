#!/bin/bash
# Start diraigent with Claude Max subscription token from macOS Keychain.
# Usage: ./start.sh

set -euo pipefail
cd "$(dirname "$0")"

# Load .env if it exists
if [ -f .env ]; then
  set -a; source .env; set +a
fi

# Extract OAuth token from macOS Keychain
CREDS=$(security find-generic-password -s "Claude Code-credentials" -w 2>/dev/null || true)
if [ -z "$CREDS" ]; then
  echo "No Claude Code credentials found in Keychain."
  echo "Run 'claude login' first."
  exit 1
fi

TOKEN=$(echo "$CREDS" | python3 -c "import sys,json; print(json.load(sys.stdin)['claudeAiOauth']['accessToken'])")
export CLAUDE_CODE_OAUTH_TOKEN="$TOKEN"
echo "Claude Max token loaded"

DEV_USER="${DEV_USER_ID:-00000000-0000-0000-0000-000000000001}"
API="http://localhost:8082/v1"

wait_for_api() {
  echo "Waiting for API..."
  for _ in $(seq 1 30); do
    if curl -sf http://localhost:8082/health/live >/dev/null 2>&1; then return 0; fi
    sleep 1
  done
  echo "API did not start in time"; exit 1
}

# Start infra + API first
docker compose up -d postgres
sleep 2
docker compose up -d api
wait_for_api

# Verify existing AGENT_ID is still valid, clear if stale
if [ -n "${AGENT_ID:-}" ]; then
  if ! curl -sf "$API/agents/$AGENT_ID" -H "X-Dev-User-Id: $DEV_USER" >/dev/null 2>&1; then
    echo "AGENT_ID $AGENT_ID is stale, re-registering..."
    unset AGENT_ID
  fi
fi

# Auto-register agent if AGENT_ID is not set
if [ -z "${AGENT_ID:-}" ]; then
  # Try to find existing agent by name, or create new
  AGENT_ID=$(curl -sf "$API/agents" -H "X-Dev-User-Id: $DEV_USER" \
    | python3 -c "import sys,json; agents=[a for a in json.load(sys.stdin) if a['name']=='orchestra-docker']; print(agents[0]['id'] if agents else '')" 2>/dev/null || true)

  if [ -z "$AGENT_ID" ]; then
    AGENT_ID=$(curl -sf -X POST "$API/agents" \
      -H 'Content-Type: application/json' \
      -H "X-Dev-User-Id: $DEV_USER" \
      -d '{"name": "orchestra-docker", "kind": "claude"}' | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
    echo "Registered agent: $AGENT_ID"
  else
    echo "Found existing agent: $AGENT_ID"
  fi

  # Create a role and membership so the agent can work on projects
  ROLE_ID=$(curl -sf -X POST "$API/roles" \
    -H 'Content-Type: application/json' \
    -H "X-Dev-User-Id: $DEV_USER" \
    -d '{"name": "orchestra", "authorities": ["execute","create","delegate","review","decide"]}' \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
  echo "Created role: $ROLE_ID"

  curl -sf -X POST "$API/members" \
    -H 'Content-Type: application/json' \
    -H "X-Dev-User-Id: $DEV_USER" \
    -d "{\"agent_id\": \"$AGENT_ID\", \"role_id\": \"$ROLE_ID\"}" >/dev/null
  echo "Agent membership created"

  # Persist to .env
  if grep -q '^AGENT_ID=' .env 2>/dev/null; then
    sed -i '' "s/^AGENT_ID=.*/AGENT_ID=$AGENT_ID/" .env
  else
    echo "AGENT_ID=$AGENT_ID" >> .env
  fi
  export AGENT_ID
fi

# Start all containers
docker compose up -d

echo "All services started. Use 'docker compose logs -f' to follow logs."
