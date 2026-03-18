#!/usr/bin/env bash
set -euo pipefail

# Usage: ./seed-test-data.sh <bearer-token>
# Grab a token from browser DevTools: Network tab → any API request → Authorization header
T="${1:?Usage: ./seed-test-data.sh <bearer-token>}"
A="https://api.diraigent.com/v1"
H="Authorization: Bearer $T"
CT="Content-Type: application/json"

post() {
  local resp
  resp=$(curl -s -X POST "$A$1" -H "$H" -H "$CT" -d "$2")
  local id
  id=$(echo "$resp" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['id'])" 2>/dev/null) || {
    echo "FAIL $1: $resp" >&2
    return 1
  }
  echo "$id"
}

echo "--- Creating projects ---"
P1=$(post "/" '{"name":"Webshop Backend","slug":"webshop-backend","description":"E-commerce API built with Rust/Axum. Handles products, orders, payments, and inventory.","repo_url":"https://github.com/acme/webshop-backend","default_branch":"main","git_mode":"standalone"}')
echo "P1=$P1"
P2=$(post "/" '{"name":"Mobile App","slug":"mobile-app","description":"React Native mobile app for the webshop. iOS and Android.","repo_url":"https://github.com/acme/mobile-app","default_branch":"main","git_mode":"standalone"}')
echo "P2=$P2"
P3=$(post "/" '{"name":"Infrastructure","slug":"infrastructure","description":"Terraform modules and Kubernetes manifests for cloud deployment.","repo_url":"https://github.com/acme/infrastructure","default_branch":"main","git_mode":"standalone"}')
echo "P3=$P3"

echo "--- Creating roles ---"
R1=$(post "/roles" '{"name":"Lead Developer","description":"Senior engineer who reviews code and makes architectural decisions","authorities":["execute","delegate","review","create","decide"]}')
R2=$(post "/roles" '{"name":"Developer","description":"Implements features and fixes bugs","authorities":["execute","create"]}')
R3=$(post "/roles" '{"name":"Reviewer","description":"Reviews pull requests and approves changes","authorities":["review","decide"]}')
R4=$(post "/roles" '{"name":"DevOps","description":"Manages infrastructure, CI/CD pipelines, and deployments","authorities":["execute","create","manage"]}')
echo "Roles: $R1 $R2 $R3 $R4"

echo "--- Creating agents ---"
A1=$(post "/agents" '{"name":"claude-backend","capabilities":["rust","sql","api-design","testing"]}')
A2=$(post "/agents" '{"name":"claude-frontend","capabilities":["react-native","typescript","ui","testing"]}')
A3=$(post "/agents" '{"name":"claude-infra","capabilities":["terraform","kubernetes","docker","ci-cd"]}')
echo "Agents: $A1 $A2 $A3"

echo "--- Creating tasks for Webshop Backend ---"
post "/$P1/tasks" '{"title":"Add product search with full-text indexing","kind":"feature","context":{"spec":"Implement PostgreSQL tsvector-based full-text search on product name and description."}}'
post "/$P1/tasks" '{"title":"Fix race condition in order placement","kind":"bug","urgent":true,"context":{"spec":"Concurrent order placement can oversell inventory. Add row-level locking."}}'
post "/$P1/tasks" '{"title":"Implement webhook delivery with retry logic","kind":"feature","context":{"spec":"Send HTTP webhooks for order events with exponential backoff retry."}}'
post "/$P1/tasks" '{"title":"Add rate limiting per API key","kind":"feature","context":{"spec":"Sliding window rate limiting using Redis. 100 req/min default."}}'
post "/$P1/tasks" '{"title":"Migrate payment provider from Stripe v2 to v3","kind":"chore","context":{"spec":"Update Stripe SDK and migrate to PaymentIntents API."}}'
post "/$P1/tasks" '{"title":"Add OpenAPI spec generation","kind":"feature","context":{"spec":"Use utoipa to auto-generate OpenAPI 3.1 spec."}}'
post "/$P1/tasks" '{"title":"Optimize product listing query","kind":"refactor","context":{"spec":"Product listing takes >500ms. Add composite indexes and optimize query plan."}}'
post "/$P1/tasks" '{"title":"Set up integration test suite","kind":"chore","context":{"spec":"Create test harness with testcontainers for PostgreSQL."}}'

echo "--- Creating tasks for Mobile App ---"
post "/$P2/tasks" '{"title":"Implement product catalog with infinite scroll","kind":"feature","context":{"spec":"Display products in grid with lazy loading and image caching."}}'
post "/$P2/tasks" '{"title":"Add biometric authentication","kind":"feature","context":{"spec":"Support Face ID and fingerprint login with PIN fallback."}}'
post "/$P2/tasks" '{"title":"Fix cart total calculation rounding errors","kind":"bug","urgent":true,"context":{"spec":"Floating point causes 1-cent discrepancies. Use integer cents."}}'
post "/$P2/tasks" '{"title":"Add push notification support","kind":"feature","context":{"spec":"Integrate Firebase Cloud Messaging for order updates."}}'
post "/$P2/tasks" '{"title":"Implement offline mode for order history","kind":"feature","context":{"spec":"Cache order history with WatermelonDB. Sync when online."}}'

echo "--- Creating tasks for Infrastructure ---"
post "/$P3/tasks" '{"title":"Set up Terraform state backend with S3+DynamoDB","kind":"chore","context":{"spec":"Configure remote state with encryption, locking, versioning."}}'
post "/$P3/tasks" '{"title":"Create Kubernetes namespace isolation","kind":"feature","context":{"spec":"Separate staging/prod with network policies and RBAC."}}'
post "/$P3/tasks" '{"title":"Add horizontal pod autoscaler for API","kind":"feature","context":{"spec":"Configure HPA based on CPU and request latency p99."}}'
post "/$P3/tasks" '{"title":"Set up GitHub Actions CI pipeline","kind":"chore","context":{"spec":"Build, test, lint on PR. Deploy staging on merge. Manual prod promote."}}'
post "/$P3/tasks" '{"title":"Configure database backup automation","kind":"chore","urgent":true,"context":{"spec":"Daily PostgreSQL backups to S3 with 30-day retention."}}'

echo "--- Creating knowledge ---"
post "/$P1/knowledge" '{"title":"API Authentication Design","category":"architecture","content":"All API endpoints use JWT bearer tokens issued by Authentik. Tokens have a 10-minute lifetime with refresh token rotation. API keys are supported for service-to-service communication with configurable rate limits per key.","tags":["auth","security","jwt"]}'
post "/$P1/knowledge" '{"title":"Database Migration Strategy","category":"convention","content":"Migrations use sqlx-migrate and run automatically on startup. Always write reversible migrations. Never alter columns in-place on large tables - use the expand-contract pattern.","tags":["database","migrations"]}'
post "/$P1/knowledge" '{"title":"Order State Machine","category":"pattern","content":"Orders follow: draft -> placed -> paid -> processing -> shipped -> delivered. Cancelled from placed or paid. Refund from paid, processing, shipped, or delivered. Each transition fires a webhook.","tags":["orders","state-machine"]}'
post "/$P2/knowledge" '{"title":"React Native Build Process","category":"setup","content":"iOS builds use Fastlane with match for code signing. Android uses Gradle. Both build in GitHub Actions. TestFlight for iOS beta, Firebase App Distribution for Android.","tags":["build","ios","android"]}'
post "/$P3/knowledge" '{"title":"Infrastructure Cost Guardrails","category":"convention","content":"All Terraform changes must include cost estimates via Infracost. Monthly budget alert at 80%. No instances larger than m5.2xlarge without VP approval. Spot instances for non-prod.","tags":["cost","policy","terraform"]}'

echo "--- Creating playbooks ---"
post "/playbooks" '{"title":"Standard Feature Pipeline","trigger_description":"New feature request","steps":[{"name":"dream","label":"Design"},{"name":"implement","label":"Implementation"},{"name":"review","label":"Code Review"},{"name":"test","label":"Testing"},{"name":"deploy","label":"Deploy"}],"tags":["feature","standard"]}'
post "/playbooks" '{"title":"Hotfix Pipeline","trigger_description":"Critical production bug","steps":[{"name":"implement","label":"Fix"},{"name":"review","label":"Quick Review"},{"name":"deploy","label":"Deploy"}],"tags":["hotfix","urgent"]}'
post "/playbooks" '{"title":"Infrastructure Change","trigger_description":"Infrastructure modification","steps":[{"name":"dream","label":"Plan"},{"name":"review","label":"Review"},{"name":"implement","label":"Apply"},{"name":"test","label":"Verify"}],"tags":["infrastructure"]}'

echo ""
echo "=== DONE ==="
echo "Created: 3 projects, 4 roles, 3 agents, 18 tasks, 5 knowledge items, 3 playbooks"
