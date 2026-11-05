# workbench-protocol crate

## 职责

Canonical Rust transport contract: handshake, schema negotiation, Request/Response/Event, CoreTransport and wire DTOs.

## 非目标

It does not own layout, Asset, Job, persistence, plugin runtime or UI state.

## 稳定性

Highest-stability cross-process seam. Protocol versions, envelope fields and idempotency rules require compatibility fixtures.

## 输入和输出

输入：Typed envelopes, handshake requests and owner projections.
输出：CoreTransport、普通及插件 request/response/event envelope、Request、Response、Event、CoreError、握手和 projection DTO。

## 状态所有权

Domain owners and ApplicationCore own state; this crate only validates and describes the wire contract.

## 依赖方向

asset-core, layout-model, job-system, plugin-api and preview-core.

## 公共接口

CoreTransport、PluginRequestEnvelope、PluginResponseEnvelope、PluginEventEnvelope、Request、Response、Event、CoreError、握手和 projection DTO。

## 模块图

Target modules are handshake, profile, plugin, recovery, release, operation, bundle, request, response, event and error.

## 测试入口

Negotiation, serialization, idempotency, sequence and typed-error tests; cargo test -p velvet-workbench-protocol.

## 失败边界

Rejects incompatible schema/session/request shapes before dispatch; never performs I/O or runtime work.

## 迁移注意事项

在第二个客户端真正需要之前，保持 Rust、Tauri 和 TypeScript 的显式映射。manifest 与插件 envelope 参见 [plugin-manifest-v1.md](../../docs/engineering/plugin-manifest-v1.md)。

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
