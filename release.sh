#!/usr/bin/env bash
set -euo pipefail

TAG="v$(date -u +%Y%m%d-%H%M)"

git checkout main
git merge --squash dev

# Generate changelog entry and commit message from the squashed diff
COMMIT_MSG=$(git diff --cached --stat | claude -p \
  "You are writing a release commit message and changelog entry.
   Above is the diff stat for a squash merge from dev to main.
   Here are the individual commits being merged:

   $(git log main..dev --oneline)

   1. Output a COMMIT MESSAGE (first line: 'Release $TAG', then blank line, then bullet points summarizing the changes — group by area: API, Orchestra, Web, TUI, etc.)
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
git tag "$TAG"

git push origin main --tags
git push github main --tags

# Merge main back into dev so changelog + tag are present
git checkout dev
git merge main --no-edit