#!/usr/bin/env bash
set -euo pipefail

# Usage: release.sh [--release]
#   Without --release: squash-merge dev→main, no changelog, no tag/push
#   With --release:    same + changelog + tag + push to all remotes
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
REPO_ROOT="$(git rev-parse --show-toplevel)"

git checkout "$TARGET"
git merge --squash "$SOURCE"

# Generate commit message (and changelog for releases) from the squashed diff
COMMITS=$(git log "$TARGET".."$SOURCE" --oneline)

if $RELEASE; then
  COMMIT_MSG=$(git diff --cached --stat | claude -p \
    "You are writing a release commit message and changelog entry.
     Above is the diff stat for a squash merge from $SOURCE to $TARGET.
     Here are the individual commits being merged:

     $COMMITS

     1. Output a COMMIT MESSAGE (first line: 'Release $TAG', then blank line, then bullet points summarizing the changes — group by area: API, Orchestra, Web, TUI, CI/CD, etc.)
     2. Output '---CHANGELOG---' on its own line
     3. Output a CHANGELOG entry in this format:
        ## $TAG ($(date -u +%Y-%m-%d))
        - group changes by type: ### Added, ### Changed, ### Fixed, ### Removed (omit empty sections)
        - each bullet should start with a bold component prefix: **API**, **Orchestra**, **Web**, **TUI**, or **CI/CD**
        - example: '- **Web**: Token usage chart on dashboard'

     IMPORTANT rules for the changelog:
     - Write for end users and admins, not developers. Omit internal refactors, code reorganization, module splits, and implementation details that have no user-visible effect.
     - Deduplicate: if multiple commits touch the same feature, write ONE bullet for the final result.
     - Keep bullets short (one line, under 100 chars). Focus on WHAT changed, not HOW.
     - Aim for 5-15 bullets total. Collapse minor fixes into a single 'Various UI/stability fixes' bullet if there are many small ones.
     - Do NOT mention file names, function names, struct names, or migration numbers.

     Output ONLY the commit message and changelog, nothing else." 2>/dev/null)

  # Split output into commit message and changelog
  COMMIT_BODY=$(echo "$COMMIT_MSG" | sed '/^---CHANGELOG---$/,$d')
  CHANGELOG_ENTRY=$(echo "$COMMIT_MSG" | sed '1,/^---CHANGELOG---$/d')

  # Insert changelog entry after the "# Changelog" header (in repo root)
  CHANGELOG="$REPO_ROOT/CHANGELOG.md"
  if [ -f "$CHANGELOG" ] && grep -q '^# Changelog' "$CHANGELOG"; then
    sed '/^# Changelog$/r /dev/stdin' "$CHANGELOG" <<EOF > "$CHANGELOG.tmp"

$CHANGELOG_ENTRY

---
EOF
    mv "$CHANGELOG.tmp" "$CHANGELOG"
  else
    cat > "$CHANGELOG" <<EOF
# Changelog

$CHANGELOG_ENTRY
EOF
  fi
else
  COMMIT_BODY=$(git diff --cached --stat | claude -p \
    "You are writing a commit message for a squash merge from $SOURCE to $TARGET.
     Above is the diff stat. Here are the individual commits being merged:

     $COMMITS

     Output a commit message: first line 'Merge $SOURCE into $TARGET', then blank line, then bullet points summarizing the changes grouped by area (API, Orchestra, Web, TUI, CI/CD).
     Output ONLY the commit message, nothing else." 2>/dev/null)
fi

git add .
git commit -m "$COMMIT_BODY"

if $RELEASE; then
  git tag "$TAG"
fi

# Push to all configured remotes (with tags only for releases)
for remote in $(git remote); do
  if $RELEASE; then
    git push "$remote" "$TARGET" --tags || true
  else
    git push "$remote" "$TARGET" || true
  fi
done

# Merge target back into source so changelog is present in both branches
git checkout "$SOURCE"
git merge "$TARGET" --no-edit
