# plugin-host crate

## Responsibility

Extension lifecycle orchestration: manifest validation, registration, activation, trust, runtime leases, storage, diagnostics and failure isolation.

## Non-goals

It does not own Asset identity, layout semantics, CoreTransport implementation, UI framework state or arbitrary filesystem access.

## Stability

PluginHost, PluginRuntime, capability-facing handles, projections and typed errors are the public seam; internal runtime helpers stay private.

## Inputs and outputs

Inputs: Validated ExtensionManifest, package roots, runtime adapters and host policies.
Outputs: PluginHost, InMemoryPluginHost, PluginRuntime, runtime/diagnostic/storage/trust handles and extension projections.

## State ownership

PluginHost owns extension lifecycle, namespaced extension data, trust records and diagnostics; persistence is an adapter.

## Dependency direction

layout-model, job-system, persistence and plugin-api.

## Public interface

PluginHost, InMemoryPluginHost, PluginRuntime, runtime/diagnostic/storage/trust handles and extension projections.

## Module map

Owner-named modules include extension_storage, platform_process, publisher_trust, runtime_lease and trust_store; the host facade is now separate from crate lib.rs.

## Tests

Manifest, permission, package path, runtime crash, trust, storage migration and lifecycle tests; cargo test -p velvet-plugin-host.

## Failure boundary

A plugin may be disabled, quarantined, timed out or upgraded without taking down core Asset/layout/Preview state.

## Migration notes

RuntimeKind, sidecar and trust types do not prove a production sandbox. Preserve install/recovery generations and public projections during future module extraction.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
