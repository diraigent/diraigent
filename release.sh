git checkout main
git merge --squash dev
git add .
git commit -m "Release"
#git tag "v$(date -u +%Y%m%d-%H%M)"

# git push origin main --tags
# git push github main --tags
git checkout dev