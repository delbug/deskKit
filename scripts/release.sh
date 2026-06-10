#!/usr/bin/env bash
# 自动 patch +1，提交、打 tag，并推送以触发 GitHub Actions 打包 Release
set -euo pipefail
cd "$(dirname "$0")/.."

BUMP="${1:-patch}"
PUSH=false
for arg in "$@"; do
  if [[ "$arg" == "--push" ]]; then
    PUSH=true
  fi
done
if [[ "$BUMP" == "--push" ]]; then
  BUMP="patch"
fi

if [[ ! "$BUMP" =~ ^(patch|minor|major)$ ]]; then
  echo "用法: $0 [patch|minor|major] [--push]"
  echo "  patch  默认，第三位 +1，如 1.0.2 → 1.0.3"
  echo "  minor  第二位 +1，如 1.0.2 → 1.1.0"
  echo "  major  第一位 +1，如 1.0.2 → 2.0.0"
  echo "  --push  自动 git push 与 push tag（触发 CI Release）"
  exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "错误: 工作区有未提交改动，请先 commit 或 stash"
  exit 1
fi

VERSION="$(node scripts/bump-version.mjs "$BUMP")"
TAG="v${VERSION}"

echo "==> 新版本: ${TAG}"
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/Cargo.lock
git commit -m "chore: release ${TAG}"

git tag "$TAG"

echo ""
echo "已本地提交并打 tag: ${TAG}"
echo "修改的文件:"
git show --stat --oneline HEAD | tail -n +2

if $PUSH; then
  echo ""
  echo "==> 推送到远程（将触发 GitHub Actions 打包 Release）"
  git push origin HEAD
  git push origin "$TAG"
  echo "完成。请在 GitHub Actions / Releases 查看进度。"
else
  echo ""
  echo "下一步（推送后 CI 会自动打包并发布 Release）:"
  echo "  git push origin HEAD"
  echo "  git push origin ${TAG}"
  echo ""
  echo "或一条命令: $0 ${BUMP} --push"
fi
