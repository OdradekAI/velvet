# Plugin registry v1

Status: Alpha contract frozen in-repository; registry fixtures are not a
published SDK or production runtime.

This document records the implementation slice for issue 003. It describes the
current host-owned registry contract; it does not claim that a WASM or sidecar
runtime, marketplace, or capability policy is complete.

The frozen Alpha surface is the declarative registry snapshot, lifecycle state,
opaque contribution cleanup handle and transport projection. The host remains
the only owner of registration state. External SDK compatibility is deferred
until a second real consumer and production runtime, trust and platform
qualification evidence exist.

## Owner and seam

`velvet-plugin-host::InMemoryPluginHost` owns active contribution
registrations. A registration is created only after a manifest has passed the
existing host validation and its runtime has started. The registry stores a
declarative `ContributionDescriptor`, its extension owner, namespace, scope,
zone and `CONTRIBUTION_SCHEMA_VERSION` (`1`). It does not store React/Vue
objects, Tauri state, database handles or filesystem paths.

`PluginHost::registry_snapshot` is the read seam for `ApplicationCore` and
transport adapters. `ContributionRegistrationHandle` is an opaque host handle
for the one registration that created it. `dispose_contribution` is
idempotent while the extension remains registered: the first call removes that
contribution and later calls are no-ops. After uninstall, the same handle returns
`PluginError::NotFound`. Disposing one contribution does not change the
extension lifecycle; only host lifecycle operations do.

## Lifecycle rules

- Registering a manifest does not publish contributions.
- Activation publishes all validated descriptors in one registry generation.
- Disable, runtime crash, storage/runtime failure, orphan reconciliation,
  uninstall and rollback revoke every contribution for that extension.
- An orphan whose process identity cannot be safely confirmed is projected as
  `quarantined`; its contributions and capability handles stay revoked until a
  host-owned recovery action resolves the lease evidence. A quarantined
  extension is not implicitly reactivated.
- A failed registration shuts down only the starting runtime and leaves the
  extension in `Failed`; other extensions keep their registrations.
- `active_contributions` reads the host registry, so a disposed contribution
  cannot be invoked or used to create a plugin Job handle.
- The registry generation is a reconnect/resync hint. It is not a durable
  persistence revision and must not be used as an asset, layout or profile
  revision.

## Transport projection

`Query::PluginRegistry { extension_id }` projects the current snapshot through
`ApplicationCore`, `workbench-protocol`, the Tauri wire adapter and the desktop
TypeScript mirror. The optional extension filter is a projection filter only;
it does not grant the caller access to another extension's runtime or data.
The desktop may cache the snapshot, but the Rust host remains authoritative.

The transport DTO contains declarative descriptor metadata and a schema version.
It does not contain the internal disposable token. A future capability-gated
mutation may add a typed host operation if a real consumer needs it; plugins
must not receive the registry map or call another view's implementation.

`activation_candidates` and `activate_for_event` are host-local seams for exact
manifest event matching. They are not yet wired to an ApplicationCore or
CoreTransport event source, so callers must not treat them as a completed
Workbench/View/Command/Preview/Job event bus.

## Deferred work

Activation selector indexing, runtime watchdogs and full lifecycle event
streaming remain follow-up work in issues 004, 006 and 007 or a later ADR. The
current contract now validates API/capability declarations before registration,
uses opaque generation-bound capability handles, and returns stable timeout /
cancelled error codes for bounded runtime invocations. Job cancellation remains
cooperative in protocol v1: the Rust JobSystem owns the durable transition and
the runtime may report a bounded `cancelled` event. It establishes the
registration owner, cleanup seam, projection and failure isolation needed by
the later selector and runtime work.

The cross-workspace evidence for this registry is listed in the
[plugin contract matrix](plugin-contract-matrix.md). A developer-facing
walkthrough is available in the [plugin developer guide](plugin-developer-guide.md).

## Host capability calls

The same owner boundary now carries the minimum host services needed by a
runtime adapter. `PluginRequest` supports bounded log, notification,
confirmation and Job submit/cancel/query messages. `ApplicationCore` maps those
messages to `PluginHost` and the Rust `JobSystem`; it overwrites Job provenance
ownership with the caller extension, checks lifecycle and capabilities on every
call, and filters Job status by owner. The desktop `JobClient` and
`PluginClient` expose only typed CoreTransport requests, while
`velvet_plugin_request` is a thin Tauri adapter for a bound plugin session.

Logs and notifications are redacted, bounded and rate-limited. Confirmation is
data-only and defaults to deny in headless mode until a host Overlay adapter is
installed. A disabled extension cannot emit new records or submit/query/cancel
Jobs, and disabling it still cancels its owned Jobs through `JobSystem`. These
contracts do not claim a production WASM/sidecar SDK or arbitrary runtime
callbacks.
