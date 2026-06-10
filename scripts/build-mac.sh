#!/usr/bin/env bash
# 本地打包 Mac M1 + Intel；Windows 需在 Windows 电脑或 GitHub Actions 上打
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> 安装 Rust target（若已安装会跳过）"
rustup target add aarch64-apple-darwin x86_64-apple-darwin 2>/dev/null || true

echo "==> 构建前端"
npm run build

echo "==> Mac M1 (arm64)"
npx tauri build --target aarch64-apple-darwin

echo "==> Mac Intel (x64)"
npx tauri build --target x86_64-apple-darwin

mkdir -p release
ARM64_DMG="src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/DeskKit_1.0.0_aarch64.dmg"
X64_DMG="src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/DeskKit_1.0.0_x64.dmg"

cp -f "$ARM64_DMG" release/DeskKit-1.0.0-mac-arm64.dmg
cp -f "$X64_DMG" release/DeskKit-1.0.0-mac-intel.dmg

echo ""
echo "==> 完成！Mac 安装包："
ls -lh release/*.dmg

echo ""
echo "Windows 包请在 Windows 上执行: npm run tauri:build"
echo "或使用 GitHub Actions: .github/workflows/build.yml"
