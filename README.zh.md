# Velvet

> **白翎** —— 让素材与信息一目了然。

[English](README.md) · [架构决策](docs/adr/) · [变更记录](CHANGELOG.md) · [许可证](LICENSE)

---

> [!NOTE]
> Velvet `0.0.3` 是一个 Workbench 基础版本。React 壳层、带版本控制的布局模型、原生 Tauri 窗口、外部插件生命周期和持久化操作面板已经建立，素材索引器和生产级扩展仍在开发中。

## 一个工作台，两种入口

Velvet 是一个使用 Rust 和 Tauri 构建的桌面工作台，用于在一个可灵活调整的工作区中管理素材和信息。

首批内置工作台包括：

- **资产管理器**：管理图片、RAW 文件、视频、音频、文档和 3D 模型。
- **信息订阅工作台**：集中管理订阅、消息和阅读材料。

## 一看就懂的布局

界面设计借鉴了 VS Code 和 JetBrains IDE 的空间布局思想。每个区域都有明确职责，用户无需学习插件特有的布局规则，就能理解某项功能应该出现在哪里。

```text
活动栏       在内置工作台之间切换
主侧栏       导航、素材库、来源和集合
工作区       网格、预览、比较和编辑器
辅助侧栏     检查器、元数据和第三方工具
操作面板     任务、日志、同步和类似终端的操作
状态栏       当前状态和进度
```

初始版本按功能固定三类扩展区域：

1. `navigation`：工作台和素材库导航；
2. `contextual`：元数据、检查和预览工具；
3. `operations`：任务、日志和操作工具。

扩展可以在所属区域内定制内容。初始架构不允许扩展把贡献内容移动到其他区域。

## 选择合适深度的预览

Velvet 将预览设计为由宿主管理的能力，而不是为每种文件格式创建一个独立的大型查看器。

- **快速预览**：显示干净、响应迅速的预览。
- **检查**：显示有用的文件和元数据信息。
- **分析**：提供 RAW 直方图、RGB 曲线、色彩信息或 3D 材质控制等专业工具。

预览系统将主渲染器与增强能力分开：

```text
ContentKindProvider   识别文件类型
PreviewProvider       渲染主预览
InspectorContribution 增加元数据和分析区块
PreviewTool           增加交互式命令
AnalysisJob           执行耗时的后台工作
```

3D 或 RAW 扩展可以提供渲染器、缩略图、元数据和分析能力。只提供元数据的扩展也可以向辅助侧栏增加信息，而不替换主预览。

## 按能力扩展

扩展 manifest 会声明自己的贡献内容和权限。宿主会在注册前校验扩展所属区域。

- 导航扩展使用素材库、来源和工作台 API。
- 上下文扩展使用素材、元数据和预览 API。
- 操作扩展使用任务、日志以及经过明确授权的进程或网络 API。

第三方扩展计划运行在隔离的 WASM 或 Sidecar 进程中。内置适配器可以在进程内运行，但仍然使用同一套公开贡献模型。扩展通过稳定的协议类型通信，不依赖内部 Vue 或 React 组件类型。

## 架构

首个版本采用本地优先的单进程拓扑：

```text
Tauri Webview
      │ CoreTransport
      ▼
Rust ApplicationCore
      ├── 布局状态和迁移
      ├── 预览解析器
      ├── 插件宿主
      ├── 持久化
      └── 任务和索引
```

前端负责渲染和交互。Rust 负责持久化状态、权限、插件生命周期、预览选择和后台工作。Tauri 适配器保持精简，因此未来可以使用同一协议接入本地守护进程或远程传输。

桌面前端采用以应用为中心的组织方式，借鉴 vben 对可运行应用、可复用能力和工程工具的分层思路，同时保持首个客户端自包含：

```text
apps/desktop/src/
  app/          启动流程和依赖装配
  platform/     CoreTransport 和面向 Tauri 的适配器
  workbench/    布局、区域、视图、预览和扩展宿主
  workspaces/   资产管理器和信息订阅工作台业务模块
```

工作台本身与 Vue、React 解耦。渲染器只消费工作台公开的描述和接口；只有出现第二个客户端或需要独立版本管理的 SDK 时，才提取前端根级 package。

## 从源码构建

### Rust 工作区

通过 [rustup](https://rustup.rs/) 安装 Rust，然后运行：

```bash
cargo test --workspace
cargo fmt --all -- --check
```

### 桌面 TypeScript 骨架

```bash
cd apps/desktop
npm install
npm run typecheck
```

当前桌面包包含应用装配入口、平台传输类型、与框架无关的 Workbench、两个内置工作台描述、预览宿主、视图注册表、区域策略和插件客户端接口。完整的 Tauri 窗口和生产级 UI 将在后续实现。

## 仓库结构

```text
crates/
  application-core/       Rust 状态所有者和请求分发器
  workbench-protocol/     命令、查询、事件和传输 seam
  layout-model/           Part、区域和版本化布局状态
  preview-core/           预览意图、provider、解析器和 fallback
  plugin-api/             manifest、贡献和权限
  plugin-host/            注册、激活和区域校验
  persistence/            布局存储 seam
  job-system/             后台任务模型
  tauri-adapter/          面向 Tauri 的传输适配器

apps/desktop/
  src/app/                 桌面端依赖装配入口
  src/platform/            与框架无关的核心传输类型
  src/workbench/           布局和扩展宿主基础设施
  src/workspaces/          资产管理器和信息订阅工作台模块
  src-tauri/               未来的 Tauri 命令集成边界
```

## 项目文档

- [术语表](docs/GLOSSARY.md)
- [架构决策](docs/adr/)
- [工作台基础计划](docs/engineering/workbench-foundation-plan.md)
- [变更记录](CHANGELOG.md)
- [许可证](LICENSE)

## 许可证

Velvet 使用 [Apache License 2.0](LICENSE) 发布。
