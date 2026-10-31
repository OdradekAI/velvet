# Glossary

## Asset

一个可被工作台识别、索引、预览或组织的用户素材实体。Asset 的身份与它当前所在的文件路径分开。

## ContentFingerprint

由 Asset/source owner 提供的完整素材内容指纹。它与 AssetId 分开，用于判断预览、Facet 和 Job 派生结果是否仍对应当前内容；缺少指纹时只能视为未知输入，不能命中缓存。

## ContentKind

素材的内容类别，例如图片、RAW、视频、音频、文档或 3D 模型。它描述素材能使用哪些解析和预览能力。

## Workspace

用户正在使用的一套工作环境，包含当前数据范围、打开的素材、活动的 WorkbenchApp、视图可见性和布局状态。Workspace 是用户上下文，不等同于 WorkbenchApp。

## WorkbenchApp

工作台中可从 ActivityBar 选择的顶层业务应用。它提供一个主工作区界面，并可声明符合语义区域约束的 ViewContainer 和 View；它不拥有工作台布局或核心素材数据。

## Resource Manager

负责管理用户数字资产的 WorkbenchApp。它使用 Asset 作为核心实体，提供组织、预览、检索和处理入口；Resource Manager 不拥有 Asset 的底层身份和来源数据。

## FeedItem

由网络来源、订阅或监控产生的采集记录。FeedItem 保留来源、采集状态和可追溯信息，可以继续留在 Feed Workbench，也可以通过明确操作进入资源管理流程。

## Capture

FeedItem 关联的内容快照、附件或解析结果。Capture 可以在保留来源和采集 provenance 的前提下被 materialize 为 Asset。

## Workbench contribution

插件向 Workbench、WorkbenchApp 或可复用能力注册的扩展能力。Contribution 必须声明目标容器、作用域和所需权限，由宿主负责生命周期和失败隔离。

## Contribution scope

Contribution 的作用范围。`workbench` 表示全局工作台，`workbench-app` 表示一个指定的 WorkbenchApp，`capability` 表示可复用的数据、预览、处理或任务能力。

## Host container

由 Workbench 宿主拥有的注册和渲染位置，例如 ActivityBar、WorkspaceArea、PrimarySidebar、AuxiliarySidebar、OperationsPanel、OverlayLayer 和 ContextMenu。插件声明目标容器，但不拥有容器布局。

## Workbench namespace

插件贡献可见范围的命名空间。全局 Workbench namespace 作用于整个工作台；WorkbenchApp namespace 只在指定的 WorkbenchApp 激活时生效；能力 namespace 用于可复用的数据和处理能力。

## CompanionView

工作台级的常驻辅助视图，通常位于 AuxiliarySidebar，例如 AI 助手、受限浏览器或数据库工具。它跟随共享 WorkspaceContext，但不属于某个 WorkbenchApp 的主界面。

## OperationView

位于 OperationsPanel 中的全局操作视图，例如 Job、日志、历史或同步状态。它读取共享 JobSystem 的投影，不拥有独立的任务状态。

## AppSurface

WorkbenchApp 在 WorkspaceArea 中呈现的主界面。AppSurface 可以拥有自己的导航、标签或面板组织方式，但通过工作台提供的上下文和协议访问共享数据。

## RecoverySurface

由宿主拥有的最小恢复界面。它只在 Workbench 无法安全挂载时出现，用于重试启动、禁用最近失败的可选组件、恢复备份、重置可选状态和导出诊断；它不拥有 Asset、Workspace、Profile 或 Job 的权威数据。WebView 可用时由渲染器呈现；WebView 不可用时由平台宿主提供等价的最小恢复动作。

## RecoverySupervisor

宿主拥有的生命周期恢复边界。它负责记录有限的启动、就绪和退出证据，识别异常退出，并把恢复决定交给 `ApplicationCore` 的 Safe Mode 策略；它不拥有 Asset、Workspace、Profile 或 Job 的权威数据，也不执行网络上传或声称从进程内部监督活进程。P0 使用进程内实现；只有在自动重启成为已验证需求时，才增加外部实现。崩溃证据由独立的 CrashCaptureAdapter 负责。

## CrashCaptureAdapter

负责把平台提供的本地崩溃证据转换为有界、脱敏的诊断记录的宿主适配器。它不决定 Safe Mode、不拥有业务状态、不自动上传数据，也不等同于 RecoverySupervisor；某个平台没有可靠的 native crash capture 时可以只记录退出码、信号和启动 journal。

## ReleaseManifest

描述一个 Velvet 应用发布产物的签名元数据。它绑定发布 channel、版本、目标 OS/architecture、产物哈希、updater schema、支持的数据 schema 范围、最低客户端版本、单调递增的 release sequence、新鲜度信息和 signing key id；客户端在安装前校验签名、目标、版本、新鲜度和兼容性。

## UpdateTransaction

由宿主记录的一次应用更新事务，包含 manifest 校验、下载 staging、待迁移状态、旧 package/data generation 保留和健康 checkpoint。事务在新版本进入 `ready` 前可以成对回退到旧 package 与旧 `StateSnapshot` generation；提交后不允许旧 binary 直接打开新 schema。

## ReleaseChannel

应用安装实例选择的发布通道，例如 `stable` 或 `preview`。通道选择参与更新 manifest 校验，更新不能静默跨通道或降级；alpha 构建使用 `preview`。

## TransportSession

由协议 handshake 建立的前端到 Rust 核心的逻辑会话。`sessionId` 约束事件、revision、sequence 和 opaque handle 的有效范围；重连会创建新会话并重新同步权威快照，旧会话的消息和 handle 必须丢弃。

## PlatformTier

宿主对 OS、WebView 和硬件能力的支持等级。Tier A 是启动、恢复、持久化、输入、无障碍、窗口和 CoreTransport 所需的基础能力；Tier B 是可诊断降级的通知、剪贴板、托盘、快捷键、GPU、深链接和外部卷能力。

## ActivationRequest

由文件关联、拖放、命令行或深链接产生的统一激活请求。平台适配器只负责解码，Rust 负责校验 scheme、路径根、文件类型、能力和幂等性，并在核心 ready 后投递。

## SourceAvailability

Asset source 当前是否可访问的 Rust 投影。移动硬盘或网络盘断开时，AssetId、元数据和 source registration 保留，SourceAvailability 进入 degraded；路径和卷身份重新校验后才恢复 Preview 或 Job。

## DiagnosticBundle

由用户主动导出的诊断包，包含经过脱敏的版本、平台、启动阶段、退出信息、组件身份和有限日志尾部，用于本地排障或在用户单独同意后上传；默认不包含 Asset 内容、凭据、密钥和未脱敏路径。

## WorkspaceContext

由核心和工作台提供给多个 WorkbenchApp 的共享工作上下文，例如当前数据范围、选中的 Asset、Collection、查询和筛选条件。它是跨应用的只读投影和事件来源，不是某个应用的私有数据库。

## Asset source location

由 `application-core` 持有的 AssetId 到应用源文件的持久化注册。注册保存 canonical regular-file location、ContentKind 和可选的 source-owned fingerprint；路径只在 Rust source adapter 内部使用，不通过 Preview、插件运行时或渲染层投影。

## State scope

持久化状态的生命周期范围，包括 Asset/source、Profile、Workspace、Session/Window、Job、Extension 以及宿主生命周期记录。Scope 决定状态由谁拥有、何时恢复以及何时失效。

## StateSnapshot

由某个状态 owner 产生的、带有 scope、schema version、generation、校验和和提交标记的持久化状态代。快照可以独立校验、迁移、恢复或淘汰；它不改变 owner 归属。

## RecoverySet

由恢复索引记录的一组相关 StateSnapshot generation。它表示一次跨 owner 变更或迁移的可恢复提交点；只有在引用校验通过并完成提交屏障后，相关 generation 才能一起成为当前状态。RecoverySet 不取代各 owner 对数据和 schema 的所有权。

## QuarantinedState

因损坏、schema 不兼容或迁移失败而暂时移出正常启动路径的原始状态。QuarantinedState 保留有限诊断元数据和原始字节，等待重试、恢复、导出或用户删除；它不应阻塞核心 Workbench 启动。

## Profile locale

由 Rust Profile state 持有的工作台显示语言和 revision。桌面 LocalizationService 通过 CoreTransport 查询、设置并投影该状态；插件 bundle 只提供扩展自己的消息资源，不拥有全局 locale。

## Plugin resource loader

由 PluginHost 持有的受限资源读取适配器。它根据已注册扩展和声明的 resourceRoot/locale 读取有界 bundle，校验包路径、符号链接和大小后通过 CoreTransport 投影资源 DTO；桌面渲染层不直接访问插件文件系统。

## Plugin archive adapter

由 PluginHost 在安装阶段使用的归档输入适配器。首期支持 ZIP，将归档安全解包到临时目录后交给既有的 manifest 校验、staging、activation、commit 和 rollback 流程；归档格式处理不进入 PluginResourceLoader。

## State owner

对某类持久化字段或派生缓存负责的唯一模块。State owner 负责 schema、迁移、失效、恢复和对外投影；存储适配器不成为第二 owner。

## ResourceBudget

由宿主执行的设备全局、owner 和 Job 三级资源上限与软预算。内存、磁盘、文件描述符、子进程和并发属于硬 admission 边界；CPU 和网络通过优先级、节流、字节计量和 backpressure 治理。具体值由版本化的平台/工作负载策略确定，扩展不能通过 manifest 放宽预算。

## Projection

由宿主或核心从权威状态产生、供渲染器或扩展读取的只读表示。Projection 可以失效并重新同步，但不能反向成为持久化来源。

## Failure isolation

插件、View provider、AppSurface、Preview provider、Job、传输会话或存储文档发生故障时，只影响自己的状态和投影，并保留 Workbench、核心数据和其他能力可用。

## Collection

用户用于组织素材的逻辑集合。Collection 不等同于文件系统目录。

## AssetFacet

附着在 Asset 上的派生或扩展数据，例如 EXIF、直方图、AI 标签或视频波形。Facet 有来源、版本和状态。

## Part

工作台中的稳定空间区域，例如 ActivityBar、PrimarySidebar、WorkspaceArea、AuxiliarySidebar 和 OperationsPanel。

## Zone

工作台区域承担的稳定职能。当前包括 navigation、contextual 和 operations。插件在声明的 Zone 内贡献内容。

## ViewContainer

一组相关 View 的容器，例如素材库、检查器或任务列表。

## View

工作台中可显示、隐藏、排序和聚焦的功能视图。

## View provider

为 View 提供列表、树、检查器、时间线或隔离界面内容的扩展能力。View provider 只负责内容投影和 View 范围状态，不拥有 Part 或全局布局。

## Command

由工作台宿主解析和执行的稳定用户操作。Command 通过命名空间、上下文和权限与菜单、快捷键及插件运行时连接。

## Context expression

描述 Command、View、Menu 或 Overlay 是否可见、可用的版本化条件表达式。它读取宿主提供的上下文快照，不直接执行插件代码。

## Overlay

由宿主管理的浮层表面，例如 Dialog、Popover、Command Search、Quick Input、Toast、Alert 或 Notification Center。宿主负责位置、焦点、模态和恢复。

## PreviewProvider

为某类素材和预览意图提供主预览内容的扩展能力。

## PreviewAugmentation

附着在主预览或检查器上的扩展能力，例如元数据、直方图或材质信息。

## PreviewIntent

用户打开预览时的目标层级：Quick、Inspect 或 Analyze。

## PreviewHost

由工作台拥有的预览宿主。它管理预览 frame、生命周期、焦点、主题、加载/错误/降级状态以及插件扩展入口。

## PreviewSession

一次针对 Asset、ContentKind、PreviewIntent 和目标 surface 的预览会话。会话拥有独立的取消、尺寸、焦点和失效边界。

## Preview cache

由 PreviewProvider 或 capability owner 产生的派生结果缓存。缓存带有输入、provider、版本和设置 provenance，可以失效、重算或独立清理。
首期复用入口是 Rust JobSystem 的完整 provenance 结果索引；具体 provider 或 AssetFacet 需要物化缓存时，必须自行声明存储 owner、保留和清理策略。

## Job

可观察、可取消、可重试的异步工作，例如缩略图生成、索引或分析。Job
由 Rust JobSystem 统一拥有生命周期、Runner、结果和 provenance；Runner 接收
受限 input 并可上报进度，结果带有 schema、content type 和 payload，
provenance 记录 owner、输入、provider 和设置指纹。

## JobCheckpoint

Job 在睡眠、资源压力或进程重启前由 owner 写入的可恢复生命周期快照。它保存 attempt、取消/重试状态和 provenance；派生结果是否可复用仍由完整 provenance 决定。

## Product extension

向工作台贡献视图、预览、命令、检查器或任务的扩展。它与提供桌面系统能力的 Tauri plugin 分开定义。

## ExtensionId

产品扩展的稳定全局身份。它与显示名称、版本和具体运行时分开，并用于拥有扩展命名空间及其贡献。

## ExtensionKind

扩展所提供能力的类别，例如 UI、WorkspaceApp 或可复用 capability。它描述扩展用途，不等同于运行时隔离方式。

## ActivationEvent

触发扩展按需激活的稳定工作台事件，例如 Workbench 启动、应用切换、View、Command、Preview 或 Job 事件。

## ExtensionContext

宿主在一次扩展会话中提供的受限上下文，包括扩展身份、取消、日志、本地化以及经授权的存储、配置、URI 和能力句柄。它不包含宿主框架对象或无限制路径。

## Runtime trust

插件运行时所处的信任层级。它区分应用内可信代码、能力沙箱代码、仅具备进程故障隔离的原生子进程以及显式开发模式代码；运行时形式本身不等于安全信任。

## Sandbox policy

宿主对插件运行时可见资源和可执行操作的约束。它描述能力是否由 WASI、平台沙箱或宿主代理强制执行，并明确哪些边界只提供崩溃隔离而不提供安全隔离。

## ExtensionStorage

由 PluginHost 持有的、按 ExtensionId 命名空间隔离的插件数据。插件可以通过受限句柄读写自己的数据，但不拥有应用数据库、其他 owner 的状态或恢复索引。

## BackupParticipant

插件对自身 ExtensionStorage 提供的备份参与能力。它可以导出有界、可迁移的插件数据表示；宿主决定快照、提交屏障、恢复顺序和是否包含在用户选择的导出中。

## MigrationParticipant

插件对自身数据 schema 提供的前向迁移能力。迁移读取宿主提供的旧代际并写入 staging 代际，经过宿主校验后才发布；迁移失败只隔离插件数据，不重置核心数据。

## PluginTrustRecord

宿主对插件包的信任记录，包含包摘要、发布者身份、签名和撤销状态、兼容范围以及可选的构建 provenance/SBOM 引用。它与付费 entitlement 分开。

## Entitlement

针对某个插件或功能的授权状态，例如 active、trial、grace、expired 或 unknown。Entitlement 表示账户或安装的使用权，不证明插件包完整性，也不包含支付凭据。

## PluginDiagnosticSink

由宿主提供的结构化插件诊断入口。它接收与扩展会话和 Job attempt 关联的有界日志、错误和恢复提示，并按宿主的脱敏、保留和导出策略处理内容。

## CapabilityGrant

宿主根据扩展声明、信任状态和 Profile/Workspace 策略授予的有效能力集合。它可以被允许、提示或拒绝，并独立于 Part 或 Zone。

## Workbench

应用内部承载工作区、布局、视图、预览和命令的完整工作台。

## Language

工作台可识别和处理的文本或代码语言。Language 可以由核心或插件提供，包含稳定身份以及可组合的声明式资源和程序化服务。

## LanguageId

Language 的稳定身份，用于把文档、文件关联、语法资源和语言服务连接起来。LanguageId 不等同于显示名称，也不随插件版本变化。

## Extension localization

插件为自己的命令、视图、设置、预览和运行时界面提供的本地化文本能力。它与工作台显示语言以及面向文档内容的 Language 能力分开管理。

插件通过 Manifest 的 `localization` 声明和包内 `l10n/<locale>.json`
bundle 提供资源。消息身份使用 `<extensionId>.<localMessageId>`；宿主负责
locale 选择、`requested exact locale → language subtag → extension
defaultLocale → defaultMessage` 回退、参数校验、版本化投影和故障隔离。
可信 UI 与隔离 UI Bridge 使用同一语义 `l10n` 服务。
