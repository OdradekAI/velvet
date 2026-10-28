# Glossary

## Asset

一个可被工作台识别、索引、预览或组织的用户素材实体。Asset 的身份与它当前所在的文件路径分开。

## ContentKind

素材的内容类别，例如图片、RAW、视频、音频、文档或 3D 模型。它描述素材能使用哪些解析和预览能力。

## Workspace

用户正在使用的一套工作环境，包含当前活动工作区、打开的素材、视图可见性和布局状态。

## Collection

用户用于组织素材的逻辑集合。Collection 不等同于文件系统目录。

## AssetFacet

附着在 Asset 上的派生或扩展数据，例如 EXIF、直方图、AI 标签或视频波形。Facet 有来源、版本和状态。

## Part

工作台中的稳定空间区域，例如 ActivityBar、PrimarySidebar、WorkspaceArea、ContextSidebar 和 OperationsPanel。

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
