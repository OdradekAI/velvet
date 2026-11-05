# workbench-protocol crate

## Responsibility

Canonical Rust transport contract: handshake, schema negotiation, Request/Response/Event, CoreTransport and wire DTOs.

## Non-goals

It does not own layout, Asset, Job, persistence, plugin runtime or UI state.

## Stability

Highest-stability cross-process seam. Protocol versions, envelope fields and idempotency rules require compatibility fixtures.

## Inputs and outputs

Inputs: Typed envelopes, handshake requests and owner projections.
Outputs: CoreTransport, protocol envelopes, plugin request/response/event envelopes, Request, Response, Event, CoreError, handshake and projection DTOs.

## State ownership

Domain owners and ApplicationCore own state; this crate only validates and describes the wire contract.

## Dependency direction

asset-core, layout-model, job-system, plugin-api and preview-core.

## Public interface

CoreTransport, PluginRequestEnvelope, PluginResponseEnvelope, PluginEventEnvelope, Request, Response, Event, CoreError, handshake and projection DTOs.

## Module map

Target modules are handshake, profile, plugin, recovery, release, operation, bundle, request, response, event and error.

## Tests

Negotiation, serialization, idempotency, sequence and typed-error tests; cargo test -p velvet-workbench-protocol.

## Failure boundary

Rejects incompatible schema/session/request shapes before dispatch; never performs I/O or runtime work.

## Migration notes

Keep Rust, Tauri and TypeScript mappings explicit until a second client justifies a generated/shared protocol. The manifest and plugin envelope reference is [plugin-manifest-v1.md](../../docs/engineering/plugin-manifest-v1.md).

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
