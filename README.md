# Merge Clash

[![CI](https://github.com/huihuibuswo/merge-clash/actions/workflows/ci.yml/badge.svg)](https://github.com/huihuibuswo/merge-clash/actions/workflows/ci.yml)
[![Build Windows Installer](https://github.com/huihuibuswo/merge-clash/actions/workflows/release.yml/badge.svg)](https://github.com/huihuibuswo/merge-clash/actions/workflows/release.yml)

Merge Clash 是一个面向 Mihomo/Clash 配置的本地桌面工具。它用于管理多个订阅、合并节点、可视化编辑代理组、校验并预览 YAML，以及在局域网内发布可供其他设备订阅的配置地址。

## 功能

- 管理、测试和刷新多个订阅源。
- 使用内置模板合并 Mihomo/Clash 配置。
- 可视化编辑代理组并检查无效引用。
- 预览、导出和发布通过校验的 YAML。
- 在局域网启动本地订阅服务并生成访问地址。
- 使用 SQLite 在本机保存设置、草稿和发布记录。

## 下载 Windows 安装包

### 正式版本

打开仓库的 [Releases](https://github.com/huihuibuswo/merge-clash/releases) 页面，下载对应版本的 `.exe` 安装程序。

### 最新 CI 构建

1. 打开 [Build Windows Installer](https://github.com/huihuibuswo/merge-clash/actions/workflows/release.yml) 工作流。
2. 可以下载最近一次 `main` 推送生成的构建，或选择 `Run workflow` 手动启动新构建。
3. 构建完成后，在任务页面底部下载 `Merge-Clash-Windows-*` artifact。
4. 解压 artifact 后运行其中的 `.exe` 安装程序。

CI 产物默认保留 30 天。安装包当前未进行商业代码签名，Windows 可能显示 SmartScreen 未知发布者提示。

## 本地开发

环境要求：

- Windows 10/11
- Node.js 22
- Rust stable 与 Cargo
- Microsoft Edge WebView2 Runtime

安装依赖：

```powershell
npm ci
```

启动桌面开发环境：

```powershell
npm run tauri:dev
```

仅启动前端开发服务器：

```powershell
npm run dev
```

## 构建与验证

执行前端构建和 Rust 测试：

```powershell
npm run check
```

生成 Windows NSIS 安装程序：

```powershell
npm run tauri:build -- --bundles nsis
```

安装程序输出到：

```text
src-tauri/target/release/bundle/nsis/
```

## CI/CD

- `.github/workflows/ci.yml`：向 `main` 推送或创建 Pull Request 时，执行前端构建和 Rust 测试。
- `.github/workflows/release.yml`：推送 `main` 或手动运行时生成可下载的 Actions artifact；推送 `v*` 标签时同时创建或更新 GitHub Release，并上传 `.exe` 安装程序。

创建正式版本前，请确保 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 中的版本号一致，然后推送标签：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

## 技术栈

- Vue 3、TypeScript、Vite、Pinia、Naive UI
- Tauri 2、Rust、Tokio、SQLx、SQLite
- GitHub Actions

## 数据与安全

应用数据默认保存在本机。局域网发布功能会监听本地网络端口；只应在可信网络中启用，并妥善保管生成的订阅访问令牌。
