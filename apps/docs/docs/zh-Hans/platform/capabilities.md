---
id: capabilities
title: 能力目录
---

# 能力目录

从这里查找已有边界。**Alpha** 表示兼容性承诺；**fixture、宿主实现、发布门禁** 表示不同的实现证据。这里没有任何条目代表已经发布的 SDK 包。

| 任务 | 声明或调用 | Owner 与必要边界 | 当前路径 |
| --- | --- | --- | --- |
| 执行操作 | `command`、`menu`、`context-menu`、`keybinding`；`invoke-command` | PluginHost 与 command registry 校验身份、上下文和权限 | [Commands 与 Views](../guides/commands-and-views)；Echo sidecar fixture |
| 增加 View | `view-container`、`view`、`companion-view`、`operation-view` | Workbench 拥有容器、scope 和 zone | Echo manifest 与 [UI Bridge](./ui-bridge) |
| 增加产品应用 | `workbenchApp`、`app-surface`、`activity-bar-entry` | WorkbenchApp registry 拥有激活和 surface 生命周期 | 宿主契约与生命周期测试；没有独立 SDK starter |
| 提供预览 | `preview-provider`、`preview-augmentation`、`inspector` | PreviewHost 拥有选择和 fallback；权限策略由宿主执行 | Echo preview fixture 与 [Asset 分析](../guides/asset-analysis) |
| 执行后台工作 | `job`；`submit-job`、`query-job`、`cancel-job` | Rust JobSystem；每次调用检查 caller 所有权和能力 | [宿主服务](/reference/plugin-api-index)；本地契约测试 |
| 读取 Asset 或写入 facet | fixture `assets.readProjection`、`facets.read`、`facets.write` | 核心拥有 Asset 身份；写入要求命名空间、schema 和最新指纹 | 仅 Asset Inspector fixture；这些方法名不是已发布的 SDK API |
| 保存扩展数据 | manifest `storage`；fixture `storage.read/write/migrate` | PluginHost ExtensionStorage、配额和事务迁移 | 宿主存储实现；便捷 facade 仍只存在于 fixture |
| 读取配置 | fixture `configuration.version()` | Profile/configuration owner；不提供任意配置修改 API | 仅 Asset Inspector fixture |
| 本地化扩展 | manifest `localization`；`host.locale` | 宿主 bundle loader 和 locale 投影；有界包资源 | 宿主本地化实现与 Bridge 契约 |
| 诊断或请求交互 | `log`、`notify`、`confirm`；Bridge 通知和 overlay 消息 | 宿主限量、脱敏并拥有 UI；没有 adapter 时确认默认拒绝 | 宿主请求契约与 fixture 证据 |
| 渲染自定义 UI | contribution `ui`；`surface.*` / `host.*` 消息 | 宿主拥有来源策略、焦点、布局和销毁 | [隔离 UI](../guides/isolated-ui)；Alpha adapter |

Manifest 声明仅请求能力，不授予能力。具体权限和失败行为见 [API 索引](/reference/plugin-api-index)及[安全指南](./security)。运行时枚举值不代表已经具备安全沙箱。

Schema 和宿主消息见 API 参考。上表 `HostFacade` 方法见明确标注的 [fixture 目录](/reference/plugin-developer-guide#fixture-hostfacade-api-catalog)。生产 SDK 发布仍受[生态门禁](/reference/plugin-ecosystem-qualification)约束。
