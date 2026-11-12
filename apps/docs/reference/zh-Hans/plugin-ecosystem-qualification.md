---
id: plugin-ecosystem-qualification
title: 插件生态资格验证门槛
---

# 插件生态资格验证门槛

<span id="插件生态-qualification-gate" className="velvet-doc-anchor" />

状态：生态门槛暂缓（`deferred`）；记录于 2026-11-06。

本文定义将仓库拥有的 Alpha 宿主契约升级为外部 SDK 契约之前所需的证据。这是一份资格验证计划，不是 Marketplace、远程部署服务或生产 WASM/sidecar 运行时的实现。结果采用与[插件契约矩阵](/reference/plugin-contract-matrix)相同的词汇：`pass`、`pass (fixture)`、`partial`、`not run` 和 `deferred`。

## 升级规则 {/* #promotion-rule */}

外部 SDK 门槛保持 `deferred`，直到下表每一行都达到 `pass` 或具备明确接受的发布资格验证结果。同时必须没有未解决的 P1，且第二个真实消费者已独立完成开发路径。只有在已接受的 ADR 中记录升级决定之后，才能改变契约范围；在此之前，唯一冻结的契约仍是 Alpha 契约及其兼容性测试夹具。

<span id="gate" className="velvet-doc-anchor" />

## 证据矩阵 {/* #evidence-matrix */}

| 门槛 | 所有者 | 必需输入 | 证据接口 | 当前结果 | 通过条件 |
| --- | --- | --- | --- | --- | --- |
| 第二个真实消费者 | 开发者体验 + 工作台 | 仅使用已发布文档开发的独立扩展 | 独立包仓库或隔离工作区；安装、升级、禁用、卸载和恢复测试夹具 | `deferred` | 消费者无需内部导入或宿主专用测试替身，即可完成完整生命周期 |
| 运行时约束 | PluginHost + JobSystem + 平台发布 | WASM/sidecar 插件包和资源策略 | 运行时预算测试、协作式取消/超时、崩溃和畸形响应注入、孤儿运行时核对，以及目标 OS 上的 watchdog 证据 | `partial` | 每个受支持目标上的资源限制、取消、崩溃隔离、孤儿清理和 watchdog 行为均通过验证 |
| 插件包信任与回滚 | PluginHost + 发布工程 | 已签名插件包、发布者密钥和安全事件文档 | 摘要/身份绑定、发布者信任、撤销、防降级、离线缓存、暂存安装和回滚测试夹具 | `partial` | 签名包和撤销/回滚行为在线、离线均通过验证，包含密钥轮换和过期元数据情形 |
| UI Kit 平台资格验证 | 桌面 UI + 无障碍 | 受支持 OS/WebView 矩阵和 UI Kit 测试包 | 主题/令牌、来源、键盘、焦点转移、屏幕阅读器、减少动态效果和无障碍探测；插件界面释放/重试 | `deferred` | 每个受支持 OS/WebView 都通过宿主/插件边界验证，插件不能任意操纵宿主 DOM 或取得布局所有权 |
| 传输与状态权威 | 协议 + ApplicationCore | 会话中断和结果未知场景 | CoreTransport 重连/重新同步、旧会话拒绝、Job 来源信息、取消和插件失败隔离测试夹具 | `pass (fixture)` | 测试夹具通过，且目标运行时证据确认不存在重复的状态所有者或丢失的恢复路径 |
| 失败隔离与恢复 | PluginHost + 工作台 | 崩溃、挂起、被禁用和不兼容的扩展 | 贡献清理、Job 取消、UI 卸载、诊断投影、核心 Asset/布局/预览持久化检查 | `pass (fixture)` | 每条运行时路径中的扩展失败都不会破坏核心状态或其他扩展的可用性 |
| SDK/测试工具包生命周期 | 架构 + 开发者体验 | 版本策略、兼容性矩阵和弃用计划 | SDK 包、测试夹具/测试工具包发布、退役和迁移测试；API 差异审查 | `deferred` | 有版本的 SDK 和测试工具包可以发布、弃用和退役，且不存在未记录的兼容性承诺 |
| 发布就绪 | 发布工程 + 架构 | 资格验证报告和问题清单 | `cargo clippy --workspace --all-targets -- -D warnings`、平台报告、P1 分诊和已接受 ADR 的引用 | `partial` | 没有未解决的 P1，严格 lint 通过，附齐所有目标的资格验证报告，且 ADR 状态明确 |

<span id="当前限制" className="velvet-doc-anchor" />

## 当前 Alpha 证据 {/* #current-alpha-evidence */}

以下都是有效的仓库内证据；如果未实际覆盖生产运行时，就必须继续标注为测试夹具：

- `crates/plugin-api`、`plugin-host`、`workbench-protocol`、`application-core` 和 `job-system` 契约测试。
- 独立 Tauri 工作区测试和桌面 `npm test` 门槛。
- `examples/plugins/asset-inspector` 和 `examples/plugins/echo-sidecar`。
- 模拟 `HostFacade` 以及 UI Kit 桥接/组件测试夹具。
- [插件契约矩阵](/reference/plugin-contract-matrix)和[插件开发者指南](/reference/plugin-developer-guide)。

这些测试夹具证明类型化接口、生命周期清理、权限检查、取消和失败投影。宿主测试夹具也覆盖插件包签名验证、发布者密钥轮换和已撤销密钥的拒绝；它们并不证明已有生产签名/撤销服务、安全沙箱、目标平台无障碍保障或公开 SDK ABI。

## 所有权与变更控制 {/* #ownership-and-change-control */}

Alpha 契约仍由 `plugin-api`、`plugin-host`、`workbench-protocol`、`application-core`、`JobSystem`、桌面平台镜像和宿主 UI Kit 拥有。本资格验证文档不创建新 crate、协议所有者或持久化状态存储。升级为外部 SDK 时，必须在同一次审查中更新相关 ADR、术语表、兼容性矩阵、发布说明和开发者文档。
