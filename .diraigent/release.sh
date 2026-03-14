#!/usr/bin/env bash
set -euo pipefail

# Usage: release.sh [--release]
#   Without --release: squash-merge dev→main, generate changelog, no tag/push
#   With --release:    same + tag + push to all remotes
#
# Environment variables (set by orchestra, or defaults):
#   DIRAIGENT_BRANCH         — source branch (default: dev)
#   DIRAIGENT_TARGET_BRANCH  — target branch (default: main)
#   DIRAIGENT_VERSION        — tag name (default: vYYYYMMDD-HHMM)

RELEASE="${DIRAIGENT_RELEASE:-false}"
for arg in "$@"; do
  case "$arg" in
    --release) RELEASE=true ;;
  esac
done

SOURCE="${DIRAIGENT_BRANCH:-dev}"
TARGET="${DIRAIGENT_TARGET_BRANCH:-main}"
TAG="${DIRAIGENT_VERSION:-v$(date -u +%Y%m%d-%H%M)}"

git checkout "$TARGET"
git merge --squash "$SOURCE"

# Generate changelog entry and commit message from the squashed diff
COMMITS=$(git log "$TARGET".."$SOURCE" --oneline)

if $RELEASE; then
  MSG_FIRST_LINE="Release $TAG"
else
  MSG_FIRST_LINE="Merge $SOURCE into $TARGET"
fi

COMMIT_MSG=$(git diff --cached --stat | claude -p \
  "You are writing a release commit message and changelog entry.
   Above is the diff stat for a squash merge from $SOURCE to $TARGET.
   Here are the individual commits being merged:

   $COMMITS

   1. Output a COMMIT MESSAGE (first line: '$MSG_FIRST_LINE', then blank line, then bullet points summarizing the changes — group by area: API, Orchestra, Web, TUI, CI/CD, etc.)
   2. Output '---CHANGELOG---' on its own line
   3. Output a CHANGELOG entry in this format:
      ## $TAG ($(date -u +%Y-%m-%d))
      - bullet points of notable changes (user-facing, not internal refactors)

   Output ONLY the commit message and changelog, nothing else." 2>/dev/null)

# Split output into commit message and changelog
COMMIT_BODY=$(echo "$COMMIT_MSG" | sed '/^---CHANGELOG---$/,$d')
CHANGELOG_ENTRY=$(echo "$COMMIT_MSG" | sed '1,/^---CHANGELOG---$/d')

# Insert changelog entry after the "# Changelog" header
if [ -f CHANGELOG.md ] && grep -q '^# Changelog' CHANGELOG.md; then
  sed '/^# Changelog$/r /dev/stdin' CHANGELOG.md <<EOF > CHANGELOG.md.tmp

$CHANGELOG_ENTRY

---
EOF
  mv CHANGELOG.md.tmp CHANGELOG.md
else
  cat > CHANGELOG.md <<EOF
# Changelog

$CHANGELOG_ENTRY
EOF
fi

git add .
git commit -m "$COMMIT_BODY"

if $RELEASE; then
  git tag "$TAG"

  # Push to all configured remotes
  for remote in $(git remote); do
    git push "$remote" "$TARGET" --tags || true
  done
fi

# Merge target back into source so changelog is present in both branches
git checkout "$SOURCE"
git merge "$TARGET" --no-edit
