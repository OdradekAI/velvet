<p align="center">
  <img src="../../docs/logo/logo.png" alt="Velvet" width="120" />
</p>

# Velvet 桌面应用

[English](README.md) | [简体中文](README.zh.md)

Velvet 桌面应用使用 React、TypeScript 和 Vite 构建界面，以 Tauri 作为原生宿主。本目录负责应用组装、界面渲染和原生集成。持久化的产品状态、权限、扩展生命周期和后台工作由 Rust 的 `ApplicationCore` 与领域 crate 负责。

[项目概览](../../README.zh.md) · [前端架构](../../docs/adr/0008-desktop-frontend-organization.md) · [原生宿主](src-tauri/README.zh.md) · [文档站](../docs/README.zh.md)

## 准备开发环境

请检出完整仓库，并安装 **Node.js 22** 和 npm，与桌面应用 CI 保持一致。原生开发还需要 Rust stable，以及 [Tauri 前置要求](https://v2.tauri.app/start/prerequisites/)中列出的平台依赖。启动器准备脚本使用 Bash；Windows 上请使用 Git Bash，或 CI 所用的同等构建环境中的 Bash。

从仓库根目录执行：

```sh
cd apps/desktop
npm ci
```

除非另有说明，下文命令均在 `apps/desktop` 中执行。

## 运行应用

### 浏览器开发

```sh
npm run dev
```

打开终端中 Vite 输出的地址，通常是 `http://localhost:5173`。在 Tauri 之外运行时，渲染器会启动 `DemoApp`。此模式适合开发布局、组件和测试 fixture 中的交互；原生传输与操作系统集成需要在 Tauri 应用中运行。

### 原生开发

先为当前 Rust 编译目标准备启动器二进制文件，再启动 Tauri：

```sh
bash ../../tools/build-release-launcher.sh
npm run tauri -- dev
```

Tauri 通过 `beforeDevCommand` 启动 Vite，并打开 `NativeWorkbenchApp`。请确保端口 `5173` 可用：`src-tauri/tauri.conf.json` 指向该地址。修改启动器源码或目标架构后，需要重新构建启动器。

## 代码职责边界

| 路径 | 职责 |
| --- | --- |
| `src/app/` | 启动与依赖组装 |
| `src/app/renderer/` | React 工作台外壳、原生/演示入口与 UI 测试场景 |
| `src/app/ui/` | 可信的 Velvet UI 组件、设计 token、契约与交互场景 |
| `src/platform/` | 面向传输层的类型与 Tauri 适配器 |
| `src/workbench/` | 与框架无关的布局、视图、命令、预览和扩展基础设施 |
| `src/workspaces/` | 内置 WorkbenchApp 描述符与适配器 |
| `src-tauri/` | 原生启动、命令/事件映射、恢复与打包 |
| `test/`, `tools/` | 前端回归测试、测试场景运行器与验收工具 |
| `../../crates/` | Rust 应用、领域与协议的实现模块 |

保持导入方向：`app → platform/workbench/workspaces`、`workspaces → public workbench/platform contracts`、`workbench → platform`。`platform` 不得依赖 React 或功能模块。渲染组件通过传输边界发送类型化请求，不直接读取持久化存储或插件运行时。完整的边界规则见 [ADR-0008](../../docs/adr/0008-desktop-frontend-organization.md)，术语见[术语表](../../docs/GLOSSARY.md)。

文档站通过 `src/app/ui/index.ts` 使用可信组件示例。这些 React 导出不是第三方扩展 SDK；扩展作者应使用文档中定义的贡献契约和隔离 UI 桥接层。

## 验证变更

| 命令 | 用途 |
| --- | --- |
| `npm run typecheck` | 严格的 TypeScript 类型检查 |
| `npm test` | 编译测试场景并运行前端回归测试 |
| `npm run build` | 执行类型检查并生成 Vite 生产构建产物 |
| `npm run generate:ui-kit-catalog` | 重新生成纳入版本控制的组件目录 |
| `npm run verify:ui-kit` | 执行类型检查、组件目录生成、测试、构建和浏览器 UI 验收 |

使用与 CI 相同的运行器执行可复现的浏览器验收：

```sh
npx playwright install chromium
UI_BROWSER_ENGINE=playwright npm run verify:ui-kit
```

以上环境变量赋值语法适用于 POSIX shell。在 Linux 上，CI 使用 `npx playwright install --with-deps chromium`，同时安装浏览器所需的系统依赖。生成文件 `src/app/ui/ui-kit-catalog.json` 的变更应与生成它的契约变更一并保留。

Rust 检查需另行**从仓库根目录**执行：

```sh
cargo fmt --all -- --check
cargo test --workspace
bash tools/build-release-launcher.sh
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --features launcher-bin
```

`src-tauri` 是独立的 Cargo workspace，因此根 workspace 的测试不会覆盖它。请根据所修改的边界选择检查；涉及传输、权限和恢复的变更，需要相应的 Rust 与前端测试覆盖。平台矩阵和发布验收步骤见[桌面应用 CI](../../.github/workflows/desktop.yml)。

## 构建原生安装包

```sh
npm run tauri -- build
```

Tauri 会调用 `build:tauri`，构建前端并准备启动器 sidecar。所选目标的安装包产物位于 `src-tauri/target/` 下。单独运行 `npm run build` 只会在 `dist/` 中生成 Web 资源。

签名、更新器配置和发布验收由仓库的发布工作流负责。修改相关流程时，请遵循[发布验收指南](../../tools/release-qualification/README.md)和[原生宿主 README](src-tauri/README.zh.md)。

## 生成文件与故障排查

[`.gitignore`](.gitignore) 管理本应用的依赖与构建产物忽略规则，包括 `node_modules/`、`dist/`、原生 `target/`、启动器 `binaries/`、生成目录 `gen/` 和 `src-tauri/tauri.release.generated.json`。仓库级忽略规则仍放在根目录的文件中。前端测试目前将编译产物写入 `/tmp/velvet-plugin-tests`；测试命令会重新创建其中的依赖链接。

| 现象 | 检查方法 |
| --- | --- |
| 浏览器模式没有原生集成能力 | 运行 `npm run tauri -- dev`；浏览器开发模式按设计使用 `DemoApp` |
| Tauri 无法加载前端 | 检查 Vite 是否在配置的端口 `5173` 上运行 |
| 缺少启动器二进制文件，或目标平台不匹配 | 针对所选 Rust 编译目标重新运行 `bash ../../tools/build-release-launcher.sh` |
| 原生编译因系统库失败 | 检查对应平台的 Tauri 前置要求和 CI 配置 |
| 浏览器验收找不到 Chromium | 使用 Playwright 安装 Chromium，并选择 `UI_BROWSER_ENGINE=playwright` |
