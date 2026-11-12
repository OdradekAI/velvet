---
id: plugin-contract-matrix
title: 插件契约与集成验证矩阵
---

# 插件契约与集成验证矩阵

<span id="插件-contract-与集成验证矩阵" className="velvet-doc-anchor" />

状态：有效的 Alpha 契约门槛；记录于 2026-11-06。

本文是 issue 010 的验证入口，用于检查横跨 Rust 工作区、独立 Tauri 工作区和桌面 TypeScript/UI Bridge 的插件边界。文档记录可重复运行的测试夹具，以及每项结果的适用范围。单元测试或契约测试通过，只能证明指定接口；不能证明生产插件市场、签名服务或平台沙箱已经可用。

## Alpha 契约冻结范围 {/* #alpha-contract-freeze */}

当前冻结范围包括仓库拥有的 manifest schema 与 ID/版本范围规则、生命周期和贡献注册表、能力检查、CoreTransport 镜像、Job/诊断接口、扩展存储/facet 来源信息，以及框架无关的 UI Kit 桥接。遇到主版本不匹配、manifest 中非保留的未知字段、无效能力/区域或不兼容的组件契约时，这些接口均拒绝继续；次版本演进只能是可选的增量扩展。

Asset Inspector、Echo sidecar 和模拟 `HostFacade` 都是本地确定性测试夹具。适用时，它们的结果标为 `pass (fixture)`，不能据此认定已有生产 WASM/sidecar 运行时、公开 SDK、Marketplace ABI、发布者签名/撤销服务或跨平台 UI 资格验证。

<span id="证据类型" className="velvet-doc-anchor" />

## 结果词汇 {/* #result-vocabulary */}

| 结果 | 含义 |
| --- | --- |
| `pass` | 指定的确定性测试夹具或命令已运行且通过。 |
| `pass (fixture)` | 该接口由模拟实现或本地测试夹具覆盖；平台资格验证仍需单独完成。 |
| `partial` | 宿主契约通过，但目标平台、发布产物或生产运行时门槛仍未关闭。 |
| `not run` | 记录该次运行时，命令或环境不可用。它不代表通过。 |
| `deferred` | 场景超出当前 Alpha 范围，例如 Marketplace 操作或实际断电。 |

## 三个验证层 {/* #three-validation-layers */}

分别运行这些验证层，因为根工作区和 Tauri 不属于同一个 Cargo 工作区，前端也拥有自己的协议镜像。

| 层 | 命令 | 覆盖范围 | 当前结果 |
| --- | --- | --- | --- |
| 根 Rust 工作区 | `cargo fmt --all -- --check` | 根工作区十个 crate 的格式 | `pass` |
| 根 Rust 工作区 | `cargo test --workspace` | 领域、协议、manifest、注册表、能力、Job、存储、诊断和 ApplicationCore 契约 | `pass` |
| 根 Rust 工作区 | `cargo clippy --workspace --all-targets` | 跨 crate lint 和公共接口回归 | `pass` |
| Tauri 工作区 | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` | Tauri JSON 映射、命令策略、原生生命周期和平台适配器 | `pass` |
| 桌面前端 | `npm --prefix apps/desktop run typecheck` | TypeScript 协议镜像、Workbench、PluginClient、UI Bridge 和渲染器适配器 | `pass` |
| 桌面前端 | `npm --prefix apps/desktop test` | 编译后的渲染器测试及完整桌面测试夹具套件 | `pass` |

上表结果是此检出版本在 2026-11-06 记录的本地运行结果。若某个目标环境无法运行命令，应记录 `not run` 或 `deferred`，并写明具体环境原因，不能改标为 `pass`。

## 跨边界契约矩阵 {/* #cross-boundary-contract-matrix */}

| 契约 | Rust 所有者与接口 | Tauri/传输接口 | 桌面/UI 接口 | 可重复验证的证据 | 结果 |
| --- | --- | --- | --- | --- | --- |
| Manifest schema 与标识符 | `plugin-api::ExtensionManifest`、`validate_contract` | `PluginManifestDto` 和 Tauri JSON 映射 | `apps/desktop/src/platform/protocol.ts` 镜像 | `crates/plugin-api/src/lib.rs` 中的 manifest 往返/未知字段测试；Asset Inspector 测试夹具；`plugin-manifest-v1.md` | `pass` |
| 插件信封与会话策略 | `workbench-protocol::PluginRequestEnvelope` | `tauri-adapter` 会话、请求结果和事件映射 | `apps/desktop/src/platform/tauri-transport.ts` | `crates/workbench-protocol/src/protocol_tests.rs`；`crates/tauri-adapter/src/transport_tests.rs`；`apps/desktop/test/tauri-transport.test.mjs` | `pass` |
| 注册表生命周期与贡献清理 | `plugin-host::PluginHost`/`InMemoryPluginHost` | 经 `ApplicationCore` 输出的插件注册表投影 | `PluginClient`、`ViewRegistry`、命令/菜单注册表和 UI Bridge | `crates/plugin-host/src/host_tests.rs`；`apps/desktop/test/plugin-contribution-lifecycle.test.mjs`；`plugin-registry-v1.md` | `pass` |
| 能力与区域约束 | `plugin-host` 能力策略 | 分发前进行类型化 manifest/投影校验 | 渲染器消费已筛选的投影 | `crates/plugin-host/src/capabilities.rs` 和 `host_tests.rs`；`apps/desktop/test/plugin-surface-adapter.test.mjs` | `pass` |
| 命令、菜单与快捷键 | 宿主拥有的贡献描述符和调用校验 | `invoke-plugin-command` CoreTransport 路径 | `CommandRegistry`、`MenuRegistry`、快捷键适配器 | `crates/plugin-host/src/host_tests.rs`；`apps/desktop/test/external-plugin-e2e.test.mjs`；`apps/desktop/test/workbench-dom-regression.test.mjs` | `pass` |
| 诊断、通知与恢复 | `PluginDiagnosticSink` 和有界的 ApplicationCore 历史 | 类型化诊断投影/导出 | 诊断服务、恢复操作和通知接口 | `crates/plugin-host/src/host_tests.rs`；`crates/application-core/src/composition_tests.rs`；`apps/desktop/test/diagnostics-service.test.mjs`（包含在 `npm test` 中）；`apps/desktop/test/plugin-recovery-operations.test.mjs` | `pass` |
| 宿主服务与插件拥有的 Job | `PluginHost` 能力检查、确认提供者和 `JobSystem` 所有者 | `PluginRequest` 宿主消息、`velvet_plugin_request` 和类型化 Job DTO | `PluginClient` 宿主服务调用，以及 `JobClient` 插件提交/查询/取消方法 | `crates/plugin-host/src/host_tests.rs`；`crates/workbench-protocol/src/protocol_tests.rs`；`crates/application-core/src/composition_tests.rs`；`apps/desktop/src-tauri` 消息映射测试 | `pass (fixture)` |
| Job、取消与来源信息 | `job-system::JobSystem` 和 `PluginJobInvocation` | Job 请求/结果 DTO 和取消命令 | `JobClient` 和操作面板投影 | `crates/job-system/src/lifecycle_tests.rs`；`crates/plugin-host/src/host_tests.rs`；`apps/desktop/test/job-operations.test.mjs` | `pass` |
| UI Kit 主题与组件协商 | 宿主拥有的 UI 桥接 schema 和渲染器组件 | 桥接版本、来源策略以及类型化覆盖层/进度请求 | `PluginUiRegistry`、`PluginSurfaceHost`、令牌清理器和宿主 UI Kit | `apps/desktop/test/ui-kit-contract.test.mjs`（schema、令牌过滤、主题更新、组件回退）；`apps/desktop/test/ui-kit-components.test.mjs`（标签页、状态、网格、表格、命令面板、主题应用）；`apps/desktop/test/plugin-surface-adapter.test.mjs`（来源与释放）；`apps/desktop/test/workbench-dom-regression.test.mjs`（ARIA 标签与焦点恢复）；`ui-kit-contract-v1.md`；`ui-kit-component-guide.md` | `pass (fixture)` |
| Asset Inspector 开发路径 | 命名空间内的扩展 facet 和 Job 所有者 | Manifest/投影/Job DTO | 预览增强和 View | `examples/plugins/asset-inspector/manifest.json`；`examples/plugins/asset-inspector/src/asset-inspector.mjs`；`examples/plugins/asset-inspector/src/fixture-host.mjs`；`examples/plugins/asset-inspector/run-demo.mjs`；`apps/desktop/test/asset-inspector-demo.test.mjs` | `pass (fixture)` |
| 外部 sidecar 失败隔离 | `ProcessPluginRuntime` 和各扩展独立的失败域 | 运行时状态、禁用和恢复投影 | 移除 UI 界面并在恢复后重新挂载 | `examples/plugins/echo-sidecar`；`crates/plugin-host/src/host_tests.rs`；`apps/desktop/test/external-plugin-e2e.test.mjs` | `pass (fixture)` |
| 扩展存储迁移 | `plugin-host::ExtensionStorage` 和迁移参与者 | Manifest 存储声明和恢复操作 | 恢复 UI 仅消费元数据投影 | `crates/plugin-host/src/host_tests.rs`；`apps/desktop/test/asset-inspector-demo.test.mjs` | `pass` |
| 安装、升级、回滚与卸载 | ApplicationCore 包事务和 PluginHost 生命周期 | 类型化检查/安装/提交/回滚命令 | Extensions 对话框和恢复操作 | `crates/application-core/src/composition_tests.rs`；`apps/desktop/test/plugin-manager-dialog.test.mjs`；`apps/desktop/test/plugin-package-picker.test.mjs` | `pass (fixture)` |
| 兼容性与迁移接口 | `plugin-api` schema/版本范围类型；PluginHost 存储和 facet 所有者 | Manifest、运行时、存储和恢复 DTO | UI Bridge/组件/主题协商 | `crates/plugin-api/src/lib.rs`；`crates/plugin-host/src/host_tests.rs`；`crates/workbench-protocol/src/protocol_tests.rs`；`apps/desktop/test/ui-kit-contract.test.mjs`；Asset Inspector schema/迁移测试夹具 | `pass (fixture)` |

facet schema 是命名空间内所有者的契约，不是第二个全局 schema 注册表。当前测试夹具携带 `schemaVersion: 1` 和输入指纹；未来不兼容的 facet schema 必须先增加所有者迁移或重新计算测试夹具，才能被接受。facet 过期、被禁用或被隔离时，宿主必须保持 Asset 身份可读。

矩阵明确让 `PluginHost` 拥有扩展生命周期、能力、诊断和扩展存储。`CoreTransport` 承载类型化投影和命令，不成为第二个插件所有者。前端拥有渲染和临时交互状态，Rust 则始终是持久化状态与失败恢复的权威来源。

## 测试夹具目录约定 {/* #fixture-directory-convention */}

选择拥有该接口的最小测试夹具位置：

```text
examples/plugins/<extension-id>/
  manifest.json          # 包级 manifest 测试夹具
  README.md              # 面向开发者的契约和限制
  README.zh.md           # 公开测试夹具的中文配套文档
  src/ or sidecar.mjs    # 确定性测试夹具实现

crates/<crate>/src/*_tests.rs
crates/<crate>/tests/     # Rust 契约或资格验证测试夹具
apps/desktop/test/        # 前端、传输和 UI Bridge 测试夹具
tools/release-qualification/fixtures/
                            # 仅存放发布/平台测试夹具
```

测试夹具必须是确定性、本地、有界的，并独立于用户 Asset 数据。插件测试夹具可以使用模拟宿主句柄，但不能读取真实数据库、来源路径、Tauri 状态对象、React 组件或其他 View。运行时测试夹具必须将 stdout 留给协议数据，并限制 stderr 和载荷大小。模拟失败的测试夹具还必须断言清理结果：贡献已移除、Job 已取消或中断、核心状态保留，且没有孤立的运行时句柄。

## 失败与恢复覆盖 {/* #failure-and-recovery-coverage */}

每个新的跨边界契约至少应为以下每条路径添加一个测试夹具，才算完成：

1. 有效的往返或投影。
2. 拒绝未知 schema、版本、能力或贡献。
3. 异步契约中的取消、超时或过期结果处理。
4. 所有者失败后，其他扩展和核心状态仍然可用。
5. 禁用、升级、回滚或卸载后的清理。
6. 在不改变 Asset、布局或 Profile 权威归属的前提下，迁移或恢复扩展拥有的数据。

负向路径也是契约的一部分。仅断言成功投影的测试不足以覆盖插件工作。

## 资格验证限制 {/* #qualification-limits */}

以下仍是明确的门槛，不能视为已满足的前提：

- `RuntimeKind::Wasm` 和 `RuntimeKind::Sidecar` 描述公开契约；它们不会让 sidecar 自动成为安全沙箱，也不能证明生产 WASM 运行时已经可用。
- 本地插件包签名、发布者密钥轮换和已撤销密钥的拒绝已有宿主测试夹具。生产签名/撤销服务的运营、Marketplace 发布、公证和安装器回滚仍需发布资格验证。
- 发布验证工具中已有 macOS/Linux 原生生命周期证据；Windows 运行时、Job Object 和安装器资格验证仍受环境门槛约束。
- `npm test` 通过只能证明当前桌面测试工具覆盖的情形，不能验证所有 WebView、GPU、文件系统、电源或网络卷组合。
- 当前测试夹具 `HostFacade` 是可运行的结构化测试接口，不是已交付的 SDK。`node examples/plugins/asset-inspector/run-demo.mjs` 直接运行插件包，但仍不能验证生产 WASM/sidecar 运行时。公开 SDK 需要已接受的 API/版本策略 ADR，以及第二个消费者。

面向开发者的操作说明和仅适用于测试夹具的 HostFacade 目录，见[插件开发者指南](/reference/plugin-developer-guide)。各项契约见 [Plugin manifest v1](/reference/plugin-manifest-v1)、[插件注册表 v1](/reference/plugin-registry-v1)和 [UI Kit 契约 v1](/reference/ui-kit-contract-v1)。

未来外部 SDK 的升级门槛定义在[插件生态资格验证门槛](/reference/plugin-ecosystem-qualification)中。目前该项暂缓，不改变当前 Alpha 契约。
