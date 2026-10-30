# Velvet 对照 VS Code 的工作台骨架与插件生态审计

取证日期：2026-10-30（Asia/Shanghai）
参考实现：`/Users/luiz/Odradek/vscode`
被审计项目：`/Users/luiz/Odradek/velvet`

这份审计回答两个问题：

1. Velvet 当前有哪些真正可运行、可恢复、可测试的工作台基础设施；
2. 如果首期目标是“后续应用可以复用的完整工作台骨架”，还缺哪些契约、状态所有权、运行时和验收场景。

这里把 VS Code 作为能力基线，不把 VS Code 的全部产品功能直接当成 Velvet 的产品需求。Velvet 已经接受的固定语义区域仍然是一个有效的产品选择；当它与 VS Code 的可任意停靠布局冲突时，本文会明确标出需要重新决策的地方。

本文保留初始取证时的实现状态。后续决策票已经把 AppSurface 宿主契约和
插件清单/运行时契约写入 ADR-0010、ADR-0005 与 ADR-0006；因此下文仍然
列出的插件差距，除非明确标为“已决定”，都表示实现尚未完成，而不是
边界仍未确定。

View、Command、Menu、Keybinding、Focus 与 Overlay 的宿主边界也已在
Wayfinder 票中确认，并补入 ADR-0011/0006；下文对应条目继续表示待实现
的 provider、UI、可访问性和恢复测试，不再表示产品边界未决。

State、Transport 与 Failure Acceptance 也已确认：剩余差距是 owner、DTO、
迁移、resync 和故障矩阵的工程实现与测试，不是是否需要这些边界的产品
决策。

PreviewHost、Primary PreviewProvider、PreviewAugmentation、Inspector/Tool、
Job handoff 和 cache invalidation 的边界也已确认；审计中的 Preview 缺口
现在表示实现、adapter 和 fixture 尚未完成。

## 证据与验证范围

- VS Code 官方资料：
  - [Extension API](https://code.visualstudio.com/api/)
  - [Custom Layout](https://code.visualstudio.com/docs/configure/custom-layout)
  - [Language Extensions Overview](https://code.visualstudio.com/api/language-extensions/overview)
  - [Contribution Points](https://code.visualstudio.com/api/references/contribution-points)
  - [Activation Events](https://code.visualstudio.com/api/references/activation-events)
  - [Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host)
  - [Supporting Remote Development](https://code.visualstudio.com/api/advanced-topics/remote-extensions)
  - [Web Extensions](https://code.visualstudio.com/api/extension-guides/web-extensions)
  - [Extension Manifest](https://code.visualstudio.com/api/references/extension-manifest)
- VS Code 本地源码证据与能力清单见 [vscode-workbench-extension-reference.md](vscode-workbench-extension-reference.md)。
- Velvet 设计依据：`docs/GLOSSARY.md`、`docs/adr/0001`–`docs/adr/0012`、`docs/engineering/workbench-foundation-plan.md`。
- 已执行并通过：
  - `cargo test --workspace`
  - `cargo fmt --all -- --check`
  - `apps/desktop/npm run typecheck`
  - `apps/desktop/npm test`
- 测试通过表示现有切片自洽，不表示 VS Code 基线能力已经覆盖。当前计划第 12 行已经明确写出：React shell 是 renderer slice，尚未完成 foundation。

## 结论

Velvet 目前是“固定语义区域的 Workbench shell + Rust 所有权的布局/插件/Job 切片”，还不是可以承载多种应用的完整 Workbench 模板。现有实现的强项是边界已经被拆出来：Rust layout owner、revisioned transport、ViewContainer/View 注册、命名空间、隔离 UI surface、sidecar 启动、失败隔离、安装恢复和 Job 快照都已经有可运行证据。

首期缺口集中在四个阻断面：

1. **AppSurface 宿主契约不完整**：WorkspaceArea 由插件/WorkbenchApp 自行渲染是正确方向，但宿主仍需补齐 mount、suspend/resume、resize、theme、context、focus、权限、错误隔离和恢复的统一契约。
2. **Text/Language 能力属于可选后续能力**：代码中没有 language id/selector、language configuration、grammar/tokenizer、snippets、diagnostics、provider registry 或 LSP seam。它不是 WorkspaceArea 的内置职责，也不是当前 editorless Workbench P0 的阻断项；只有具体的文本/代码型 `WorkbenchApp` 需要它时，才应单独接入 capability pack。
3. **插件生态实现尚未完成**：初始代码中 Rust `plugin-api` 与 TypeScript `ContributionRegistry` 的贡献种类和 manifest 字段存在漂移，缺 activation events、host location、extension context、storage/secrets、依赖和 trust/profile/remote 约束。契约边界现已由 ADR-0005/0006 固定，剩余工作是迁移代码、统一 DTO 和补齐验收夹具。
4. **Workbench chrome 不完整**：没有独立 Banner、Notification Center/Toast/Alerts、完整的 View host、可移动 Activity Bar/Sidebar、Panel position/alignment/group 和完整的 view/panel 拖放恢复。

## 当前实现盘点

| 领域 | 当前证据 | 状态 |
| --- | --- | --- |
| Rust/桌面拓扑 | `application-core`、`workbench-protocol`、`tauri-adapter`、Tauri command/event mapping | 已有骨架 |
| Parts | 8 个 Part：TitleBar、ActivityBar、PrimarySidebar、WorkspaceArea、AuxiliarySidebar、OperationsPanel、StatusBar、OverlayLayer | 部分完成 |
| 布局状态 | `schemaVersion/revision/activeApp/parts/containers/views`，Rust 单一 owner，JSON 持久化和迁移入口 | 部分完成 |
| ViewContainer/View | 描述符、scope/zone、visibility/order/move、Activity Bar 投影、隔离 surface | 部分完成 |
| 命令/菜单/上下文 | command、keybinding、menu、context key、overlay registry | 部分完成 |
| Focus/键盘 | Activity Bar roving focus、Part resize keyboard、focus restoration | 部分完成 |
| App model | 内置 Resource Manager/Feed Workbench 描述符、插件 AppSurface seam | 部分完成，WorkspaceArea 的实际渲染交给 AppSurface，但内置 trusted surface 仍是占位/fallback |
| Preview | provider candidate/resolver、Quick/Inspect/Analyze、generic fallback | 最小闭环 |
| Jobs | Rust JobSystem、取消/重试/中断恢复、OperationsPanel 投影 | 最小闭环 |
| Plugin runtime | sidecar/WASM launch seam、health、crash isolation、JSONL command、install/rollback/recovery | 部分完成 |
| Plugin UI | iframe/Tauri child WebView、UI Bridge 生命周期和失败恢复 | 部分完成 |
| Plugin localization | Manifest 已有严格的 `localization` 元数据字段，但仍没有 l10n bundle loader、宿主解析服务或 locale revision 投影 | 契约已决定，运行时与夹具缺失 |
| WorkspaceArea/AppSurface | 有 isolated AppSurface 生命周期和 UI Bridge，但没有统一的 trusted/isolated surface host contract、能力握手和完整 fallback | 部分完成 |
| Text/Language capability | 未发现相关 registry/provider/grammar/snippet/LSP 实现 | 延后；属于可选插件能力，不是 WorkspaceArea 或 P0 基建职责 |
| Notification | Overlay 类型包含 notification，但没有 Notification Center/toast/alerts/status entry 的实现 | 缺失 |
| 扩展贡献点 | Rust canonical Manifest、protocol DTO、Tauri wire DTO 和 TS mirror 已统一 P0 枚举与 scope；renderer registry 仍只投影已实现的 View/Command/Menu/Dialog/AppSurface 子集 | 契约已对齐，运行时仍部分完成 |
| Extension Host | 已有手动 activate、进程边界、失败隔离；事件索引、统一 typed RPC、ExtensionContext 和多约束策略仍待实现 | 部分完成；契约已决定 |
| 生态运维 | 本地扫描、安装、升级、卸载、恢复日志；没有 profile/trust/依赖/签名/marketplace/update channel | 部分完成 |

## 详细差距

### 1. Workbench Parts 与布局

**已经有的能力**

- `apps/desktop/src/workbench/parts/part-registry.ts` 与 `crates/layout-model/src/lib.rs` 定义了 8 个稳定 Part 和尺寸约束。
- Rust 通过 `LayoutService` 处理 revision、stale write、迁移、默认状态和 JSON store；桌面端不另建持久化 owner。
- 可见性、collapse、resize、ViewContainer/View 顺序和移除已有命令和回归测试。
- Activity Bar 的 roving focus、resize separator、focus restore 有测试。

**对照 VS Code 的缺口**

- 没有独立 Banner、Notification Center、Toast、Alerts、Command Center；`OverlayLayer` 只是宿主抽象，不是这些可用的基础设施。
- Activity Bar 只能固定在左侧，不能切换到顶部/底部、右侧或 compact；Primary Sidebar 不能左右切换；Auxiliary Sidebar 没有“始终在另一侧”的位置模型。
- OperationsPanel 只有固定底部网格区域，没有 VS Code 的 top/left/right、alignment（center/justify/left/right）、maximize、内部 panel group 和 Panel↔Sidebar 拖放。
- 所有 `PartDescriptor.movable` 都是 `false`。ViewContainer/View 的命令只支持在兼容 Zone 内排序或显式 move，renderer 没有拖放适配器，也没有键盘 `Move View`/`Move Focused View` 选择目标的 UI。
- 没有 `Reset View Locations`、`Reset All Menus`、按 Part/区域恢复默认的完整命令语义；当前 `reset-layout` 只覆盖 Velvet 现有状态。
- `LayoutState` 只有 `visible/size/collapsed`；缺少 Part 位置、Panel alignment、editor grid、sash 分割、notification position、density、window/auxiliary window 状态。
- 当前持久化文件是单一 `workbench-layout.json`。没有 profile/workspace/session 三个作用域的分离，也没有多窗口布局恢复。

**决策影响**

ADR-0003 已接受“v1 固定 Part docking 和三类语义 Zone”。这不是实现遗漏，而是与 VS Code Custom Layout 的任意拖放、Panel 重定位、浮动窗口相冲突的产品决策。若“完整工作台模板”要求 VS Code 级布局自由度，必须重新打开 ADR-0003；若保留固定 Zone，则应把它写成首期的明确兼容边界，并补齐固定区域内部的 AppSurface/Panel/Notification 基础设施。

### 2. WorkspaceArea/AppSurface：不内置 Editor，但宿主契约仍需完整

根据当前产品边界，WorkspaceArea 不承担 VS Code 式 Editor Part；它是由内置 WorkbenchApp 或产品插件自行渲染的 AppSurface 容器。这样可以让 Asset Manager、Feed Workbench 和后续应用拥有自己的网格、预览、画布、时间线或其他交互模型，不被编辑器 tab/group 模型绑死。

这项选择移除了“Editor host 缺失”这个首期阻断项，但没有移除 WorkspaceArea 的宿主责任。当前仍有以下差距：

- 内置 `trusted` AppSurface 仍返回 fallback，尚未证明 Resource Manager/Feed Workbench 可以通过统一 AppSurface contract 真实渲染。
- `AppSurfaceDescriptor` 只有 trust、entrypoint、bridgeVersion 等入口信息，没有完整的 host API/capability handshake、版本协商、最小尺寸、输入法/拖放/下载策略和资源配额。
- isolated surface 已有 mount/activate/suspend/dispose，但 trusted surface、isolated surface 和未来 renderer adapter 还没有一套相同的生命周期与错误 DTO。
- active app 切换时的 context、theme、size、focus、selection、handoff 传播已有切片，但没有明确的顺序、丢失事件补偿、重连和版本迁移验收。
- AppSurface 可以自行管理内部布局，但不能拥有 Workbench Part、全局 layout、核心 Asset/Feed 数据或跨 app 状态；这条 ownership 规则需要进入 manifest、runtime 和测试。
- AppSurface 失败时已有局部 fallback，但缺少统一的加载、禁用、崩溃、重试、超时、权限不足和版本不兼容状态模型。

**预期效果**：WorkspaceArea 成为稳定的 host container；每个应用可以独立选择内部渲染模型，同时得到统一的上下文、主题、尺寸、焦点、命令和失败恢复能力。后续应用不需要重复实现外层布局、插件隔离和生命周期。

### 3. ViewContainer/View 与视图生态

**已经有的能力**

- ViewContainer/View 有稳定 id、title/icon、scope、Zone、preferred Part、order、defaultVisible、enablement、requiredCapabilities。
- ViewRegistry 和 Rust layout 共享可见性/顺序/兼容移动；插件 View 可映射到 isolated UI Bridge。
- 右键菜单提供 hide、up/down、部分 layout actions。

**缺口**

- 没有 VS Code `TreeView`/`WebviewView`/provider/resolve 生命周期，只有 renderer registry 或 fallback 文本。
- 没有 `viewsWelcome`、empty state、badge、view title toolbar、view context menu 分组、view serializer、view state restore、view enablement 的完整契约。
- 没有任意目标容器选择、拖放、跨 View/Panel 的 group 创建；当前 move API 存在，但 renderer 没有完整的目标选择和 DnD adapter。
- 没有 view container 的 location/secondary sidebar/panel 互换模型。
- descriptor 中的 `activation: 'eager'|'on-demand'` 不是 VS Code activation event；没有 `onView:<id>`、onCommand、onStartupFinished 等 event index。

### 4. Command、Keybinding、Menu、Context、Overlay

**已经有的能力**

- CommandRegistry、KeybindingRegistry、MenuRegistry、ContextKeyService、OverlayRegistry 有独立 host owner。
- 已支持 workbench/workbench-app namespace、`when` 基础表达式、命令参数和 layout command。

**缺口**

- ContextKeyService 只解析 `&&`、`!`、`==`、`!=` 四种简单表达式；没有括号/优先级、`in`、正则、集合、数值比较、key set/unset 传播和兼容的表达式语法版本。
- Keybinding 只有字符串归一化和简单优先级，没有完整 chord resolver、默认/用户/工作区层级、冲突诊断、禁用规则、平台 keymap 和 keybinding UI。
- 没有真正的 Command Palette/Quick Input host；`command-palette` 只作为容器枚举。
- MenuRegistry 有 title/context/app-toolbar 等数据结构，但 shell 没有通用 toolbar、overflow、hide/reset menu 和 action enablement renderer。
- OverlayRegistry 的 notification/command-search 类型没有对应的 Notification Center、toast、alerts、quick input UI；dialog 的 focus trap、位置、announcement、超时和恢复需要统一验证。
- 没有稳定的 command/result/error DTO 体系；部分 plugin args/context 仍是 `unknown`/JSON opaque。

### 5. Plugin manifest 与 contribution points

Rust 的 `crates/plugin-api/src/lib.rs` 当前只有 12 个 `ContributionKind`：ActivityBarEntry、CompanionView、OperationView、ViewContainer、View、Dialog、ContextMenu、PreviewProvider、PreviewAugmentation、Inspector、Command、Job。

TypeScript 的 `ContributionRegistry` 另外定义了 title-bar-action、keybinding、notification、settings、app-surface、app-toolbar、app-context-menu、source-connector、feed-monitor、content-kind-provider、processor、asset-facet、extension-model 等种类。它们没有在 Rust 外部 manifest 中形成同一套 schema，导致“本地 TS 能注册”不等于“外部插件能声明”。

对照 VS Code manifest/extension points，缺少：

- 稳定发布身份与展示元数据：publisher/name、displayName、description、categories、icon、README/changelog、activation scope；
- 兼容性：`engines.vscode` 对应的 host API range 之外，没有 manifest schema version、deprecated/feature negotiation、unknown field policy；
- 运行入口：`main`、`browser`、web/Node host 能力、`extensionKind`、virtual workspace/untrusted workspace capabilities；
- 生命周期：`activationEvents`、`activate`/`deactivate`、事件到 extension 的索引、依赖扩展和 activation order；
- 贡献点：languages、grammars、snippets、configuration、configurationDefaults、jsonValidation、themes/colors/iconThemes、localizations、statusBarItems、submenus、customEditors、notebooks、tasks、terminal、debuggers、SCM、testing、webviews 等；
- 生态元数据：extensionDependencies、extensionPack、profile/enablement、安装来源、平台/架构兼容性、完整性/签名信息。

**必须先解决的结构问题**：Rust `plugin-api`、Tauri wire schema、TypeScript `platform/protocol.ts`、PluginClient manifest 和 example manifest 要收敛成一份有版本号的外部契约；不能继续通过两套手工类型让“插件已完成”的状态看起来比真实能力更完整。

### 6. Plugin Host、Extension Host 与 RPC

**已经有的能力**

- `PluginHost` 能注册、激活、禁用、健康检查、恢复、命令调用、安装、升级、回滚、卸载和恢复日志。
- sidecar/WASM 启动通过 `RuntimeLaunchSpec`，进程崩溃只移除该扩展的贡献；JSONL 调用和回归 fixture 可运行。
- UI Bridge 支持 iframe/Tauri child WebView、mount/activate/suspend/dispose、theme/context/size/focus 和失败重试。

**缺口**

- `ApplicationCore::activate_registered_plugins` 会把所有已发现且未恢复中的插件批量 activate；没有以 activation event 为索引的真正懒激活。当前 `activation` 字段只是 View descriptor 的 eager/on-demand 提示。
- 没有 VS Code 的 local Node、web Worker、remote Node 三类 extension host 选择与 `main`/`browser`/`extensionKind` 路由。
- 没有 main thread/extension host 的 RPC customer、proxy id、DTO、取消、超时、重连和宿主重启协议；当前 JSONL 只覆盖命令调用。
- 没有 ExtensionContext、global/workspace storage、SecretStorage、configuration、URI、workspace.fs、network/auth capability handles；权限目前主要被展平为运行环境元数据，而不是可调用的最小能力句柄。
- Manifest 现在已经声明 `activationEvents`、`extensionDependencies`、`support`、required/optional capabilities 和 localization 元数据，并由 Rust 校验 schema、id、scope、namespace 与运行时约束；真正的事件索引、依赖拓扑激活、profile/trust 策略和 capability handle 仍未接入。
- BuiltIn runtime 使用 no-op process，尚未证明内置扩展和第三方扩展使用同一套完整 public contribution/API contract。

#### Plugin localization gap

VS Code 将 Workbench 显示语言包、Manifest 贡献项本地化和扩展运行时文本分开处理；Velvet 当前只有零散的 `titleKey`/`labelKey` 字段和 Surface 的 locale 输入，没有一条插件可消费的完整本地化端口。具体缺口是：

- Manifest 没有 `localization` 声明，无法校验 `defaultLocale`、支持的 BCP-47 locale、bundle schema 和包内资源根目录；
- 没有 `l10n/<locale>.json` bundle 的严格解析、参数形状校验、路径隔离、缓存和版本迁移；
- Manifest 标签、Preview/Job/Overlay 标签与插件运行时字符串没有统一的 namespaced message id 和 `l10n.t(id, defaultMessage, args)` 语义；
- trusted UI 与 isolated UI Bridge 没有统一的 locale/bundle/message-resolution 协议和 locale revision；
- 没有 exact locale → language subtag → extension defaultLocale → defaultMessage 的确定性回退，也没有缺失翻译、坏 bundle、占位符不匹配和旧 session 更新的故障夹具。

该边界已在 ADR-0005/0006 和 Wayfinder 工单 09 中确定：实现应把坏 bundle 或单条消息隔离在扩展范围内，保留 fallback 文本和 Workbench 可用性。ADR-0006 仍为 Proposed；当前代码仍只能称为“过程边界切片”，不能把完整 UI SDK、运行时本地化或生态契约说成已交付完成。

### 7. Text/Language capability：可选后续能力，不是 P0 基建

VS Code 官方明确说明核心编辑器不内置具体语言，语言能力由扩展提供；声明式与程序化能力都需要稳定的宿主 API。这个结论只能说明“如果 Velvet 将来承载文本/代码型应用，插件需要一条语言能力接入面”，不能推出当前 editorless Workbench 必须实现 Language 或 LSP。

当前 P0 只需要保留插件国际化、ContentKind/PreviewProvider 和 AppSurface 宿主能力。`LanguageId`、Grammar、Snippet、diagnostics、completion、hover、definition、formatting 和 LSP 都属于可选的 text-language capability pack，等具体消费者出现后再单独立项。

**未来 text-language capability 可能需要的声明式模型**

1. language identity：language id、文件扩展名、文件名、first line、URI scheme、embedded language、selector score；
2. language configuration：comments、brackets、autoClosingPairs、surroundingPairs、wordPattern、indentationRules、onEnterRules、folding markers、electric characters；
3. tokenizer/grammar：TextMate grammar、scope、injection、embedded language、主题 token 映射；
4. snippets：语言/文件范围、变量、占位符、转换、排序和冲突规则；
5. schema/validation：JSON/YAML 等 schema provider、fileMatch、schema resolution/cache；
6. semantic tokens：token type/modifier/scope、legend、增量更新、主题映射和失效。

**未来 text-language capability 可能需要的程序化 provider 模型**

至少需要 selector、provider registry、优先级、取消、超时、dispose、文档版本检查和失败隔离，覆盖：

- completion、hover、signature help；
- definition/declaration/type definition/implementation/references；
- document/workspace symbols、document highlight、rename/prepare rename；
- format document/range/on type、code actions/resolve、code lens；
- folding、selection range、inlay hints、inline values、inline completions、linked editing；
- call hierarchy、type hierarchy、document colors、document links；
- diagnostics、semantic tokens、document paste/drop/edit；
- language status 和 provider health/latency diagnostics。

**LSP 仅在未来选择时补充**

- 当前没有 LSP client/server adapter、initialize/capability negotiation、document sync、workspace folders、diagnostics push、request cancellation、server restart 和 crash recovery；这是 editorless P0 的刻意边界，不是当前交付遗漏。
- 如果未来的 text-oriented `WorkbenchApp` 选择语言服务，需要另行定义 language service 与该 App 自己的文档/资源模型、URI/WorkspaceContext 的连接；不能把它倒灌进通用 Workbench。
- JSON fixture 可以先验证格式解析/Schema/Preview 路径；HTML/CSS/TypeScript/Markdown 等编辑器型语言 fixture 只有在具体文本应用立项后才增加。

**未来 text-language capability 的验收（不属于当前 P0）**

- 一个 `language.json`/manifest 能声明 id、文件关联、configuration、grammar、snippets；
- 宿主能按 selector 选择文档，并提供至少 diagnostics、completion、hover、definition、format 五种 provider 的统一注册/调用/dispose；
- provider 崩溃、超时、版本不一致只影响该语言扩展；
- 至少一个内置 fixture（建议 JSON 或 Markdown）从打开文件到高亮、snippet、诊断、补全、跳转和格式化走通；
- 如果未来选择 LSP 作为 Provider transport，再增加一个独立的 LSP adapter fixture，验证重启、取消和 malformed response；它不能成为 Workbench foundation 的启动或交付依赖。

### 8. 编辑器之外的 VS Code 扩展面

这些能力不是首期都必须实现，但若目标是“后续应用的模板”，至少要有明确的 public seam、disabled/fallback 行为和延期边界：

- **Tasks/Terminal**：task definition/schema、problem matcher/pattern、进度、取消、终端 profile、shell integration、terminal quick fix；
- **Debug**：debugger type、adapter/session、breakpoints、variables、debug visualizer；
- **SCM/Testing**：SCM provider、resource groups、history、test controller/tree/run/cancel/result；
- **插件自定义渲染面**：如果某个应用需要 notebook、时间线、画布或 webview，应由 AppSurface/Plugin UI Bridge 定义 serializer、save/restore 和安全策略；这不是 Workbench 内置 Editor 的首期职责；
- **配置与外观**：configuration schema/defaults、theme/color/icon theme/product icon、status bar item、localization；
- **文件/搜索/工作区**：URI、workspace.fs、file system provider、search provider、multi-root workspace、workspace folders；
- **身份与安全**：secrets、authentication、workspace trust、virtual workspace、network/process/filesystem capability；
- **远程与 Web**：Node/Web host、remote location、web extension 限制、跨宿主异步命令协议。

当前 Velvet 只有 Job、Preview 和 isolated UI surface 的最小切片，不能把这些扩展面归入“插件生态已完成”。

### 9. 持久化、恢复与状态所有权

已有 layout JSON 和 Job JSON store，但完整模板还需要把状态按 owner 与 scope 分开：

- profile：Parts、全局 ViewContainer/Companion/Operation、命令菜单偏好、theme/keymap；
- workspace：active WorkbenchApp、app-scoped views、AppSurface state、filters/saved views；
- session/window：focus、selection、surface restoration hints、window bounds、浮动窗口（若产品启用）；
- extension global/workspace storage、secrets、manifest enablement、provider cache；
- asset/Feed identity、source、facet、preview cache、job result 的 owner、schema、migration、invalidation 和 recovery。

目前 layout schema 只有 1，migration 主要补默认字段；没有 profile/workspace/session 文档、插件存储、secrets、AppSurface state、provider cache、facet store 或跨窗口恢复。

### 10. 质量、可访问性和可运维性

现有测试覆盖 Activity Bar、focus、resize、DOM shell、layout sync、plugin lifecycle/surface/install/recovery 和 jobs，但没有：

- Panel position/alignment/maximize、Secondary Sidebar/Activity Bar positioning；
- Notification Center/toast/alerts、status bar item、toolbar/menu reset；
- View DnD、Tree/Webview/provider、view serializer/welcome；
- Future syntax-highlighting resource format（当前 P0 不纳入）；更丰富的文本能力仍待具体消费者决定；
- manifest schema/activation event/dependency/compatibility；
- workspace trust/virtual workspace/profile/remote/web host；
- property/fuzz/contract tests for protocol migration, unknown fields, stale events, crash/timeout and cancellation。

可访问性还需要把 focus trap、live region、keyboard move、roving tab index、high contrast/theme token、screen reader labels 和 failed surface announcement 纳入可执行验收，而不能只在 Part 上设置 `tabIndex`。

## 按交付目标分组的差距与预期效果

下面按“首期模板交付”分组，每组都列出当前差距和完成后的可观察效果。WorkspaceArea 按 AppSurface 处理，不要求内置 Editor。

### A. Workbench shell 与布局状态

**差距细项**

- Part 只有固定位置和 `visible/size/collapsed`，缺少位置、Panel alignment、density、窗口/辅助窗口和跨窗口恢复模型。
- Activity Bar、Primary Sidebar、Auxiliary Sidebar、OperationsPanel 不能按 VS Code 方式移动、重定位或跨区域拖放。
- 没有独立 Banner、Command Center、Notification Center/Toast/Alerts。
- View/Panel 没有完整 DnD、键盘移动、group 创建和 Reset View Locations 语义。
- profile、workspace、session 三层布局状态没有拆分 owner。

**预期效果**

- 用户可以隐藏、展开、调整尺寸、聚焦、排序和恢复 Workbench 区域，重启后状态可预测。
- 固定 Zone 的产品约束与可支持的移动范围明确写进 schema 和命令，不再让 UI 行为超过 Rust owner 的能力。
- 任何区域、视图或插件被禁用/移除后，布局仍可恢复，未知贡献不会阻塞启动。

### B. AppSurface 与插件渲染宿主

**差距细项**

- trusted AppSurface 仍是 fallback，内置应用尚未通过统一 surface adapter 渲染。
- trusted/isolated surface 的入口、能力握手、生命周期、错误 DTO 和版本协商没有统一模型。
- context/theme/size/focus/selection/handoff 的顺序、重连和丢失事件补偿没有验收。
- 插件内部布局可以自主管理，但 Workbench ownership、跨 app 数据和全局 layout 的边界尚未通过 manifest/schema 强制覆盖。
- 缺少 surface 资源配额、拖放/下载/输入法/权限和 host API 细节。

**预期效果**

- Resource Manager、Feed Workbench 和第三方 AppSurface 可以各自选择网格、画布、预览、时间线等渲染模型。
- 应用切换只改变 surface lifecycle，不破坏共享 WorkspaceContext、布局和核心状态。
- 一个 surface 崩溃、超时、禁用或版本不兼容时，宿主保留 shell 并提供明确的 retry/fallback。

### C. View、Command、Menu、Focus 与 Overlay host

**差距细项**

- ViewRegistry 没有 Tree/Webview provider、welcome/empty、toolbar、badge、serializer 和 view state restore。
- Command Palette/Quick Input 只有枚举，没有完整 UI、参数 DTO、取消和历史状态。
- ContextKey 只支持极简 `&&/!/==/!=`，缺少括号、`in`、正则、集合和表达式版本。
- Keybinding 缺少多 chord、层级配置、冲突诊断、平台差异、禁用规则和编辑 UI。
- Menu 没有通用 toolbar、overflow、hide/reset 和 action enablement renderer。
- Overlay notification 类型没有对应的 toast、center、alerts、live-region 和完整 focus trap。

**预期效果**

- 内置和插件 View 使用同一套 provider、空状态、工具栏、菜单、badge 和恢复路径。
- 用户可以通过鼠标和键盘完成 View/Container 的显示、隐藏、移动、排序和聚焦。
- 命令、快捷键、菜单、上下文和 Overlay 对同一份 context snapshot 做一致解析，冲突与失败可诊断。

### D. Plugin manifest、贡献点与 Extension Host

**差距细项**

- Canonical v1 已统一 Rust `plugin-api`、`workbench-protocol` DTO、Tauri wire、TypeScript mirror 和 example manifest 的字段、scope、namespace 与 P0 contribution kinds；renderer 仍只对已实现的宿主 registry 做行为投影。
- Manifest 已声明并校验 activation events、依赖、extension kind、target app、trust/profile/virtual-workspace 支持和 required/optional capabilities，但事件索引、依赖图激活、host location 与 profile/trust 策略尚未接入。
- 缺少 ExtensionContext、global/workspace storage、SecretStorage、URI/fs、configuration 和 capability handles。
- 当前激活主要是手动/批量 activate，不是事件索引的 lazy activation。
- JSONL 主要覆盖命令调用，没有完整 RPC/DTO、取消、超时、重连、宿主重启和 provider bridge。
- 没有统一声明 languages、grammars、snippets、themes、settings、custom editors、tasks、terminal、SCM、testing、debug、webview 等扩展面。
- 没有插件本地化声明、`l10n` bundle、namespaced typed message API、locale revision、确定性回退和 bundle/message 级故障隔离。

**预期效果**

- 一个 manifest 可以被 Rust、Tauri、TypeScript 和插件 SDK 以同一 schema 校验和投影。
- 插件只在需要的事件发生时激活，能够声明运行位置、能力、依赖和兼容范围。
- 插件获得窄能力句柄，不能因拥有 UI surface 就获得隐含的文件、网络、进程或 metadata 写权限。
- 插件崩溃、超时、禁用、卸载和升级只清理自身 projection，Workbench 和其他插件继续工作。
- 插件可以用同一套本地化语义为 Manifest 标签和 trusted/isolated UI 提供翻译；locale 切换、缺失翻译和坏 bundle 不会删除贡献或替换 Workbench。

### E. Language、Preview、Job 等可复用能力

**差距细项**

- Text/Language capability 延后：当前只记录 LanguageId 命名空间和未来语法高亮入口；selector、configuration、grammar 资源格式、snippet、schema validation、semantic token、diagnostics/provider registry、LSP 都不属于当前 P0。
- Preview 只有 candidate/resolver/fallback，缺少贡献来源、加载/错误/禁用扩展状态、augmentation lifecycle 和结果缓存策略。
- Job 有提交/取消/重试/中断恢复，但 JobSpec 仍是演示字段，缺少 ownership、输入/输出 schema、进度事件、日志、依赖、并发/配额和结果持久化。
- Capability contributions 在 TS 中有类型草图，但 Rust manifest 不能完整声明 source connector、feed monitor、content-kind provider、processor、facet 等能力。

**预期效果**

- Preview、Job 和 Feed/Asset capability 由插件提供，不要求插件拥有 shell 或 WorkspaceArea 外层布局。未来 Text/Language capability 也应遵守同一边界，但不阻断 P0。
- provider、previewer 或 job 失败时，只隔离该能力并保留 generic fallback、历史结果和可恢复状态。
- 后续应用可以组合共享能力，而不复制语言、预览、任务和缓存逻辑。

### F. 持久化、Transport、安全与交付质量

**差距细项**

- Layout/Job 已有 JSON store，但缺 profile/workspace/session/extension storage、secrets、editorless AppSurface state、facet/cache owner。
- Transport 缺 handles、cancellation、event ordering/replay、reconnect、feature negotiation 和统一 typed error taxonomy。
- 没有 workspace trust、virtual workspace、profile enablement、签名/完整性/依赖和更新兼容矩阵。
- 测试没有覆盖 AppSurface contract、manifest compatibility、activation events、Preview provider timeout、transport reconnect 和 a11y。未来 syntax-highlighting capability 的资源/渲染 fixture 另行设计；Language provider/LSP 不阻断当前 foundation。

**预期效果**

- 每个 durable field 和 cache 都有唯一 owner、schema、migration、invalidation 和 recovery。
- Renderer 只消费 projection/event，不直接碰 filesystem/database/plugin runtime。
- corrupted store、stale event、transport 断线、provider 超时和插件崩溃都能恢复到可用 shell。
- “foundation complete” 可以用一张可执行验收表证明，而不是凭页面是否能显示判断。

## 首期“完整工作台骨架”的建议交付定义

建议把“完整”定义为一个可交付的 Workbench foundation，而不是一次复制 VS Code 的全部 59 个 extension points。首期必须让后续 Resource Manager、Feed Workbench、Preview/Job 和插件 AppSurface 共用一套稳定宿主；Text/Language 属于有具体消费者后再接入的可选能力。

### P0：阻断交付的必做项

1. **Shell/Parts**：固定 Zone 决策写清楚；补齐 Notification Center/Toast/Alerts、Panel 内部 group；实现 visibility/collapse/resize/focus/reset 和跨会话恢复。
2. **AppSurface host**：统一 trusted/isolated surface 的入口、能力握手、mount/suspend/resume/dispose、context/theme/size/focus、错误和 fallback；内置应用通过真实 surface adapter 渲染。
3. **View host**：ViewContainer/View 的 provider、empty/welcome、toolbar/context menu、badge、state serializer、on-demand activation、hide/show/move/reorder/dispose。
4. **Command seam**：版本化 command/menu/keybinding/context expression/command palette DTO；冲突、禁用、取消、错误和焦点恢复。
5. **Plugin contract**：Rust/TS/Tauri/external manifest 合一；schema version、host API range、unknown field policy、runtime entrypoint、activation events、capability、dependency 和 typed errors。
6. **Plugin runtime**：event-indexed lazy activation、可替换 host seam、capability handles、ExtensionContext/storage/secrets、RPC/DTO、timeout/cancellation、crash/restart。
7. **Plugin localization**：`manifest.localization`、包内 `l10n/<locale>.json`、namespaced typed `l10n`、Profile/Session locale revision、确定性 fallback 和 trusted/isolated UI Bridge 统一语义；坏 bundle 与缺失消息按扩展/消息隔离。
8. **Preview/Content capability**：ContentKind、PreviewProvider、PreviewAugmentation、host-owned preview surface、loading/error/disabled/fallback 和 Job handoff 形成最小闭环；不要求 WorkspaceArea 内置 Editor。
9. **Transport/state**：request/response/event/handle/error/cancellation/version/feature negotiation；layout/app/workspace/profile/session/extension state 的 owner、migration、recovery。
10. **Failure isolation**：扩展 disabled/crashed/incompatible/uninstalled、surface/preview-provider timeout、malformed manifest、stale layout 和 corrupted store 都保留核心 Workbench 与 generic fallback 可用；未来 text-language capability 的失败隔离另行设计。
11. **Contract fixtures**：至少一个真实 AppSurface fixture、一个 Preview provider/augmentation fixture、一个 sidecar plugin fixture、一个 failing surface/provider fixture，以及一个 trusted/isolated localization fixture，分别覆盖重启、取消、超时、回退、locale 切换、恢复和卸载；syntax-highlighting fixture 延后到有具体文本型消费者时，Language/LSP 不在当前 fixture 范围。

### P1：首期模板完成后应紧接补齐

- VS Code 式 View/Panel DnD、可移动 Sidebar/Panel、Panel position/alignment/maximize、floating window；
- multi-root workspace 与跨 AppSurface 的 workspace scope；
- tasks/problem matcher/terminal、SCM、testing、debug 的 public seam 和 disabled placeholder；
- 可选 text-language capability：先确认语法高亮资源格式和渲染入口；JSON schema/validation、semantic tokens、embedded language 及 LSP workspace integration 只有在具体文本型应用确认后加入；
- profile、workspace trust、virtual workspace、global/workspace extension storage、secrets；
- Node/Web/remote host location、extensionKind、web extension restrictions、RPC reconnect；
- theme/color/icon、Workbench display-language packs、status bar/notifications/toolbar 完整 API；插件本地化端口已列入 P0，但完整显示语言包仍是后续能力；
- 本地 package integrity/signature/dependency/update/rollback；Marketplace 可以继续延期，但包格式和兼容策略不能含糊。

### P2：生态规模化

- Marketplace/gallery/account/update channel；
- 签名供应链和审核策略；
- remote server 自动安装、跨窗口/跨 profile sync；
- 第三方 SDK 版本治理、deprecation window、proposed/experimental API；
- telemetry、性能预算、extension host profiling 和生态兼容矩阵。

## 推进计划

### Phase 0：先锁边界和合约

- 决定是否保留 ADR-0003 的 fixed Part docking；若要 VS Code 级自由布局，先 supersede ADR-0003。
- ADR-0005/0006 已固定 capability handles、lazy activation、RPC、storage/secrets、AppSurface/Preview seam；当前代码已经完成 Rust `plugin-api` → `workbench-protocol` DTO → Tauri wire → TypeScript mirror 的 canonical v1 对齐，下一步补齐运行时实现和兼容夹具，不要把 LSP 作为 P0 协议依赖。
- 以 Rust `plugin-api` 为 canonical manifest schema，所有边界通过显式 DTO 映射；禁止重新引入直接复用 Rust Manifest 的第二个 transport owner。
- 按已确认的 owner 决策实现 state scopes（profile/workspace/session/extension）和各自 store adapter，并补齐 migration/invalidation/recovery 夹具。

### Phase 1：AppSurface + Preview capability 基础骨架

- 把 WorkspaceArea 的 AppSurface contract 固定下来，先接一个真实内置 AppSurface fixture 和一个 isolated plugin surface fixture。
- 完成 `PreviewHost`/`PreviewResolver`/`PreviewProvider`/`PreviewAugmentation` 的宿主契约，先接一个内置图片或文件 provider、一个插件格式 provider 和一个 augmentation fixture；Quick 可以落在基础预览弹框上。
- 不在 P0 新增 `language-core` 或 LSP adapter。只有具体的文本/代码型 `WorkbenchApp` 出现后，才另开 text-language capability map，复用已确认的 LanguageId 命名空间，并重新决定高亮资源 fixture。

### Phase 2：Workbench chrome 与 View host

- 补齐 Panel group、Notification Center/Toast/Alerts、Command Palette/Quick Input、status bar item。
- 完成 View provider/empty/welcome/toolbar/menu/badge/serializer；将 DnD/keyboard move 作为 renderer adapter 接到已有 typed layout commands。
- 扩展 layout schema/migration，加入 AppSurface/notification/panel state 和 reset commands。

### Phase 3：统一 Plugin/Extension Host

- 按 ADR-0005 的 canonical manifest 实现 activation event index、dependency graph、lazy start、host selection、ExtensionContext、storage/secrets 和 capability handles。
- 实现 versioned typed RPC/DTO、cancellation/timeout/restart、provider/AppSurface/language contribution mapping，并以 trusted、sidecar、WASM 三种 adapter 复用同一公共贡献路径。
- 实现 plugin localization：Manifest `localization` 校验、`l10n` bundle loader/cache、namespaced typed message resolution、fallback/argument validation、Profile/Session locale revision，以及 trusted/isolated UI Bridge 的同语义投影。
- sidecar、WASM、built-in 走同一公共贡献路径；外部扩展失败时只移除自己的 projections。

### Phase 4：应用和生态接入

- Resource Manager 与 Feed Workbench 通过 AppSurface/View/WorkspaceContext 真实消费模板，不再只显示 fallback surface。
- 提供 preview、job、AppSurface 的最小示例扩展；text-language 示例只有在具体文本型应用确认后加入。
- 加入 profile/trust/virtual workspace 和本地 package update/rollback 的兼容矩阵。

### Phase 5：交付前硬化

- 增加 contract/migration/property tests、DOM/a11y tests、crash/timeout/restart/transport reconnect tests。
- 运行 Rust workspace、desktop typecheck/test、clippy、fixture e2e；记录缺失工具和平台限制。
- 更新 README、foundation plan、ADR、GLOSSARY 和 CHANGELOG，让“foundation complete”的声明只在验收表全部通过后出现。

## 需要明确的判断

- 如果目标是“Velvet 的可复用 Workbench 模板”，固定语义 Zone 可以保留，但 AppSurface host、Preview/Content capability、Notifications、View provider、插件国际化和统一 Plugin contract 不能继续延期；View/Command/Overlay 的宿主边界现已确定，剩余是实现和验收；Editor、文本 provider 和 LSP capability 可以等待具体消费者，当前 Language 只保留未来高亮入口记录。
- 如果目标是“兼容 VS Code 扩展”，则必须采用更接近 VS Code 的 manifest、activation、Extension Host、Editor、Language、Web/Remote 和 contribution point 语义；当前固定 Zone 及自定义 permission schema 不足以承诺兼容。但这属于另一条产品路线，不是当前 WorkspaceArea 设计的首期目标。
- 当前 README 的“Workbench foundation release”可以描述已经交付的 shell/runtime slice，但不应被解释成“VS Code 基础骨架和插件生态已完成”。
