# tauri-adapter crate

## Responsibility

Framework-free implementation of CoreTransport session, idempotency, outcome caching, events and platform signal adaptation.

## Non-goals

It does not own Tauri State, persistence, plugin runtime, preview providers or renderer components.

## Stability

TauriTransport, CoreEventSink and session/outcome behavior are the adapter seam.

## Inputs and outputs

Inputs: ApplicationCore, typed Requests, handshake/session data and platform signal observations.
Outputs: TauriTransport, CoreEventSink, InMemoryEventSink and platform signal controller.

## State ownership

TauriTransport owns adapter session/outcome/event state; ApplicationCore owns durable domain state.

## Dependency direction

application-core, job-system and workbench-protocol.

## Public interface

TauriTransport, CoreEventSink, InMemoryEventSink and platform signal controller.

## Module map

transport.rs contains transport behavior; platform_signals.rs owns signal adaptation.

## Tests

Handshake, session expiry, idempotency, event mapping and platform signal tests; cargo test -p velvet-tauri-adapter.

## Failure boundary

Mapping/session failures become typed CoreError; native Tauri APIs stay outside this crate.

## Migration notes

Keep command names and wire JSON stable. Tauri command handlers remain thin adapters in velvet-desktop.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
