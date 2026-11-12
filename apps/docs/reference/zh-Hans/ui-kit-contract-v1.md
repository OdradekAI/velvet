---
id: ui-kit-contract-v1
title: Velvet UI Kit contract v1
---

# Velvet UI Kit 契约 v1

<span id="velvet-ui-kit-contract-v1" className="velvet-doc-anchor" />

状态：Alpha 宿主契约已在仓库内冻结。本文描述桌面 UI Bridge 和宿主渲染 UI Kit 当前使用的框架无关接口。它不是已发布的第三方 SDK，也不为未来运行时指定 iframe、子 WebView 或原生渲染器。

冻结范围包括语义令牌、主题协商、组件可用性、语义区域、来源校验，以及释放与错误行为。不在冻结范围内的内容包括生产渲染器、跨平台无障碍资格验证和外部软件包 ABI。

<span id="版本面" className="velvet-doc-anchor" />

## 版本接口 {/* #version-surfaces */}

| 接口 | 当前版本 | 兼容规则 |
| --- | ---: | --- |
| UI Bridge 消息 | `1` | 注册时拒绝不兼容的描述符；`surface.ready` 报告的版本不同时，会使会话失败并释放。 |
| 主题模式 | `velvet.ui.theme/1` | 旧快照会先规范化；未知模式版本按拒绝原则处理。 |
| 组件契约 | `1` | 宿主公布版本 `1`；注册时检查已声明的版本，`surface.ready` 报告的版本不兼容时，会使会话失败并释放。 |

激活流程不等待 `surface.ready`；ready 消息到达时才检查其报告的兼容性。

宿主拥有布局、活动栏、窗格、PreviewHost 主预览选择、OperationsPanel、全局浮层、焦点恢复和全局快捷键。宿主渲染器实现[UI Kit 组件指南](ui-kit-component-guide.md)中的基础组件和组合组件。插件界面只接收投影和类型化动作；它不会收到 React 元素、DOM 节点、宿主 CSS 类、文件路径或其他 View 的实现。

## 主题快照 {/* #theme-snapshot */}

宿主发送只读的 `velvet.ui.theme/1` 快照：

```ts
interface UiThemeSnapshot {
  schema: 'velvet.ui.theme';
  schemaVersion: 1;
  scheme: 'light' | 'dark';
  density: 'compact' | 'default' | 'comfortable';
  contrast: 'normal' | 'high';
  reducedMotion: boolean;
  revision: number;
  tokens: Partial<Record<UiSemanticToken, string>>;
}
```

公开语义令牌的允许列表位于 `apps/desktop/src/workbench/plugins/ui-kit.ts`。基础色值、字体文件、Tailwind 类和组件实现令牌仍是宿主私有细节。快照跨越界面边界前，会移除未知令牌、控制字符以及包含 `url(`、`expression(` 或闭合 style 标签的 CSS 值。插件不能将快照写回宿主。

## 组件可用性 {/* #component-availability */}

插件界面可以通过桥接契约描述或渲染兼容令牌的文本、图标、按钮、输入框、选择框、复选框、标签页、菜单、对话框正文、Toast、进度、加载、空状态、错误、列表、树、表格和网格内容。活动栏、导航/上下文/操作窗格、全局浮层、PreviewHost、OperationsPanel 和全局快捷键仍由宿主拥有。

自定义 View 通过宿主注册表注册，并保持在声明的语义区域内。自定义界面接收宿主推送的尺寸、语言区域和主题更新，并可以请求焦点和已注册命令派发。它不能创建第二个布局所有者、安装宿主全局按键处理器、向宿主文档注入任意 DOM/CSS 或直接修改宿主状态。隔离渲染器拥有自己的文档。

## 视觉系统与渲染器边界 {/* #visual-system-and-renderer-boundary */}

默认宿主视觉语言以已观察到的桌面工作台设计为参考：紧凑的导航区、工作区和 Inspector 区域，近乎扁平的炭灰色表面，细分隔线，圆角选中行，紧凑的系统无衬线字体，单一饱和靛蓝或紫色强调色，以图标为主并明确提供搜索、筛选和缩放控件的工具栏，以及引导用户采取行动的空状态。宿主可以通过现有语义主题快照支持浅色、宽松密度和高对比度模式；这些模式保留相同的组件状态和所有权规则。

视觉系统通过 Velvet 语义令牌和具名组件变体表达。基础色值、Tailwind 工具类、DOM 结构、参考素材和精确像素值都是实现细节。若视觉修改改变键盘行为、状态所有权或令牌含义，就需要作出 UI 契约版本决策；调色和间距优化则保留在 Velvet 封装内部。

公开的第一方渲染接口是 `Velvet*` React 封装层。封装提供语义属性、宿主拥有选择或浮层生命周期时的受控状态、本地输入的默认状态、标准 React 事件和 Velvet 无障碍行为。它们不暴露 Radix 类型、`asChild`、Radix 数据属性、Tailwind 类名或宿主私有 portal。Radix Dialog 和 Context Menu 保持为内部基础实现；额外的 Radix 包按具体宿主用途逐项引入，并先测量生产包体积。

组件集按所有权分为三组：

1. **基础组件**：text、icon、button、icon button、input、select、checkbox、badge、separator、stack、inline、tabs、progress 和 state。
2. **宿主组合组件**：Activity Bar、Panel、Pane、PreviewHost、Inspector、AssetGrid、DataTable、FilterPanel、FilterFacetMenu、CommandPalette、QuickSearch 和 ZoomControl。它们保留宿主拥有的上下文、焦点、布局和投影状态。
3. **浮层封装**：目前包括 Dialog、Context Menu 和 Toast；只有具体宿主界面需要时才引入 Tooltip、Popover、DropdownMenu 或 ScrollArea。

每组实现都可以在语义封装和框架无关插件桥接之后替换。隔离扩展接收组件名称、主题令牌，以及类型化浮层与命令请求；它们不会接收 React 元素或渲染器实现依赖。

Story 模块和生成的智能体目录属于宿主开发产物。它们验证语义封装行为并记录支持的状态，不会扩展插件桥接，也不会将 story 元数据变成运行时权限接口。

<span id="surface-lifecycle" className="velvet-doc-anchor" />

## 界面与来源规则 {/* #surface-and-origin-rules */}

当前宿主仅接受声明的 `http`、`https`、`tauri-webview`、`asset`、`data`、`blob`、`wasm` 和 `builtin` 入口协议。浏览器界面使用由入口推导出的精确网络来源发送消息，并丢弃来自其他来源的消息；不透明协议保留来源窗口检查。iframe 仍在沙箱内运行。Tauri 子 WebView 继续使用现有的显式能力和生命周期清理路径。

<span id="failure-boundary" className="velvet-doc-anchor" />

## 失败与兼容行为 {/* #failure-and-compatibility-behavior */}

- 不兼容的桥接版本、组件契约或主题模式会产生类型化宿主错误，使对应界面独立失败。
- 界面失败时显示宿主占位与重试路径，不改变布局、Asset、Preview、Job 或无关插件状态。
- 传输恢复会释放会话，保留宿主描述符，并允许在权威状态重新同步后创建新会话。
- 契约由 `apps/desktop/test/ui-kit-contract.test.mjs`、`plugin-surface-adapter.test.mjs` 和现有界面生命周期测试验证。

<span id="继续阅读" className="velvet-doc-anchor" />

完整的 Rust/Tauri/前端验证入口见[插件契约矩阵](plugin-contract-matrix.md)；面向插件的 UI 边界见[插件开发指南](plugin-developer-guide.md)。
