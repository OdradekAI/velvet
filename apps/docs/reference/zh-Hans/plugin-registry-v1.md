---
id: plugin-registry-v1
title: 插件注册表 v1
---

# 插件注册表 v1

<span id="插件-registry-v1" className="velvet-doc-anchor" />

状态：Alpha 契约已在仓库内冻结；注册表测试夹具不等于已发布的 SDK 或生产运行时。

本文记录 issue 003 的实现切片，描述当前由宿主拥有的注册表契约，不宣称 WASM 或 sidecar 运行时、插件市场或能力策略已经完备。

冻结的 Alpha 范围包括声明式注册表快照、生命周期状态、不透明的贡献清理句柄和传输投影。宿主仍是注册状态的唯一所有者。外部 SDK 兼容性承诺将推迟到第二个真实消费者出现，并具备生产运行时、信任和平台资格验证证据之后。

<span id="所有权" className="velvet-doc-anchor" />

## 所有者与接口 {/* #owner-and-seam */}

`velvet-plugin-host::InMemoryPluginHost` 拥有活动贡献的注册记录。只有 manifest 通过现有宿主校验且运行时已启动，才会创建注册。注册表保存声明式 `ContributionDescriptor`、所属扩展、命名空间、作用域、区域和 `CONTRIBUTION_SCHEMA_VERSION`（`1`），不保存 React/Vue 对象、Tauri 状态、数据库句柄或文件系统路径。

`PluginHost::registry_snapshot` 是 `ApplicationCore` 和传输适配器读取注册表的接口。`ContributionRegistrationHandle` 是不透明的宿主句柄，仅对应创建它的那一条注册。只要宿主仍保存该扩展的 manifest，`dispose_contribution` 就是幂等的：首次调用移除该贡献，后续调用不执行任何操作。卸载后再次使用同一句柄会返回 `PluginError::NotFound`。释放单个贡献不会改变扩展生命周期；只有宿主生命周期操作才会改变它。

<span id="注册顺序" className="velvet-doc-anchor" />
<span id="失败处理" className="velvet-doc-anchor" />

## 生命周期规则 {/* #lifecycle-rules */}

- 注册 manifest 不会发布贡献。
- 激活会在同一注册表代际中发布所有已校验的描述符。
- 禁用、运行时崩溃、存储/运行时失败、孤儿运行时核对、卸载和回滚都会撤销该扩展的全部贡献。
- 无法安全确认进程身份的孤儿运行时会投影为 `quarantined`；其贡献和能力句柄保持撤销，直到宿主拥有的恢复操作处理完租约证据。处于隔离状态的扩展不会被隐式重新激活。
- 注册失败时，只关闭正在启动的那个运行时，并将扩展置于 `Failed`；其他扩展保留各自的注册。
- `active_contributions` 读取宿主注册表，因此已释放的贡献不能被调用，也不能用于创建插件 Job 句柄。
- 注册表代际是重连/重新同步的提示，不是持久化修订版本，不能用作 Asset、布局或 Profile 的修订版本。

## 传输投影 {/* #transport-projection */}

`Query::PluginRegistry { extension_id }` 经 `ApplicationCore`、`workbench-protocol`、Tauri 消息映射适配器和桌面 TypeScript 镜像投影当前快照。可选的扩展筛选条件仅筛选投影，不授予调用者访问其他扩展运行时或数据的权限。桌面可以缓存快照，但 Rust 宿主始终是权威来源。

传输 DTO 包含声明式描述符元数据和 schema 版本，不包含内部释放令牌。如果未来有真实消费者需要受能力控制的变更，可以新增类型化宿主操作；插件不得获得注册表映射，也不得调用其他视图的实现。

`activation_candidates` 和 `activate_for_event` 是宿主内部的接口，用于精确匹配 manifest 事件。它们尚未接入 ApplicationCore 或 CoreTransport 事件源，因此调用者不能把它们视为完整的 Workbench/View/Command/Preview/Job 事件总线。

## 后续工作 {/* #deferred-work */}

激活选择器索引、运行时 watchdog 和完整的生命周期事件流仍留待 issues 004、006、007 或后续 ADR。当前契约已在注册前校验 API/能力声明，使用与代际绑定的不透明能力句柄，并为有界运行时调用返回稳定的超时/取消错误代码。协议 v1 中的 Job 取消仍采用协作式机制：Rust JobSystem 拥有持久化状态转换，运行时可以报告有界的 `cancelled` 事件。这一契约建立了后续选择器和运行时工作所需的注册所有者、清理接口、投影和失败隔离边界。

该注册表的跨工作区证据见[插件契约矩阵](/reference/plugin-contract-matrix)。面向开发者的操作说明见[插件开发者指南](/reference/plugin-developer-guide)。

## 宿主能力调用 {/* #host-capability-calls */}

同一所有权边界现已承载运行时适配器所需的最小宿主服务。`PluginRequest` 支持有界的日志、通知、确认，以及 Job 提交/取消/查询消息。`ApplicationCore` 将这些消息映射到 `PluginHost` 和 Rust `JobSystem`；它会将 Job 来源信息中的所有者覆盖为调用扩展，在每次调用时检查生命周期与能力，并按所有者筛选 Job 状态。桌面 `JobClient` 和 `PluginClient` 仅提供类型化的 CoreTransport 请求；`velvet_plugin_request` 则是面向已绑定插件会话的薄 Tauri 适配器。

日志和通知会经过脱敏、大小限制和速率限制。确认请求仅包含数据；在安装宿主 Overlay 适配器之前，无界面的运行模式默认拒绝确认。被禁用的扩展不能发出新记录，也不能提交/查询/取消 Job；禁用操作仍会通过 `JobSystem` 取消它拥有的 Job。这些契约不代表已有生产 WASM/sidecar SDK，也不允许任意运行时回调。
