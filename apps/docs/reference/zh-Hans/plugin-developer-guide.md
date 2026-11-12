---
id: plugin-developer-guide
title: Velvet 插件开发者指南
---

# Velvet 插件开发者指南（Alpha 契约）

<span id="velvet-插件开发者指南" className="velvet-doc-anchor" />

本指南带领第三方开发者从本地插件包开始，运行当前的测试夹具宿主。文中介绍 Asset Inspector 示例所验证的公开契约，并标明哪些能力仅存在于测试夹具中，哪些仍需通过发布资格验证。这并不意味着 Marketplace、生产 SDK 包或不受限制的进程内运行时已经可用。

## Alpha 契约冻结范围 {/* #alpha-contract-freeze */}

仓库目前将 manifest schema、标识符与 API 版本范围、宿主拥有的注册表、能力检查、Job/诊断接口以及框架无关的 UI Kit 桥接冻结为仓库内的 Alpha 契约。主版本不匹配、manifest 中出现非保留的未知字段、能力或区域无效、UI 界面不兼容时，系统均拒绝继续；次版本变更只能是可选的增量扩展。

Asset Inspector、Echo sidecar 和 `HostFacade` 都是确定性的测试夹具，因此它们的 `pass (fixture)` 结果证明的是宿主接口，而非生产 WASM/sidecar 运行时。Velvet 目前不发布外部 SDK、Marketplace ABI、签名/撤销服务，也不宣称完成了跨平台 UI 资格验证。只有出现第二个真实消费者，并具备[契约矩阵](/reference/plugin-contract-matrix)列出的生态资格证据后，才能考虑这些能力。完整的升级矩阵见[插件生态资格验证门槛](/reference/plugin-ecosystem-qualification)。

<span id="开发路径" className="velvet-doc-anchor" />
<span id="当前-fixture" className="velvet-doc-anchor" />

## 选择起始测试夹具 {/* #choose-a-starting-fixture */}

- `examples/plugins/asset-inspector/` 展示资源管理器（Resource Manager，也称 Asset Manager）的开发路径。它注册 View、上下文命令、快捷键、预览增强和 Job，并在检查 Asset 指纹后写入命名空间内的 facet。
- `examples/plugins/echo-sidecar/` 展示外部运行时路径。它使用 sidecar JSONL 协议，提供 UI Bridge View 和预览操作，并可故意崩溃，以验证宿主只移除和恢复该扩展。

在仓库根目录运行完整的桌面测试夹具套件：

```sh
npm --prefix apps/desktop test
```

迭代测试夹具时，可仅运行 Asset Inspector 演示：

```sh
npm --prefix apps/desktop run test:compile
node apps/desktop/test/asset-inspector-demo.test.mjs
```

也可以直接运行 sidecar：

```sh
printf '%s\n' '{"protocolVersion":1,"kind":"invoke-command","extensionId":"velvet.example.echo","commandId":"echo.open","args":{"source":"manual"},"context":{"activeAppId":null,"keys":{}}}' \
  | node examples/plugins/echo-sidecar/sidecar.mjs
```

## 插件包布局 {/* #package-layout */}

插件包内容应支持移动位置，并可通过 manifest 定位：

```text
my-extension/
  manifest.json
  README.md
  src/                 # 受信任的测试夹具或未来的隔离运行入口
  ui/                  # 可选的 UI Bridge/静态界面资源
  l10n/                # 可选的、已声明的本地化资源包
  test/                # 宿主契约测试夹具；绝不是运行时能力
  assets/              # 宿主契约允许时使用的包内资源
  dist/                # 可选构建输出，以目录或 ZIP 形式提交
```

宿主在应用数据的插件目录下发现已提交的插件包。相对启动路径以已提交的包根目录为基准解析；规范化后超出根目录的路径会被拒绝。插件包不得依赖绝对可执行文件路径、未声明的宿主命令、原始文件系统路径或 Tauri 能力。

`ui/`、`l10n/`、`test/` 和 `dist/` 是包布局惯例，并不隐含任何权限。只有 manifest 贡献声明了界面或语言资源，且宿主通过入口/资源允许列表校验后，它们才会可见。当前示例刻意保持精简，省略了可选目录。

## Manifest、ID 与兼容性 {/* #manifest-ids-and-compatibility */}

先阅读 [Plugin manifest v1](/reference/plugin-manifest-v1)。当前 manifest 的各个兼容性维度相互独立：

| 字段 | 规则 |
| --- | --- |
| `id` | 稳定的扩展身份，升级时保持不变。 |
| 贡献的 `id` | 在 manifest 内保持稳定，并在该扩展内唯一。 |
| `manifestSchema` | 当前规范 schema 为 `1`。除顶层保留的 `x-*` 元数据外，未知字段一律拒绝。 |
| `hostApi` | 宿主契约的版本范围。主版本不匹配时禁止激活。 |
| `bridgeApi` | UI Bridge 契约的版本范围。不兼容的界面会进入失败状态并被释放。 |
| `runtimeProtocol` | 外部运行器使用的运行时信封版本范围。 |
| `targetApp`、贡献的 `scope` 和 `zone` | 明确目标 WorkbenchApp 和贡献作用域；UI 放置使用声明的语义区域。放置位置由宿主拥有。 |
| `requiredCapabilities` / `optionalCapabilities` | 仅声明所需能力；实际授权由宿主策略决定。 |
| `storage` | 声明扩展命名空间内的 schema 和配额，不暴露原始路径。 |

建议以扩展 ID 作为贡献 ID 的前缀；这是命名惯例，不是前缀校验规则。facet 应保存在扩展拥有的命名空间中。不要用用户路径、Asset 路径或 React 组件名作为身份。

## 激活与生命周期 {/* #activation-and-lifecycle */}

激活事件是选择器，不是权限授权。宿主在启动扩展前校验 manifest、目标应用、区域、能力和依赖顺序。激活通过宿主注册表登记声明式贡献。停用、禁用、运行时崩溃、回滚和卸载都必须释放该扩展拥有的所有贡献。

投影中的 `ExtensionState` 值为 `registered`、`starting`、`active`、`disabled`、`failed` 和 `quarantined`。发现和校验发生在注册之前；恢复和卸载是操作，不是额外的状态：

```text
registered → starting → active
                         ├─ disabled
                         ├─ failed
                         └─ quarantined
failed / registered → recover → starting → active
uninstall → extension absent
```

`recover-plugin` 接受 failed 或 registered 扩展；对 active 扩展不执行额外操作。它拒绝 starting、disabled 和 quarantined 状态，因此不是启用命令。manifest 事件匹配目前通过宿主内部的 `activation_candidates` 和 `activate_for_event` 接口完成，尚未接入完整的 CoreTransport 事件总线。

Rust 接受注册后，渲染器才接收投影。渲染器可以缓存描述符，但不能通过修改本地注册表激活一个非活动扩展。

## 安装、运行、升级与卸载 {/* #install-run-upgrade-and-uninstall */}

当前 Extensions 对话框与传输层使用相同的类型化操作序列。开发者无需调用私有 Tauri API 即可完成流程：

1. 选择 **Browse directory…** 或 **Browse .zip…**，然后选择插件包。
2. 宿主发送 `inspect-plugin` 或 `inspect-plugin-archive`。在更改已安装的包之前，它先解析 manifest，并返回扩展 ID、版本、目标应用、宿主 API 兼容性、运行时和请求的权限。不兼容或格式错误的包会在这里被拦截。
3. 检查预览。宿主随后通过 `install-plugin` 或 `install-plugin-archive` 暂存插件包。暂存期间，旧的已提交插件包和扩展数据仍然可用。
4. 安装响应使插件包保持 registered 状态。Extensions UI 随后显式发送 `activate-plugin`，投影其贡献，并且只在激活和 UI 投影成功后通过 `commit-plugin-install` 提交。投影、激活或运行时失败会触发 `rollback-plugin-install`，移除新装插件包或恢复旧包，同时保持其他扩展活动。
5. 升级使用相同的扩展 ID 和更高的数字版本。宿主报告 `operation: upgrade`；旧插件包和数据保留到新版本通过健康/激活检查并提交之后。除非显式恢复路径允许，否则包策略会拒绝降级。
6. 禁用/恢复操作使用宿主拥有的 `disable-plugin` 和 `recover-plugin` 命令。禁用会移除扩展贡献并取消它拥有的 Job；恢复 failed 或 registered 扩展时，会启动新的运行时并重新投影已校验的 manifest。
7. 确认后，**Uninstall** 发送 `uninstall-plugin`，停止运行时、移除已安装的包并注销投影。扩展数据默认保留，以便迁移、重装或由用户显式清理；绝不隐式删除外部 Asset 来源。

具体的 Rust 和 TypeScript 接口位于 `crates/workbench-protocol/src/protocol.rs`、`crates/application-core/src/composition.rs` 和 `apps/desktop/src/workbench/plugins/plugin-client.ts`。可重复运行的升级、回滚和卸载测试夹具见[契约矩阵](/reference/plugin-contract-matrix)。

## 贡献规则 {/* #contribution-rules */}

在 manifest 中声明命令、菜单项、快捷键、View、预览提供者/增强或 Job，由宿主创建注册。宿主拥有活动栏条目、窗格布局、PreviewHost 主预览选择、操作面板、全局覆盖层和全局快捷键。需要放置 UI 的贡献应声明语义区域（`navigation`、`contextual` 或 `operations`），并留在该区域内。

插件可以通过 UI Bridge 为已注册的 View 提供内容，也可以使用宿主语义令牌。它不能注入任意 DOM 或 CSS、调用其他 View 的实现、安装全局按键处理器，或替代布局的所有者。

## 测试夹具 HostFacade API 目录 {/* #fixture-hostfacade-api-catalog */}

目前没有生产级的 `HostFacade` Rust 或 TypeScript 类型。下表描述 `examples/plugins/asset-inspector/src/asset-inspector.mjs` 使用的结构化模拟宿主，帮助开发者理解预期的所有权边界；它不是稳定的 SDK。

模拟宿主记录注册和调用，在内存中保存测试数据，并模拟部分校验和失败情形。最后一列描述真实 PluginHost/注册表/传输层必须执行的宿主策略，不能据此认为模拟宿主可替代真实宿主。具体断言位于 `apps/desktop/test/asset-inspector-demo.test.mjs`；`node examples/plugins/asset-inspector/run-demo.mjs` 可以在不启动桌面渲染器的情况下运行同一个包。

| 测试夹具句柄 | 操作 | 宿主所有者 | 插件可读写的内容 | 能力与失败规则 |
| --- | --- | --- | --- | --- |
| `commands.register(id, handler)` | 注册命令回调 | 命令注册表 / PluginHost | 注册和释放自己的命令 | 需要 manifest 命令声明及声明的作用域/命名空间；宿主必须拒绝重复或未声明的 ID，模拟注册辅助函数不校验它们。 |
| `menus.register(id)` | 注册上下文菜单贡献 | 菜单注册表 / PluginHost | 注册和释放自己的菜单项 | 菜单位置和 `when` 表达式由宿主校验。 |
| `keybindings.register(id)` | 注册快捷键 | 快捷键注册表 / 宿主 | 注册和释放自己的快捷键 | 宿主解决冲突，并保留全局快捷键的所有权。 |
| `views.register(id)` | 注册 View 描述符 | View 注册表 / 工作台 | 注册声明式 View | 区域、容器和命名空间由宿主拥有，不暴露其他 View 的句柄。 |
| `preview.registerAugmentation(id, handler)` | 注册预览增强 | PreviewResolver / PreviewHost | 读取有界预览上下文并返回增强内容 | 提供者失败会转为类型化的回退/诊断；不能替代核心 Asset 状态。 |
| `assets.readProjection(id)` | 读取不含路径的 Asset 投影 | Asset 来源所有者 / ApplicationCore | 读取所选身份、内容类型和指纹 | 需要 `asset.read`；数据库、来源路径和原始文件句柄都不能跨越该接口。 |
| `jobs.create(request)` / `jobs.wait(id)` / `jobs.cancel(id)` | 提交、观察和取消 Job | `job-system` | 提交有界输入，读取进度/结果，取消自己拥有的 Job | 需要 `jobs.create`/`jobs.read`；取消、截止时间、过期来源信息（provenance）和重试由宿主策略控制。取消限制在所有者范围内，Rust `JobSystem` 始终是权威来源。 |
| `facets.read({namespace, assetId})` | 读取扩展派生数据 | PluginHost 扩展存储/facet 所有者 | 读取自己命名空间中的 facet | 命名空间由宿主推导，不能访问其他扩展的 facet。 |
| `facets.write(record)` | 写入派生 facet 数据 | PluginHost 与 Asset 来源信息所有者 | 写入带 schema 和预期指纹的有界值 | 需要 `asset.facet.write`；过期指纹会被拒绝，核心 Asset 身份保持不变。 |
| `configuration.version()` | 读取配置修订版本 | Profile/配置所有者 | 读取用于 Job 来源信息的版本令牌 | 此测试夹具不提供任意配置写入。未来若需写入，必须使用有版本的命令和能力。 |
| `storage.read(key)` / `storage.write(key, value)` / `storage.migrate(from, to)` | 读写并迁移扩展拥有的存储 | PluginHost 存储所有者 | 仅使用插件命名空间及声明的配额 | `storage.read`/`storage.write` 需经过能力检查；迁移必须有界且具有事务性，失败时上一代仍为权威数据。 |
| `ui.register(surface)` / `ui.render(request)` | 协商并投影 UI Kit 组件 | UI Bridge / 工作台 | 在插件 View 上仅使用已声明的语义组件 | 激活前协商 UI Kit/API 和组件契约版本；宿主拥有 DOM、焦点、布局和覆盖层状态。 |
| `diagnostics.log(record)` | 发出结构化诊断 | PluginDiagnosticSink / ApplicationCore | 发送有界且经过脱敏的记录 | 宿主限制大小并脱敏；日志不能包含来源路径、秘密或 Asset 载荷。 |
| `notifications.show(record)` | 请求用户通知 | 宿主通知界面 | 请求具有类型化严重级别/消息的通知 | 宿主拥有呈现、本地化和速率限制；插件不能写入全局 UI 状态。 |

这些句柄刻意不暴露路径。生产扩展应通过已接受的运行时/桥接协议获得按能力限制的句柄，而不是原始 Rust 对象或框架组件。

## Asset、Preview 与 Job 数据 {/* #asset-preview-and-job-data */}

分析扩展应默认遵循 Asset Inspector 模式：

1. 读取不含路径的 Asset 投影。
2. 在有界 Job 输入中包含选择修订版本、Asset 指纹和配置版本；测试夹具还在来源信息中记录所有者/提供者身份、提供者版本、组合输入指纹和 schema 版本。
3. 通过宿主 Job 句柄提交工作；生产环境中的所有者是 Rust JobSystem，测试夹具则在内存中模拟它。
4. 拒绝或丢弃指纹已过期的结果。
5. 只写入命名空间内、带 schema 版本的 facet。
6. 通过预览增强或上下文 View 呈现 facet。

扩展不得自行打开来源路径、持久化第二份 Asset 记录，或创建渲染器拥有的后台循环。扩展被禁用或失败时，通用预览回退和核心 Asset 身份仍须可用。

<span id="ui-选择" className="velvet-doc-anchor" />

## UI Bridge 与 UI Kit {/* #ui-bridge-and-ui-kit */}

当前桥接 schema、语义令牌允许列表、来源检查和组件协商见 [UI Kit 契约 v1](/reference/ui-kit-contract-v1)。宿主提供布局容器、焦点恢复、覆盖层、全局菜单和快捷键。插件界面可以渲染与令牌兼容的内容，并使用已注册命令、焦点请求和尺寸调整请求。Asset Inspector 示例通过 UI Kit 适配器使用公开的 `progress`、`table`、`loading-state`、`empty-state` 和 `error-state` 组件名；它不导入 React，也不注入 DOM。

桥接分别维护 `bridgeApi`、组件契约和主题 schema 版本。主题变化通过宿主拥有的快照传递；当前测试夹具覆盖令牌过滤和 `host.theme` 更新。键盘焦点和 ARIA 的所有权仍属于宿主工作台，DOM 回归测试夹具覆盖标签和焦点恢复。

不要依赖 React 类型、宿主 CSS 类、DOM 选择器、任意 URL 或其他 View 的实现。当前 data/blob/builtin 界面示例是测试夹具的便利机制；生产界面仍需遵守宿主的来源、CSP 和能力策略。

<span id="失败隔离" className="velvet-doc-anchor" />

## 权限、错误与恢复 {/* #permissions-errors-and-recovery */}

Manifest 权限是请求。有效授权由宿主计算，并受区域、目标应用、Profile 信任和平台策略进一步限制。对于版本不兼容、能力被拒绝、贡献无效、取消、超时、输入过期、运行时输出格式错误和存储不可用，宿主会报告类型化错误。

扩展失败时应当：

- 在收到取消或禁用后停止安排新工作。
- 提供有界诊断，以及面向用户的重试/禁用操作。
- 保持核心 Asset、布局、预览回退和其他扩展可用。
- 保留或迁移自己命名空间中的数据，不隐式删除用户来源。
- 将 `unknown` Job 结果视为需要核对权威状态的情形，而不是静默重试非幂等操作。

宿主拥有的诊断流程是具体的本地流程：

1. 打开操作面板，查询插件安装/恢复日志。可按扩展、状态或操作筛选；相同的有界记录也会出现在全局诊断投影中。
2. 使用面板的 **Export** 操作，经 CoreTransport 和原生保存对话框调用类型化的 `export-plugin-install-recovery-log` 命令。用户主动发起的更广范围 `ExportRecoveryDiagnostics` 诊断包可以包含有界插件诊断历史，但绝不包含原始 Asset 内容、凭据或不受限制的路径。
3. 运行时失败时，选择宿主提供的 **Recover**、**Disable** 或重试。对于暂存中的安装/升级，可选择继续、保留新版本、恢复旧版本或丢弃；宿主使用相应的类型化恢复命令，并在成功提交前保留旧的已提交包。
4. 问题仍未解决时，保留有界诊断记录，并在报告问题时附上测试夹具/测试结果。不要手工删除包目录或扩展数据来强行恢复。

## 本地验证清单 {/* #local-verification-checklist */}

分享测试夹具前，运行[插件契约矩阵](/reference/plugin-contract-matrix)中的命令。迭代期间，本指南开头的夹具专用命令足以支持快速验证。将每项新贡献对应到矩阵，并在其所属接口增加正向和负向测试夹具。测试夹具不能替代签名、Marketplace 发布、公证或每个目标 OS 的生产资格验证。

## 当前限制 {/* #current-limits */}

- 公开 manifest 和协议已带版本，但没有单独发布第三方 SDK 包。
- facet schema 由所有者管理版本并绑定指纹；当前宿主没有独立的全局 facet 协商注册表。
- `RuntimeKind::Wasm` 和 `RuntimeKind::Sidecar` 是契约声明；sidecar 测试夹具用于验证隔离，不是安全沙箱。
- 发布者信任、撤销、Marketplace 策略和发布签名仍需通过发布门槛。
- 第二个客户端或独立版本的 SDK 出现后，才有理由新增 ADR 并提取独立包。在此之前，应保持当前仓库内协议镜像的映射明确。
