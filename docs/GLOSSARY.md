# Glossary

## Asset

一个可被工作台识别、索引、预览或组织的用户素材实体。Asset 的身份与它当前所在的文件路径分开。

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

## WorkspaceContext

由核心和工作台提供给多个 WorkbenchApp 的共享工作上下文，例如当前数据范围、选中的 Asset、Collection、查询和筛选条件。它是跨应用的只读投影和事件来源，不是某个应用的私有数据库。

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

## PreviewProvider

为某类素材和预览意图提供主预览内容的扩展能力。

## PreviewAugmentation

附着在主预览或检查器上的扩展能力，例如元数据、直方图或材质信息。

## PreviewIntent

用户打开预览时的目标层级：Quick、Inspect 或 Analyze。

## Job

可观察、可取消、可重试的异步工作，例如缩略图生成、索引或分析。

## Product extension

向工作台贡献视图、预览、命令、检查器或任务的扩展。它与提供桌面系统能力的 Tauri plugin 分开定义。

## Workbench

应用内部承载工作区、布局、视图、预览和命令的完整工作台。
