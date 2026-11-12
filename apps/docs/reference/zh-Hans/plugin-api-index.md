---
id: plugin-api-index
title: API 与协议索引
---

# API 与协议索引

读者：产品扩展作者和宿主维护者。状态：**仓库内 Alpha**。此索引映射已有契约 owner，不将所有 Rust 或桌面类型作为公共 SDK 暴露。

## 契约地图 {/* #contract-map */}

| 表面 | Owner / 来源 | 参考与用途 |
| --- | --- | --- |
| Manifest 与贡献描述 | `plugin-api` / `ExtensionManifest`、`ContributionDescriptor` | [Manifest](plugin-manifest-v1.md)；稳定 ID、scope、zone 和请求的能力 |
| 宿主请求、响应与事件 | `workbench-protocol` / `PluginRequest`、`PluginResponse`、`PluginEvent` | 下方宿主服务；[envelope](plugin-manifest-v1.md#plugin-coretransport-envelope) |
| 隔离 UI 消息 | 桌面 `workbench/plugins/ui-bridge.ts` | [Bridge 协议](ui-bridge-protocol-v1.md)；surface 生命周期与类型化投影 |
| 语义主题和组件协商 | 桌面 `workbench/plugins/ui-kit.ts` | [UI 契约](ui-kit-contract-v1.md)；不提供宿主 React props |
| Registry 实现 | `plugin-host` | [维护者 Registry 记录](plugin-registry-v1.md)；不是插件拥有的可变 map |
| Fixture 便捷句柄 | `examples/plugins/asset-inspector/src/fixture-host.mjs` | [Fixture HostFacade](plugin-developer-guide.md#fixture-hostfacade-api-catalog)；不是已发布的 SDK 方法 |

## 宿主服务 {/* #host-services */}

每个请求都面向绑定的 caller 扩展。Adapter 校验 session 和 runtime lease，宿主执行生命周期、能力及所有权策略。各请求 DTO 位于 `crates/workbench-protocol/src/protocol.rs`，错误使用宿主 transport 错误路径。

| 请求 kind | 额外输入 | 响应 kind | 权威与失败边界 |
| --- | --- | --- | --- |
| `invoke-command` | `commandId`、`args`、`context` | `accepted` | 已声明命令和宿主授权；接受请求不代表允许重放 |
| `query-projection` | — | `projection` | 只读插件投影 |
| `query-capabilities` | — | `capabilities` | 有效授权；请求的能力不一定已获授予 |
| `log` | `request: PluginLogRequestDto` | `accepted` | 活动 owner、schema、大小、脱敏和速率限制 |
| `notify` | `request: PluginNotificationRequestDto` | `accepted` | 宿主拥有的通知界面和有界数据 |
| `confirm` | `request: PluginConfirmationRequestDto` | `confirmation` | 返回 `accepted: boolean`；无头策略默认拒绝 |
| `submit-job` | `jobKind`、`input`、`provenance`、`policy`、`budget` | `job` | 活动的已声明 Job 与 `jobs.create`；宿主覆盖 provenance owner |
| `cancel-job` | `jobId` | `job` | 自己的 Job 和活动能力策略；运行时取消为协作式 |
| `query-job` | `jobId` | `job` | 按 owner 过滤的快照与能力策略 |

事件包括 `disabled` 和 `diagnostic`，带有 session/sequence 元数据。断开后查询权威状态并建立新会话；修改结果未知时必须先核对再重试，request ID 不代表幂等性。

## 如何使用参考 {/* #how-to-use-this-reference */}

先在[能力目录](/platform/capabilities)中选择任务，再运行对应示例。使用边界前检查 caller、能力、版本、错误、取消和销毁规则。[契约矩阵](plugin-contract-matrix.md)连接 owner 与测试，[生态验证](plugin-ecosystem-qualification.md)记录外部 SDK 尚需满足的条件。
