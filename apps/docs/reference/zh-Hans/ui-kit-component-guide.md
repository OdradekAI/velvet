---
id: ui-kit-component-guide
title: Velvet UI Kit 组件指南
---

# Velvet UI Kit 组件指南

状态：UI Kit v1 的宿主实现。

渲染层 UI Kit 的实现位于 `apps/desktop/src/app/ui/index.ts`、`primitives/`、`overlays/` 和 `host-projections/` 下的分层模块，以及 `apps/desktop/src/app/ui/theme.ts`。`velvet-ui.tsx` 是兼容性汇总导出入口；新增的第一方代码应从 `ui/index.ts` 导入。其公开行为以语义契约定义并进行版本管理；React 实现和 CSS 类名仍是宿主内部细节。

## 视觉基线 {/* #visual-baseline */}

默认宿主主题以已观察到的桌面工作台视觉语言为依据：紧凑的导航区、工作区和 Inspector 区域，近乎扁平的炭灰色表面，细分隔线，圆角选中行，紧凑的系统无衬线字体，克制的靛蓝或紫色强调色，以图标为主的工具栏，以及引导用户采取行动的空状态。这些是 Velvet 的设计参考，不意味着像素级兼容，也不代表可以复制参考素材。浅色、宽松密度和高对比度模式复用相同的语义状态与无障碍规则。

使用 Velvet 语义令牌和组件变体设置界面样式。Tailwind 类、CSS 变量名、DOM 结构和 Radix 数据属性属于实现细节，不得成为扩展或插件契约。

<span id="公开边界" className="velvet-doc-anchor" />

## 版本接口 {/* #version-surfaces */}

- 宿主组件行为的 `UI_KIT_API_VERSION` 为 `1.0`。
- `velvet.ui.theme/1` 是发送给插件界面的只读语义令牌快照。
- 宿主公布 UI Bridge 组件契约版本 `1`。注册时校验描述符兼容性；`surface.ready` 报告的桥接版本或组件版本不兼容时，对应界面会独立失败并被释放。激活流程不等待 `surface.ready`。

新增字段和可选令牌保持兼容。改变令牌含义、键盘模型或状态所有者，需要升级契约主版本，并为旧版本提供测试夹具（fixture）。宿主可以替换 React、Radix 或 CSS 内部实现而不改变契约。

<span id="组件入口" className="velvet-doc-anchor" />

## 宿主组件 {/* #host-components */}

`VelvetButton`、`VelvetIconButton`、`VelvetInput`、`VelvetSelect`、`VelvetCheckbox`、`VelvetText`、`VelvetIcon`、`VelvetBadge`、`VelvetStack`、`VelvetInline`、`VelvetSeparator`、`VelvetTabs`、`VelvetProgress`、`VelvetState`、`VelvetToastProvider`、`VelvetToastHost`、`VelvetActivityBar`、`VelvetPane`、`VelvetPreviewHost`、`VelvetAssetGrid`、`VelvetDataTable`、`VelvetCommandPalette`、`VelvetQuickSearch`、`VelvetFilterPanel`、`VelvetFilterFacetMenu`、`VelvetInspector` 和 `VelvetZoomControl` 都是宿主拥有的渲染组件。`VelvetPanel`、`VelvetDialog` 和 `VelvetContextMenu` 是现有工作台界面使用的兼容封装，目前也遵循相同的令牌、焦点和状态规则。

基础组件提供语义属性和标准 React 事件。受控选择、浮层和投影状态由宿主拥有；本地表单控件可以使用默认状态。Dialog 和 Context Menu 将 Radix 隐藏在 Velvet 封装之后。未来新增 Radix 基础组件时，应以具体宿主用途逐项引入，并先完成包体积和交互资格验证。

<span id="组件分层" className="velvet-doc-anchor" />

## 模块布局 {/* #module-map */}

第一轮迁移为每个可复用基础组件提供独立模块：

```text
apps/desktop/src/app/ui/
  internal/                 # common props, data attributes and contract types
  primitives/<component>.tsx
  contracts/primitives/<component>.contract.ts
  stories/primitives/<component>.stories.tsx
  index.ts                  # formal first-party barrel
  velvet-ui.tsx             # compatibility re-export barrel
```

`overlays/` 拥有 Dialog、Context Menu 和 Toast 的实现；`host-projections/` 拥有 Panel、Activity Bar、Pane、PreviewHost、Inspector、AssetGrid、DataTable、FilterPanel、FilterFacetMenu、CommandPalette、QuickSearch 和 ZoomControl。Radix 依赖保留在 Velvet 浮层封装内部。`legacy-components.tsx` 目前也仅作为兼容性再导出入口。

<span id="组件页面阅读顺序" className="velvet-doc-anchor" />

## Story、契约测试与智能体目录 {/* #stories-contract-tests-and-agent-catalog */}

每个组件都有自己的 `*.stories.tsx` 模块，包含类型化元数据和具名 story。Velvet 维护 story 注册表及运行器；首个运行器使用现有 TypeScript 编译流程、Node 和 Happy DOM，不引入 Storybook 运行时依赖。Story 是可执行测试夹具，不会像静态截图那样脱离组件 API。

交互式 story 可以附加类型化生命周期场景：`setup → actions → checkpoints → cleanup`。运行器提供记录语义调用的模拟宿主适配器，并用 `act` 边界处理 React 更新。检查点可以断言 DOM/ARIA、焦点、发出的动作载荷和恢复状态。仅限浏览器验证的界面沿用相同场景结构；涉及 portal、几何尺寸或原生焦点行为时，通过真实浏览器资格验证脚本执行。

UI Kit 的统一验收入口是在 `apps/desktop` 中运行 `npm run verify:ui-kit`。它依次运行类型编译与目录检查、Node/Happy DOM 契约和 story 测试、生产构建，以及真实浏览器资格验证夹具。浏览器验证需要 `http://127.0.0.1:5173` 上的 Vite 夹具服务器；`qualify:ui-browser` 会复用现有服务器，或启动临时服务器并在结束后清理。CI 设置 `UI_BROWSER_ENGINE=playwright`，使用受管理的无头 Chromium 执行相同验收；本地默认使用已安装的 ego-browser 引擎。Playwright 资格验证还会记录标准的 1280×800 视觉基线，并拒绝共享外壳密度、等宽导航区与 Inspector 区域、工具栏和筛选界面的层次、选中卡片几何尺寸以及 4:3 资产缩略图发生偏移。

模拟宿主适配器还为异步 story 提供类型化延迟操作。成功路径记录 `requested → pending → committed`；终止状态包括 `cancelled`、`timed-out` 和 `failed`。终止失败之后，`retry()` 会开始一次新的 pending 尝试。这使成功、取消、超时、失败和重试都可以确定性地验证，无需将 story 连接到 `CoreTransport`。

每个交互组件都覆盖有意义的状态矩阵、键盘行为和无障碍契约：适用时包括默认、选中或打开、焦点、禁用、加载或错误，以及空状态。静态布局组件验证语义结构和代表性变体，不虚构交互状态。Story 可以使用模拟宿主投影，但不得创建第二个状态所有者或绕过 CoreTransport 语义。

组件发现以类型化的 `ComponentMeta` 记录及其配套的 `ComponentContract` 为准。构建过程为开发者和智能体生成 JSON 目录。目录条目描述稳定组件 id、类别、所有权层、契约 id、文档路径、语义用途、属性和事件、支持的状态、键盘与无障碍保证、story 名称、资格验证层级（`node`、`browser` 或 `both`）以及用法示例。目录不公开 CSS 类名、Radix 类型、私有 DOM 结构或插件内部实现细节。目录与 UI Kit 契约共同进行版本管理，并在 CI 中生成，防止元数据悄然偏离 story。

当前目录输入位于 `apps/desktop/src/app/ui/component-meta.ts`、`contracts/`、`story-registry.ts` 和独立的 `stories/` 模块中。生成结果是 `ui-kit-catalog.json`。修改元数据或 story 名称后，应在 `apps/desktop` 中运行 `npm run generate:ui-kit-catalog`；若生成结果产生差异，CI 必须失败。动作矩阵记录在 `docs/engineering/ui-kit/action-coverage.md`；每个场景声明 `scenario.covers`，需要浏览器验证的动作则以 `browserActions` 发布在类型化契约和目录中。

影响较大的宿主投影还发布类型化的视觉与行为一致性契约。视觉部分定义语义表面、密度、圆角和几何不变量；行为部分定义状态所有者、焦点模型、选择所有者，以及每个动作的 requested、committed、cancelled、failed、retry 转换。这些字段是目录元数据，不是 CSS 或 Radix API。组件契约测试会拒绝类型化契约与生成目录之间的偏差。

宿主拥有的界面包括活动栏、导航区、上下文区、Inspector、操作区、PreviewHost、全局浮层和全局快捷键。隔离插件可以通过注册表和桥接请求向这些界面提供贡献，但不能导入 React 组件、改变工作台网格、创建全局 portal，或添加文档级键盘监听器。

CommandPalette 和 QuickSearch 是只呈现投影的 UI 界面。它们接收宿主准备好的命令、结果和历史记录，上报原始查询、标签页和选择动作，仅拥有临时键盘与焦点呈现状态。查询解析、索引、拼音或简繁匹配、评分、历史持久化与失效处理属于 Workbench/CoreTransport，不属于 UI Kit 契约。

## 状态与无障碍规则 {/* #state-and-accessibility-rules */}

- 按钮派发宿主命令或本地界面动作；宿主在执行前检查命令可见性和能力。
- Tabs 使用移动式 Tab 焦点，并支持方向键、Home 和 End 导航。
- 活动栏使用移动式 Tab 焦点，并支持方向键、Home 和 End 导航。
- Asset Grid 提供 `role=grid`、选择状态以及方向键和 Enter 行为。核心选择状态仍由宿主投影拥有。
- Data Table 提供带标签的表格、可排序表头和可聚焦行。
- Inspector 接收宿主拥有的选中数量，提供受控元数据编辑，并呈现保存、校验、取消和重试状态。
- Dialog 通过 Radix 恢复焦点；使用外部触发元素的受控对话框传入 `restoreFocusRef`，使 Velvet 封装可以在 Escape 或外部点击关闭后恢复焦点。操作按钮保留在宿主拥有的页脚中。
- 加载、空状态和错误状态使用 `status`/`alert` 语义；有重试能力时，错误状态提供显式重试回调。
- Progress 是宿主 Job 状态的投影。渲染进度不意味着 Job 已完成。
- Toast 使用动态播报区域并要求显式关闭；通知不修改核心状态。
- `reducedMotion`、密度和对比度由主题快照携带，并反映到宿主 CSS 变量中。

## 插件适配器边界 {/* #plugin-adapter-boundary */}

隔离界面只接收语义令牌快照、语言区域、有界上下文、尺寸、焦点和生命周期消息。它可以通过 UI Bridge 发送类型化命令、浮层请求、通知或进度投影。宿主适配器决定是否渲染 Context Menu、Dialog、Toast 或 Progress 界面，并在执行前重新检查命令、上下文和能力状态。界面失败或插件禁用时会移除相应会话，工作台、Asset、Preview 和 Job 的所有者仍保持可用。

契约夹具位于 `apps/desktop/test/ui-kit-contract.test.mjs`；渲染组件和键盘夹具位于 `apps/desktop/test/ui-kit-components.test.mjs`。在 `apps/desktop` 中运行 `npm test` 可执行完整桌面端测试。
