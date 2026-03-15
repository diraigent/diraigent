#!/usr/bin/env bash
# post-commit-index.sh — Git post-commit hook for re-indexing the codebase.
#
# Runs the analyzer pipeline (scan → api-surface → sync) when source files
# change.  Skips entirely when no .rs/.ts/.tsx/.sql files were modified.
#
# Installation:
#   ln -sf ../../scripts/post-commit-index.sh .git/hooks/post-commit
#
# Environment variables:
#   DIRAIGENT_API_URL   — API base URL           (required)
#   DIRAIGENT_API_TOKEN — Agent API key or JWT    (required)
#   PROJECT_ID          — Diraigent project UUID  (required)
#   AGENT_ID            — Agent UUID              (optional)
#   ANALYZER_BIN        — Path to diraigent-analyzer binary (auto-detected)
#
# The script is intentionally best-effort: failures are logged but never block
# the commit.

set -euo pipefail

# ── Configuration ───────────────────────────────────────────
REPO_ROOT="$(git rev-parse --show-toplevel)"
STATE_FILE="${REPO_ROOT}/.analyzer-last-indexed-commit"

# Auto-detect the analyzer binary: next to this script's target, in PATH, or
# next to the orchestra binary.
if [ -z "${ANALYZER_BIN:-}" ]; then
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    if [ -x "${SCRIPT_DIR}/diraigent-analyzer" ]; then
        ANALYZER_BIN="${SCRIPT_DIR}/diraigent-analyzer"
    elif command -v diraigent-analyzer &>/dev/null; then
        ANALYZER_BIN="diraigent-analyzer"
    else
        echo "[index] diraigent-analyzer not found — skipping" >&2
        exit 0
    fi
fi

# Bail early if required env vars are missing (hook should not block commits).
if [ -z "${DIRAIGENT_API_URL:-}" ] || [ -z "${DIRAIGENT_API_TOKEN:-}" ] || [ -z "${PROJECT_ID:-}" ]; then
    exit 0
fi

# ── Change detection ────────────────────────────────────────
HEAD_COMMIT="$(git rev-parse HEAD)"

if [ -f "$STATE_FILE" ]; then
    LAST_COMMIT="$(cat "$STATE_FILE")"
    if [ "$LAST_COMMIT" = "$HEAD_COMMIT" ]; then
        exit 0  # Already indexed this commit
    fi

    # Check if any source files changed
    CHANGED="$(git diff --name-only "$LAST_COMMIT" "$HEAD_COMMIT" -- '*.rs' '*.ts' '*.tsx' '*.sql' 2>/dev/null || echo "FORCE")"
    if [ -z "$CHANGED" ]; then
        # No source changes — just update the stored commit hash
        echo "$HEAD_COMMIT" > "$STATE_FILE"
        exit 0
    fi
fi

echo "[index] re-indexing codebase (commit ${HEAD_COMMIT:0:12})..."

# ── Run pipeline ────────────────────────────────────────────
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

SCAN_OUT="${TMP_DIR}/scan.json"
API_SURFACE_OUT="${TMP_DIR}/api-surface.json"

# Step 1: Scan
if ! "$ANALYZER_BIN" scan "$REPO_ROOT" -o "$SCAN_OUT" 2>/dev/null; then
    echo "[index] scan failed — skipping" >&2
    exit 0
fi

# Step 2: API Surface
if ! "$ANALYZER_BIN" api-surface "$REPO_ROOT" -o "$API_SURFACE_OUT" --format json 2>/dev/null; then
    echo "[index] api-surface failed — skipping" >&2
    exit 0
fi

# Step 3: Sync to knowledge store
SYNC_ARGS=(
    sync
    -m "$SCAN_OUT"
    -a "$API_SURFACE_OUT"
    --project-id "$PROJECT_ID"
    --api-url "$DIRAIGENT_API_URL"
    --api-token "$DIRAIGENT_API_TOKEN"
    -c "${REPO_ROOT}/.analyzer-sync-cache.json"
)
if [ -n "${AGENT_ID:-}" ]; then
    SYNC_ARGS+=(--agent-id "$AGENT_ID")
fi

if ! "$ANALYZER_BIN" "${SYNC_ARGS[@]}" 2>/dev/null; then
    echo "[index] sync failed — skipping" >&2
    exit 0
fi

# ── Persist state ───────────────────────────────────────────
echo "$HEAD_COMMIT" > "$STATE_FILE"
echo "[index] done (commit ${HEAD_COMMIT:0:12})"
