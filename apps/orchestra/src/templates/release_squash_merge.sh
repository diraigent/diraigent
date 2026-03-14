#!/usr/bin/env bash
# Diraigent release template: squash-merge
# Squash-merges $DIRAIGENT_BRANCH into $DIRAIGENT_TARGET_BRANCH, tags, and pushes.
#
# Environment variables (set by orchestra):
#   DIRAIGENT_PROJECT_ID      — project UUID
#   DIRAIGENT_PROJECT_PATH    — absolute path to the repo
#   DIRAIGENT_BRANCH          — source branch (e.g. "dev")
#   DIRAIGENT_TARGET_BRANCH   — target branch (e.g. "main")
#   DIRAIGENT_USER_ID         — user who triggered the release
#   DIRAIGENT_VERSION         — auto-generated version tag
set -euo pipefail
cd "$DIRAIGENT_PROJECT_PATH"

SOURCE="${DIRAIGENT_BRANCH}"
TARGET="${DIRAIGENT_TARGET_BRANCH}"

# Verify source branch exists
git rev-parse --verify "$SOURCE" >/dev/null 2>&1 || { echo "source branch '$SOURCE' does not exist"; exit 1; }

# Checkout target branch
git checkout "$TARGET"

# Pull latest (best-effort)
git pull --rebase origin "$TARGET" 2>/dev/null || true

# Check there are commits to merge
COUNT=$(git rev-list --count "$TARGET".."$SOURCE")
if [ "$COUNT" -eq 0 ]; then
  echo "nothing to release: $SOURCE has no new commits over $TARGET"
  exit 1
fi

# Squash merge
git merge --squash "$SOURCE"

# Build commit message from log
LOG=$(git log --oneline --no-merges "$TARGET".."$SOURCE")
MSG="release: squash merge $SOURCE into $TARGET

$LOG"

git commit -m "$MSG"

# Tag
TAG="${DIRAIGENT_VERSION}"
git tag "$TAG"

# Push to all remotes
for REMOTE in $(git remote); do
  git push "$REMOTE" "$TARGET" || echo "failed to push $TARGET to $REMOTE"
  git push "$REMOTE" "$TAG"    || echo "failed to push $TAG to $REMOTE"
done

echo "Released $TAG ($COUNT commits from $SOURCE)"
