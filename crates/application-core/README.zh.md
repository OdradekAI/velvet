# application-core crate

## 职责

Composition root and request coordinator for layout, preview, jobs, PluginHost, persistence, recovery and release.

## 非目标

It is not a replacement Asset repository, persistence owner, renderer or Tauri command layer.

## 稳定性

ApplicationCore, ApplicationRecoveryStores, InProcessCoreTransport and public coordination methods are the integration seam.

## 输入和输出

输入：Owner adapters, typed Request envelopes, stores and runtime policies.
输出：ApplicationCore, ApplicationRecoveryStores, InProcessCoreTransport and exported operation/recovery/preview source types.

## 状态所有权

ApplicationCore composes owners; LayoutService, JobSystem, PreviewResolver, PluginHost and persistence adapters retain their own state ownership.

## 依赖方向

All core domain/contract crates except tauri-adapter.

## 公共接口

ApplicationCore, ApplicationRecoveryStores, InProcessCoreTransport and exported operation/recovery/preview source types.

## 模块图

Existing bundle, operation, preview_source, recovery and release modules remain; composition.rs now holds the composition facade implementation and should be split further by dispatch/service seams in later work.

## 测试入口

Core recovery, protocol dispatch, preview fallback, plugin lifecycle and Job ownership tests; cargo test -p velvet-application-core.

## 失败边界

Optional plugin/runtime failures degrade locally; core recovery and durable owner failures use typed Safe Mode/recovery paths.

## 迁移注意事项

Keep constructor and transport compatibility wrappers. Do not move domain ownership into ApplicationCore while extracting private modules.

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
