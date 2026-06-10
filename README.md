# DeskKit (Tauri 版)

本地文件与文档工具集，使用 **Tauri 2 + Vue 3 + Vite + Pinia + Rust** 重构。

原 Electron/Node 版保留在 `/Users/xiaojiang/web/sync_file`，本项目为全新实现。

📖 **详细教程**：[使用与打包教程](./docs/使用与打包教程.md)（安装、常用流程、开发、打包、常见问题）

## 功能

| 模块 | 说明 |
|------|------|
| 文件夹对比 | MD5/路径对比、主辅同步、并集、删除、移动 |
| 批量重命名 | 前缀/后缀/替换/序号/指定位置插入/清理非法字符 |
| 收藏管理 | 保存常用文件夹组合 |
| 重复文件 | 按 MD5 查找同目录重复文件 |
| 语雀导出 | API Token / 分享链接、批量导出、断点续导、限流间隔 |
| Confluence 转换 | Markdown → HTML / DOCX / MD（PDF 请用 HTML 打印） |
| 设置 | 忽略规则、默认对比模式 |

用户数据仅存 **浏览器 localStorage**（与原版一致）。

## 快速开始

**使用者（已打包）**：双击 `DeskKit.app` 即可，详见 [使用与打包教程](./docs/使用与打包教程.md#一使用者安装与启动)。

**开发者**（需 **Node.js 24+**，见 `.nvmrc`）：

```bash
cd /Users/xiaojiang/web/deskKit
npm install
npm run tauri:dev      # 开发
npm run tauri:build    # 打包
```

## 开发

```bash
cd /Users/xiaojiang/web/deskKit
npm install
npm run tauri:dev
```

## 打包

```bash
# 当前平台
npm run tauri:build

# Mac 双架构（需分别安装 target）
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri:build:mac
```

产物在 `src-tauri/target/release/bundle/`。

## 架构

```
src/                 Vue 3 前端（Element Plus + Pinia）
src-tauri/src/       Rust 后端（Tauri commands）
  compare_sync.rs    对比与同步
  rename_ops.rs      重命名
  duplicates.rs      重复文件
  yuque.rs           语雀导出
  confluence.rs      Markdown 转换
```

前端通过 `@tauri-apps/api` 的 `invoke()` 调用 Rust，不再启动 Node server。

## 与 Electron 版差异

- 安装包体积显著更小（无 Electron、无 Node、无 Puppeteer）
- PDF 导出：请使用 HTML 导出后在浏览器「打印为 PDF」
- 文件夹选择：Tauri 原生对话框（Mac / Windows 均支持）
