# plugin-host crate

## 职责

Extension lifecycle orchestration: manifest validation, registration, activation, host-owned contribution registry, trust, runtime leases, storage, diagnostics and failure isolation.

## 非目标

It does not own Asset identity, layout semantics, CoreTransport implementation, UI framework state or arbitrary filesystem access.

## 稳定性

PluginHost, PluginRuntime, capability-facing handles, projections and typed errors are the public seam; internal runtime helpers stay private.

## 输入和输出

输入：Validated ExtensionManifest, package roots, runtime adapters and host policies.
输出：PluginHost, InMemoryPluginHost, PluginRuntime, runtime/diagnostic/storage/trust handles and extension projections.

## 状态所有权

PluginHost owns extension lifecycle, namespaced extension data, trust records and diagnostics; persistence is an adapter.

## 依赖方向

layout-model, job-system, persistence and plugin-api.

## 公共接口

PluginHost, InMemoryPluginHost, PluginRuntime, ContributionRegistrySnapshot, ContributionRegistrationHandle, runtime/diagnostic/storage/trust handles and extension projections.

## 模块图

Owner-named modules include registry, extension_storage, platform_process, publisher_trust, runtime_lease and trust_store; the host facade is now separate from crate lib.rs. `registry` owns validated active contribution projections and disposable handles; it does not own frontend registry state.

## 测试入口

Manifest, registry snapshot/disposal, permission, package path, runtime crash, trust, storage migration and lifecycle tests; cargo test -p velvet-plugin-host.

## 失败边界

A plugin may be disabled, quarantined, timed out or upgraded without taking down core Asset/layout/Preview state.

## 迁移注意事项

RuntimeKind, sidecar and trust types do not prove a production sandbox. Preserve install/recovery generations and public projections during future module extraction.

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
