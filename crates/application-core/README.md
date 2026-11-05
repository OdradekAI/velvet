# application-core crate

## Responsibility

Composition root and request coordinator for layout, preview, jobs, PluginHost, persistence, recovery and release.

## Non-goals

It is not a replacement Asset repository, persistence owner, renderer or Tauri command layer.

## Stability

ApplicationCore, ApplicationRecoveryStores, InProcessCoreTransport and public coordination methods are the integration seam.

## Inputs and outputs

Inputs: Owner adapters, typed Request envelopes, stores and runtime policies.
Outputs: ApplicationCore, ApplicationRecoveryStores, InProcessCoreTransport and exported operation/recovery/preview source types.

## State ownership

ApplicationCore composes owners; LayoutService, JobSystem, PreviewResolver, PluginHost and persistence adapters retain their own state ownership.

## Dependency direction

All core domain/contract crates except tauri-adapter.

## Public interface

ApplicationCore, ApplicationRecoveryStores, InProcessCoreTransport and exported operation/recovery/preview source types.

## Module map

Existing bundle, operation, preview_source, recovery and release modules remain; composition.rs now holds the composition facade implementation and should be split further by dispatch/service seams in later work.

## Tests

Core recovery, protocol dispatch, preview fallback, plugin lifecycle and Job ownership tests; cargo test -p velvet-application-core.

## Failure boundary

Optional plugin/runtime failures degrade locally; core recovery and durable owner failures use typed Safe Mode/recovery paths.

## Migration notes

Keep constructor and transport compatibility wrappers. Do not move domain ownership into ApplicationCore while extracting private modules.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
