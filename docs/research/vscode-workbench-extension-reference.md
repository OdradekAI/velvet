# VS Code 工作台与扩展生态参考基线

> 本文是 VS Code 的对照资料，不是 Velvet 首期需求清单。Editor、编辑器
> 语言服务和 LSP 章节用于识别未来文本型应用可能需要的能力；Velvet 当前
> 的 editorless Workbench P0 不依赖这些组件。

> 目的：给 Velvet 的骨架审计提供一份可复查的 VS Code 参考基线。本文只记录 VS Code 的公开契约与本地源码证据，不把 VS Code 的全部产品功能当成 Velvet 首期必须照搬的实现。
>
> 取证时间：2026-10-30（Asia/Shanghai）。本地参考源码：`/Users/luiz/Odradek/vscode`。

## 结论先行

VS Code 的“基础工作台骨架”不是几个固定的 Vue/React 区块，而是四层契约叠加：

1. **Parts 与可恢复的布局状态**：窗口、标题栏、Activity Bar、主/次侧栏、Panel、Editor、Status Bar、通知、工具栏和编辑器组共同组成 Workbench；用户移动、隐藏、分组、拆分和重启后恢复这些区域。
2. **贡献点与运行时**：扩展以版本化 manifest 声明 contribution points、activation events、运行位置和能力；宿主验证、索引、按事件激活、通过 RPC 映射到主线程，并在扩展失效时保持核心可用。
3. **语言服务契约**：语言支持默认由扩展提供。声明式能力（language id、TextMate grammar、language configuration、snippets）和程序化能力（hover、completion、diagnostics、definition、format、refactor 等）分开接入；LSP 是可复用的进程边界。
4. **生态运维契约**：扩展需要安装/扫描/启用/禁用/更新/卸载、profile、内置扩展、兼容性、workspace trust、virtual workspace、web/remote host、状态存储、secrets、日志和失败隔离。

审计 Velvet 时，至少要逐项回答：该契约是否存在、谁拥有状态、扩展能否声明并被宿主消费、失败是否隔离、重启/迁移/禁用后是否可恢复、有没有兼容性测试或 fixture。

## 官方文档基线

| 主题 | 官方来源 | 可抽取的验收事实 |
| --- | --- | --- |
| Extension API 总览 | [Extension API](https://code.visualstudio.com/api/) | 几乎所有 UI 与编辑能力都可通过 Extension API 扩展；VS Code 的很多核心能力本身也是扩展；文档覆盖构建、调试、测试、发布、贡献点、扩展宿主和远程开发。 |
| 能力分类 | [Extension Capabilities Overview](https://code.visualstudio.com/api/extension-capabilities/overview) | Common、Theming、Declarative Language、Programmatic Language、Workbench、Debugging 是独立能力面；扩展不能访问 VS Code DOM 或注入 CSS，必须使用稳定 API。 |
| 工作台布局 | [Custom Layout](https://code.visualstudio.com/docs/configure/custom-layout) | 支持主/次侧栏、Activity Bar 位置、视图与 Panel 拖放、Panel 位置/对齐/最大化、通知位置、工具栏菜单管理、编辑器组/网格/浮动窗口、Pinned Tab、Locked Group；布局要跨会话保存并可重置。 |
| 语言支持 | [Language Extensions Overview](https://code.visualstudio.com/api/language-extensions/overview) | 核心编辑器不内置具体语言；语言扩展提供声明式与程序化能力。声明式包括 TextMate 语法高亮、snippets、括号匹配/自动闭合/环绕、注释、自动缩进、标记折叠；程序化包括 hover、completion、definition、错误检查、format、refactor、folding 等。 |
| 语言 API | [VS Code API: languages](https://code.visualstudio.com/api/references/vscode-api#languages) | 扩展只提供数据，编辑器负责 UI、定位、稳定性；provider 按 document selector 匹配并有 score/优先级。 |
| 贡献点 | [Contribution Points](https://code.visualstudio.com/api/references/contribution-points) | manifest 的 `contributes` 是声明式扩展面；`views` 可挂到内置或自定义容器，打开视图会触发 `onView:<id>`，TreeView/WebviewView 是两种内容模型。 |
| 激活 | [Activation Events](https://code.visualstudio.com/api/references/activation-events) | 扩展通过 `activate()`/`deactivate()` 生命周期和按需 activation events 懒加载；事件包括 `onLanguage`、`onView`、`onCommand`、`onFileSystem`、`onTaskType`、`onTerminal`、`onNotebook`、`workspaceContains` 等。 |
| 扩展宿主 | [Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host) | 有 local Node、web Worker、remote Node 三类宿主；manifest 的 `main`/`browser`、`extensionKind` 与能力决定运行位置；宿主隔离扩展以保护启动性能、UI 响应和稳定性。 |
| 远程扩展 | [Supporting Remote Development](https://code.visualstudio.com/api/advanced-topics/remote-extensions) | UI extension 与 workspace extension 可能在不同机器/宿主；跨宿主不能依赖返回的 JS API，应通过命令等异步协议通信；状态应使用 ExtensionContext 提供的存储与 SecretStorage。 |
| Web 扩展 | [Web Extensions](https://code.visualstudio.com/api/extension-guides/web-extensions) | 浏览器环境使用 web extension host；文件访问必须通过 `vscode.workspace.fs`，不能假设 Node 文件系统。 |
| 扩展清单 | [Extension Manifest](https://code.visualstudio.com/api/references/extension-manifest) | `engines.vscode`、`main`/`browser`、`activationEvents`、`contributes`、`capabilities`、`extensionDependencies` 等是生态兼容性与运行定位的基础字段。 |

## Workbench 基础布局模型

### 区域与职责

官方布局文档与本地源码共同指向以下区域：

| 区域/能力 | 官方语义 | 本地 VS Code 证据 |
| --- | --- | --- |
| Title Bar / Menu / Command Center | 窗口标题、菜单和布局控制；可以控制原生/自绘标题栏、菜单可见性和命令中心 | `src/vs/workbench/browser/parts/titlebar/titlebarPart.ts`、`titlebarActions.ts`、`commandCenterControl.ts`、`src/vs/workbench/electron-browser/parts/titlebar/titlebarPart.ts` |
| Banner | 窗口顶部提示和产品级 banner | `src/vs/workbench/browser/parts/banner/bannerPart.ts` |
| Activity Bar | 稳定的工作区入口、视图容器导航、徽标和拖放；可左右/上下/隐藏、compact | `src/vs/workbench/browser/parts/activitybar/activitybarPart.ts`、`compositeBar.ts`、`globalCompositeBar.ts` |
| Primary Side Bar | Explorer、Search、SCM 等 View Container 的承载区；可左右移动、调整宽度、视图间拖放 | `src/vs/workbench/browser/parts/sidebar/sidebarPart.ts`、`parts/views/viewPaneContainer.ts` |
| Secondary Side Bar / Auxiliary Bar | 与主侧栏相对的一组视图，可同时查看第二个上下文；位置跟随主侧栏相反 | `src/vs/workbench/browser/parts/auxiliarybar/auxiliaryBarPart.ts`、`auxiliaryBarActions.ts` |
| Editor Part | 编辑器组、编辑器 Tab、Diff/Side-by-side、编辑器输入模型、Breadcrumbs、Minimap 等 | `src/vs/workbench/browser/parts/editor/editorPart.ts`、`editorGroupView.ts`、`editorTabsControl.ts`、`editorPanes.ts`、`src/vs/workbench/services/editor/common/editorGroupsService.ts` |
| Panel | Problems、Terminal、Output 等面板；可上下左右移动、对齐、最大化，并与侧栏互相拖放 | `src/vs/workbench/browser/parts/panel/panelPart.ts`、`panelActions.ts` |
| Status Bar | 状态入口、消息、通知铃铛、扩展贡献的 status bar item | `src/vs/workbench/browser/parts/statusbar/statusbarPart.ts`、`statusbarModel.ts`、`statusbarItem.ts`、`src/vs/workbench/services/statusbar/browser/statusbar.ts` |
| Notifications | Toast、Notification Center、alerts、可访问视图、位置和状态入口 | `src/vs/workbench/browser/parts/notifications/notificationsToasts.ts`、`notificationsCenter.ts`、`notificationsAlerts.ts`、`notificationsStatus.ts` |
| View / View Container | 视图描述、容器模型、可见性、排序、移动、sash 尺寸、拖放、toolbar/context menu | `src/vs/workbench/common/views.ts`、`src/vs/workbench/services/views/browser/viewDescriptorService.ts`、`services/views/common/viewContainerModel.ts`、`browser/parts/views/viewPaneContainer.ts` |

Workbench 创建时直接实例化这些 Parts。`src/vs/workbench/browser/workbench.ts:325-341` 的数组包含 `TITLEBAR_PART`、`BANNER_PART`、`ACTIVITYBAR_PART`、`SIDEBAR_PART`、`EDITOR_PART`、`PANEL_PART`、`AUXILIARYBAR_PART`、`STATUSBAR_PART`；随后在 362-389 行创建通知中心、toast、alerts、状态和通知命令。

布局不是一次性的 DOM：`src/vs/workbench/browser/layout.ts` 同时保存 initialization/runtime state，监听可见性、Panel position/alignment、Zen/centered layout、窗口/容器变化，并恢复编辑器布局。`src/vs/workbench/services/layout/browser/layoutService.ts` 定义稳定的 Part ID、位置、Activity Bar、Editor Tabs、Zen Mode 等枚举和设置。

### 布局交互与持久化验收表

首期工作台骨架如果宣称支持“基础布局”，应至少逐项验证：

- [ ] Title Bar、Activity Bar、Primary Side Bar、Editor、Panel、Secondary Side Bar、Status Bar、Notification Center 都能渲染、隐藏和恢复。
- [ ] Activity Bar 可以移动到主侧栏外侧、顶部、底部或隐藏；主侧栏可左右切换；次侧栏总在另一侧。
- [ ] View/Panel 可以在主侧栏、次侧栏、Panel 之间移动，拖动到现有区域可形成组；键盘命令也能完成移动。
- [ ] 每个 View Container 的视图顺序、可见性、折叠状态、宽度/高度、sash 尺寸有明确 owner、序列化格式和迁移策略；重启后恢复。
- [ ] 有 `Reset View Locations`/`Reset Layout` 等可恢复默认的命令；未知扩展视图被禁用或卸载后不阻塞布局恢复。
- [ ] Editor 支持多个 Editor Group、左右/上下 split、grid、tab active/pinned/locked、Diff/Side-by-side、编辑器状态恢复。
- [ ] Panel 支持位置（left/right/bottom/top）、对齐（center/justify/left/right）、最大化和单面板内部 tab/split。
- [ ] 浮动编辑器/辅助窗口的创建、关闭、位置、always-on-top/compact（若产品支持）有独立窗口状态和恢复行为。
- [ ] Notification toast、center、alerts 和 status bar 入口有一致的生命周期、可访问性和错误处理。
- [ ] Focus、键盘导航、快捷键、菜单/上下文菜单和命令面板与布局状态一致；移动布局不能只依赖鼠标。

官方文档明确了上述行为：主/次侧栏与拖放和跨会话记忆见 Custom Layout 的 Workbench 小节；Panel 的位置、对齐、最大化与拖放见 Panel 小节；通知位置见 Notifications 小节；编辑器 group/grid/floating/pinned/locked 见 Editor 小节。

## 扩展生态骨架

### Manifest 与声明式契约

VS Code 扩展不是直接注入 Workbench 组件。标准最小清单需要：

- 唯一身份（`publisher` + `name`）、版本与显示信息；
- `engines.vscode` 兼容范围；
- `main`（Node host）和/或 `browser`（web host）入口；
- `activationEvents` 与 `activate`/`deactivate` 生命周期；
- `contributes` 声明式贡献；
- `capabilities`（例如 `virtualWorkspaces`、`untrustedWorkspaces`）；
- `extensionDependencies`、依赖和运行位置（`extensionKind`，若需要）；
- 可选 schema、图标、分类、README/变更记录和发布元数据。

本地 `src/vs/workbench/services/extensions/common/extensionPoints.json:1-61` 当前登记 59 个 extension point：

```text
authentication, breakpoints,
chatAgents, chatContext, chatInstructions, chatOutputRenderers,
chatParticipants, chatPlugins, chatPromptFiles, chatSessions, chatSkills,
chatViewsWelcome,
colors, commands, configuration, configurationDefaults, continueEditSession,
css, customEditors, debugVisualizers, debuggers,
grammars, iconThemes, icons, jsonValidation, jsonValidationRegistry,
keybindings, languageModelChatProviders, languageModelToolSets,
languageModelTools, languages, linkPresentationProviders, localizations,
mcpServerDefinitionProviders, menus, notebookPreload, notebookRenderer,
notebooks, problemMatchers, problemPatterns, productIconThemes,
remoteCodingAgents, remoteHelp, resourceLabelFormatters,
semanticTokenModifiers, semanticTokenScopes, semanticTokenTypes, snippets,
speechProviders, statusBarItems, submenus, taskDefinitions, terminal,
terminalQuickFixes, themes, views, viewsContainers, viewsWelcome, walkthroughs
```

这些点不是都要在 Velvet v1 一次实现，但它们揭示了生态骨架需要分组而不是只做一个“插件注册函数”：

| 契约组 | 典型 contribution points / API | 骨架应提供的能力 |
| --- | --- | --- |
| 命令与交互 | `commands`、`menus`、`submenus`、`keybindings`、when/context keys、Command Palette | 稳定 command id；菜单位置与分组；条件可见性；快捷键冲突与禁用；命令参数序列化；可追踪的 activation。 |
| 工作台视图 | `views`、`viewsContainers`、`viewsWelcome`、`statusBarItems` | 扩展注册 view/container；声明 zone/location；Tree/Webview provider；toolbar/context menu；空状态和欢迎内容；badge/状态栏；可移动和可持久化。 |
| 编辑器扩展 | `customEditors`、notebook、notebookRenderer、linkPresentationProviders、document content providers | editor type 注册、打开/保存/恢复、资源 URI、serializer、renderer 安全边界和编辑器组兼容。 |
| 语言 | `languages`、`grammars`、`snippets`、semantic token types/scopes/modifiers、`jsonValidation`、程序化 `vscode.languages.*` | language id 到文件/URI 的 selector；声明式配置；provider 注册/撤销；诊断、completion、hover、definition、references、symbols、rename、format、code actions、inlay/folding/semantic tokens。 |
| 任务与终端 | `taskDefinitions`、problemMatchers/patterns、`terminal`、terminalQuickFixes | 任务类型/schema；任务发现/解析；取消、进度、问题解析；终端 profile、shell integration 和快速修复。 |
| 调试与工程 | `debuggers`、`breakpoints`、`debugVisualizers`、SCM API、Testing API | Debug Adapter/会话生命周期；断点/变量可视化；SCM provider、资源组/状态；测试 controller、树、运行/取消/结果。 |
| 外观与本地化 | `themes`、`colors`、`iconThemes`、`productIconThemes`、`icons`、`localizations`、CSS/custom data | 主题 token、图标、产品图标、语言包、颜色与无障碍对比度；扩展不得依赖内部 DOM/CSS。 |
| 服务与 AI/远程 | `authentication`、language model providers/tools、MCP、speech、remote help/coding agents | capability/permission；secret/auth 生命周期；远程位置；跨宿主协议；故障隔离和资源配额。 |

### 运行时、激活和隔离

官方 Extension Host 文档定义三类宿主：本地 Node、浏览器 WebWorker、远程 Node。扩展的运行位置由可用 host、`main`/`browser`、能力、安装位置和 `extensionKind` 共同决定。宿主负责懒激活，避免扩展拖慢启动和 UI 操作。

本地实现中：

- `src/vs/workbench/services/extensions/common/abstractExtensionService.ts` 维护扩展描述注册表、激活事件读取器、扩展运行位置、远程崩溃跟踪、启用/禁用/安装/卸载/profile 变更，并将文件系统事件转成 `onFileSystem:<scheme>` 激活。
- `src/vs/workbench/services/extensions/common/extensionHostManager.ts` 启动宿主、建立 RPC、缓存/触发 activation events、跟踪启动成功/失败和响应性。
- `src/vs/workbench/api/common/extHostExtensionService.ts` 在宿主侧加载扩展、维护 activation、storage/secrets/workspace/configuration 等 ExtHost 服务。
- `src/vs/workbench/api/common/extHost.protocol.ts` 与 `src/vs/workbench/api/browser/mainThread*.ts` 构成跨边界协议和主线程 customer；例如语言、文件、编辑器、视图、任务、终端、SCM、调试、Notebook、Webview 等都各有 bridge。
- `src/vs/workbench/services/extensions/common/rpcProtocol.ts`、`proxyIdentifier.ts`、`extensionHostProtocol.ts` 是 RPC/DTO 边界；不能把扩展对象或 UI 内部实例直接跨边界传递。

激活事件清单见官方 Activation Events；最关键的工作台骨架事件包括 `onLanguage[:id]`、`onView[:id]`、`onCommand[:id]`、`onFileSystem:<scheme>`、`onTaskType:<type>`、`onTerminal[:profile/shellIntegration]`、`onNotebook[:type]`、`onWebviewPanel:<viewType>`、`workspaceContains:<glob>`、`onStartupFinished`。从 1.74 起，声明 commands/languages/views/tasks 可隐式激活，但宿主仍需保留事件索引和兼容行为。

### 扩展管理与生命周期

要称为“可交付生态骨架”，宿主至少需要能管理：

- 扫描 builtin、user、profile、remote、web 扩展并解析 manifest；
- 校验 id、版本、入口、贡献点 schema、引擎范围、能力和依赖；
- install/update/uninstall（包含下载、签名/完整性、缓存、迁移、失败回滚策略）；
- enable/disable（全局、workspace/profile、受信任工作区规则）；
- extension host 的启动、停止、重启、崩溃隔离和诊断日志；
- 扩展自己的 global/workspace storage、secrets、URI/path 适配和迁移；
- 内置扩展与外部扩展同一套公开 contribution/API，避免内置路径绕过宿主；
- web/remote host 的安装位置、同步和跨宿主命令通信；
- 市场/画廊元数据、更新检查、兼容平台/架构筛选（若首期暂不接市场，也要定义本地包/内置包格式）。

本地源码证据：

- `src/vs/platform/extensionManagement/common/extensionManagement.ts`、`.../extensionGalleryService.ts`、`.../extensionsScannerService.ts` 定义管理/扫描接口；
- `src/vs/workbench/services/extensionManagement/browser/builtinExtensionsScannerService.ts`、`webExtensionsScannerService.ts`、`extensionsProfileScannerService.ts` 负责内置、Web、profile 扫描；
- `src/vs/workbench/services/extensionManagement/browser/extensionEnablementService.ts` 负责启用状态；
- `src/vs/workbench/services/extensionManagement/electron-browser/extensionManagementService.ts`、`nativeExtensionManagementService.ts`、`remoteExtensionManagementService.ts` 连接本地/原生/远程安装；
- `src/vs/platform/extensions/common/extensionValidator.ts`、`src/vs/platform/extensionManagement/node/extensionSignatureVerificationService.ts` 提供清单校验与签名验证边界。

## Language 基线（容易遗漏的部分）

### 声明式语言能力

一个可交付的 Language 骨架需要把以下数据模型分开：

1. **Language identity / selector**：language id、文件扩展名、文件名、first line、URI scheme、embedded language 关系；文档到 provider 的匹配需要稳定规则和优先级。
2. **Language configuration**：comments、brackets、autoClosingPairs、surroundingPairs、wordPattern、indentationRules、onEnterRules、folding markers、electric characters、word definition。
3. **Tokenizer / grammar**：TextMate grammar、scope、injection、embedded languages；主题 token 映射不能写死在视图组件。
4. **Snippets**：语言/文件范围、变量/占位符/转换、优先级和冲突行为。
5. **Schema/validation**：JSON/YAML 等文件的 schema provider、fileMatch、远程/本地 schema 解析和缓存。
6. **Semantic tokens**：token type/modifier/scope 的稳定 id、legend、增量更新、主题映射和失效。

官方 Language Extensions Overview 明确声明这些能力由扩展提供；不能因为编辑器能显示纯文本就把 Language 视为已完成。

### 程序化语言能力

Provider 注册至少覆盖：completion、hover、signature help、definition、declaration、type definition、implementation、references、document/workspace symbols、document highlight、rename、prepare rename、format document/range/on type、code actions/resolve、code lenses、folding、selection range、inlay hints、inline values、inline completions、linked editing、call hierarchy、type hierarchy、document colors、document links、document range formatting、diagnostics、semantic tokens、document paste/drop/edit 等。

官方 API 的 `languages` 命名空间和本地 `mainThreadLanguageFeatures.ts` 共同表明：编辑器拥有 UI 和生命周期，扩展只提供 provider 数据；每个注册都要有 handle、selector、取消/超时语义和 dispose 路径。对应本地证据：

- `src/vs/workbench/api/browser/mainThreadLanguages.ts`：语言注册、language configuration 等主线程入口；
- `src/vs/workbench/api/browser/mainThreadLanguageFeatures.ts`：各类 provider 的 DTO、注册表、取消和回调桥接；
- `src/vs/editor/common/services/languageFeaturesService.ts`、`languageFeatureRegistry.ts`：核心 provider registry、排序和匹配；
- `src/vs/editor/common/languages/languageConfiguration.ts`、`languageConfigurationRegistry.ts`、`src/vs/editor/common/services/languagesRegistry.ts`：语言 id 与声明式配置；
- `src/vs/editor/common/services/languagesAssociations.ts`、`src/vs/editor/common/languages/modesRegistry.ts`：文件/URI 到语言的关联；
- `src/vs/workbench/api/common/extHostLanguageFeatures.ts`、`extHostLanguages.ts`：扩展侧 provider API。

### 内置语言扩展样本

本地 `extensions/` 目录是验证语言生态是否真正可用的参考实现：

- `extensions/html-language-features/package.json`：`main` + `browser` 双入口、`onLanguage:html/handlebars`、`virtualWorkspaces`、`untrustedWorkspaces`、configuration、JSON validation；
- `extensions/css-language-features/package.json`：`onLanguage:css/less/scss`、双入口、workspace/untrusted 能力、configuration、JSON validation；
- `extensions/typescript-language-features/package.json`：多语言 activation、commands、configuration、JSON validation、problem matchers/patterns、task definitions、workspace/virtual-workspace 能力；其源码包含 completion、hover、definition、references、diagnostics、formatting、rename、refactor、code lens、folding、semantic tokens、inlay hints、call hierarchy 等 provider；
- `extensions/markdown-language-features/package.json`：language activation、custom editor、webview preview、notebook renderer、link presentation、menus/keybindings/configuration，是语言与工作台扩展组合的样本；
- `extensions/git/package.json`：commands/keybindings/menus、SCM view welcome、configuration、file-system activation 和 extension dependency，展示非语言扩展如何消费同一宿主。

如果 Velvet 只完成“文件类型枚举 + 语法高亮占位”，而没有 selector、配置、snippets、diagnostics/provider 注册、dispose、能力声明、激活和 workspace/remote 限制，则 Language 仍处于骨架缺口。

## 建议作为 Velvet 审计表的字段

每一项差距都用同一字段记录，避免遗漏或只报 UI：

| 字段 | 说明 |
| --- | --- |
| Reference capability | VS Code 官方能力/契约名称和链接 |
| Local evidence | Velvet 文件、类型、测试、ADR 或缺失证据；VS Code 对照路径 |
| Owner | `ApplicationCore`、`PluginHost`、`LayoutService`、`PreviewResolver`、`JobSystem`、frontend workbench 或 adapter |
| State | durable / workspace / profile / session / derived / cache |
| Seam | CoreTransport、Rust trait、plugin manifest、registry、event、RPC/DTO 或测试 fake |
| Failure boundary | 扩展崩溃、超时、禁用、卸载、升级、远程断开、协议不兼容时的行为 |
| Compatibility | id/version/schema/migration/unknown-field/feature negotiation |
| Evidence status | present / partial / missing / proposed-only |
| Acceptance | 可执行的最小场景、fixture 或契约测试 |
| Priority | P0（阻断首期骨架）、P1（首期完整交付需要）、P2（生态扩展阶段） |

## 首期骨架的参考边界

VS Code 的完整生态很大，Velvet 首期不必复制 Marketplace、所有 AI contribution 或所有远程产品。但“工作台模板”至少应让后续 Asset Manager、Feed Workbench 和第三方插件在同一套边界上工作：

### P0：没有这些就不能称为工作台骨架

- Parts/区域模型：title/activity/primary sidebar/editor/panel/secondary sidebar/status/notifications；可见性、位置、尺寸、顺序、focus、restore/reset。
- Editor host：至少一个稳定的 editor input/type、group/tab/split、active editor、dirty/save/close、恢复和错误 fallback。
- View/container registry：内置视图与插件视图同一注册路径；view/container id、zone/location、排序、可移动、空状态、toolbar/menu、dispose。
- Command/context/menu/keybinding seam：稳定 id、when 条件、参数 DTO、Command Palette、禁用/冲突处理。
- Plugin manifest + validation：唯一 id、版本、host API range、入口、activation、贡献点 schema、权限/capability、未知字段策略。
- Plugin runtime：隔离进程/Worker 或至少可替换 host seam；lazy activation；取消/超时；失败隔离；禁用/卸载后核心 fallback。
- Language foundation：language identity/selector、language configuration、grammar/tokenizer、snippets，以及 provider registry 的最小闭环（至少 diagnostics、completion、hover、definition、format 或 LSP adapter 的清晰契约）。
- Transport bridge：前端只消费投影/事件，不能直读 filesystem/database/plugin runtime；跨边界请求有 typed error、version 和 cancellation。
- Persistence/migration：布局、插件状态、扩展 facet、workspace/profile 状态各自有 owner、schema version、迁移和恢复策略。

### P1：首期交付后续应用前应补齐

- 主/次侧栏、Panel、Editor 的拖放/分组/键盘移动与跨会话恢复；Pinned/Locked editor group；Panel 对齐/最大化。
- View welcome、status bar、notification center、toolbar hide/reset、theme/icon tokens、accessibility/focus。
- Extension storage（global/workspace）、secret/capability、workspace trust、virtual workspace、profile/enablement。
- Web/Node 双宿主能力声明、remote extension location、RPC/DTO、host restart/crash diagnostics。
- Task/problem matcher、terminal profile/shell integration、SCM、debug、testing、custom editor/notebook 的扩展接口占位与 fake adapter。
- JSON schema/validation、semantic tokens、embedded language、multi-root workspace、LSP client/server adapter。
- 本地包安装/更新/卸载、完整性检查、兼容性、依赖图和回滚；Marketplace 可延期但包格式不可含糊。

### P2：生态规模化阶段

- Marketplace/gallery/account/update channel、签名/供应链策略、远程 server 自动安装、跨窗口/跨 profile 同步、第三方 SDK 版本治理、proposed API/实验能力、遥测和性能预算。

这份边界的关键是把“布局做出来”和“插件可交付”分开验收：只画出 Activity Bar、Sidebar、Editor、Panel 的页面，不等于具备 VS Code 级的 Workbench；只保存一个插件 id 列表，也不等于具备扩展生态。
