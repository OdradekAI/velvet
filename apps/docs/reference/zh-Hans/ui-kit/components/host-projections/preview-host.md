---
id: preview-host
title: Preview Host
sidebar_label: Preview Host
---

# VelvetPreviewHost

`VelvetPreviewHost` 是承载预览生命周期和降级界面的 Velvet 宿主投影组件。实现位于 `apps/desktop/src/app/ui/host-projections/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `state`、`mode`、`message`、`toolbar`、`providerActions`、`children`、`onRetry`、`onModeChange`、`onProviderAction`、`onOpenExternal`、`onClose`
- **状态：** loading、ready、empty、error、disabled
- **异步生命周期：** success、cancelled、timeout、failure、retry
- **无障碍：** 具名预览区域；加载时使用 busy 语义，错误和空白降级状态会被播报
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/preview-host.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/preview-host.stories.tsx`
- **宿主边界：** 布局、浮层、焦点和投影状态仍由宿主拥有。

组件契约只规定语义。Radix 类型、私有 DOM 结构、Tailwind 类和 Workbench 实现细节不属于公开 API。

## Story 与证据 {/* #stories-and-evidence */}

- `preview-host/ready`
- `preview-host/error`
- `preview-host/retry`
- `preview-host/toolbar-mode`
- `preview-host/provider-actions`
- `preview-host/main-window-lifecycle`
- `preview-host/async-success`
- `preview-host/async-cancelled`
- `preview-host/async-timeout`
- `preview-host/async-failure`
- `preview-host/async-retry`

异步 story 使用 story 运行器提供的模拟宿主适配器。请求从 `pending` 开始，随后提交、取消、超时或失败。重试在终止失败后创建新尝试，并将两次尝试都记录在宿主调用日志中；组件只渲染宿主拥有的投影状态，从不拥有底层传输或 Job。

浏览器夹具验证错误动态播报区域、重试动作及转入 ready 预览状态，还验证提供者工具栏、模式标记、模式切换、提供者动作的按下与禁用状态，以及显式外部打开回调。

`mode` 由宿主控制。`onModeChange` 和 `onOpenExternal` 是显式宿主动作回调；组件本身不会改变预览模式，也不会打开外部应用。

`providerActions` 是类型化宿主插槽，用于提供者专属控件，例如旋转、平移或幻灯片播放。每个动作提供稳定 id 和无障碍名称；宿主拥有按下与禁用状态，并通过 `onProviderAction` 接收所选 id。组件不推断提供者能力，也不直接调用预览提供者。

`onClose` 是宿主拥有的内嵌或详情预览模式退出动作。它在 ready 和降级状态下均可用，因此宿主可以退出加载、错误或无预览分支，而无需组件拥有导航状态。主窗口生命周期 story 和浏览器夹具覆盖 open → loading → ready、提供者失败 → 重试、纵向和横向内容、无预览降级，以及由宿主恢复焦点的关闭行为。

原生 Workbench 的 `PreviewOverlay` 目前直接使用这一投影。Workbench `PreviewHost` 仍是状态所有者，将 resolving/loading、ready、failed、unsupported 和传输恢复状态映射到 UI Kit 投影。提供者渲染器、增强插槽和通用降级内容仍作为宿主内容放在投影内部；UI Kit 不解析资产，也不调用 `CoreTransport`。

位于 `?fixture=native-preview` 的浏览器资格验证夹具使用模拟 `CoreTransport`，挂载真实的 `PreviewHost`、`PreviewOverlay`、`PreviewRendererRegistry` 和 `JobClient`。它断言提供者失败与重试、提供者渲染器适配器、增强渲染器，以及关闭后两个原生预览会话均被释放。

原生冒烟资格验证使用真实的 Rust `ApplicationCore`、`velvet-tauri-adapter::TauriTransport` 会话握手和请求信封。它仅在插件运行时边界注入有界内存预览运行时：首次提供者请求失败，重试返回一个 UI Bridge 提供者和一个元数据增强，释放请求则撤销两个会话的动作授权。在 `apps/desktop` 中运行：

```sh
npm run qualify:native-preview
```

该测试验证 Rust/Tauri 传输和预览生命周期，并不声称启动了平台 WebView 渲染器。浏览器夹具覆盖渲染器的几何尺寸和焦点；原生平台 WebView 的几何尺寸和焦点需要单独进行资格验证。

语义视觉契约将预览放在 canvas 表面，为提供者动作预留 48px 控制条，并将缩放边界保留在宿主投影中（`5–800`）。这些是目录中具名的不变量；CSS 类保持私有。

当前 story 运行器提供确定性夹具覆盖。对于浮层和宿主投影，修改焦点、portal、几何尺寸或宿主投影行为时，必须运行真实浏览器资格验证夹具。未经验证的外部行为仍标记为 `unknown`，直到运行时捕获提供证据。

<span id="无障碍与验证" className="velvet-doc-anchor" />

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
