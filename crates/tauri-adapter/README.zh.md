# tauri-adapter crate

## 职责

Framework-free implementation of CoreTransport session, idempotency, outcome caching, events and platform signal adaptation.

## 非目标

It does not own Tauri State, persistence, plugin runtime, preview providers or renderer components.

## 稳定性

TauriTransport, CoreEventSink and session/outcome behavior are the adapter seam.

## 输入和输出

输入：ApplicationCore, typed Requests, handshake/session data and platform signal observations.
输出：TauriTransport, CoreEventSink, InMemoryEventSink and platform signal controller.

## 状态所有权

TauriTransport owns adapter session/outcome/event state; ApplicationCore owns durable domain state.

## 依赖方向

application-core, job-system and workbench-protocol.

## 公共接口

TauriTransport, CoreEventSink, InMemoryEventSink and platform signal controller.

## 模块图

transport.rs contains transport behavior; platform_signals.rs owns signal adaptation.

## 测试入口

Handshake, session expiry, idempotency, event mapping and platform signal tests; cargo test -p velvet-tauri-adapter.

## 失败边界

Mapping/session failures become typed CoreError; native Tauri APIs stay outside this crate.

## 迁移注意事项

Keep command names and wire JSON stable. Tauri command handlers remain thin adapters in velvet-desktop.

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
