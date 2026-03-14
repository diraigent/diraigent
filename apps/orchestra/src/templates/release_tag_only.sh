#!/usr/bin/env bash
# Diraigent release template: tag-only
# Tags the current HEAD of $DIRAIGENT_BRANCH and pushes the tag. No merge.
#
# Environment variables (set by orchestra):
#   DIRAIGENT_PROJECT_ID      — project UUID
#   DIRAIGENT_PROJECT_PATH    — absolute path to the repo
#   DIRAIGENT_BRANCH          — branch to tag (e.g. "main")
#   DIRAIGENT_TARGET_BRANCH   — (unused in tag-only mode)
#   DIRAIGENT_USER_ID         — user who triggered the release
#   DIRAIGENT_VERSION         — auto-generated version tag
set -euo pipefail
cd "$DIRAIGENT_PROJECT_PATH"

SOURCE="${DIRAIGENT_BRANCH}"

# Checkout source branch
git checkout "$SOURCE"

# Pull latest (best-effort)
git pull --rebase origin "$SOURCE" 2>/dev/null || true

# Tag
TAG="${DIRAIGENT_VERSION}"
git tag "$TAG"

# Push tag to all remotes
for REMOTE in $(git remote); do
  git push "$REMOTE" "$TAG" || echo "failed to push $TAG to $REMOTE"
done

echo "Released $TAG (tagged $SOURCE at $(git rev-parse --short HEAD))"
