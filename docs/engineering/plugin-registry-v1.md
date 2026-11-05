# Plugin registry v1

This document records the implementation slice for issue 003. It describes the
current host-owned registry contract; it does not claim that a WASM or sidecar
runtime, marketplace, or capability policy is complete.

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
idempotent: the first call removes that contribution and later calls are
no-ops. The extension lifecycle remains active until the host deactivates or
quarantines it.

## Lifecycle rules

- Registering a manifest does not publish contributions.
- Activation publishes all validated descriptors in one registry generation.
- Disable, runtime crash, storage/runtime failure, orphan reconciliation,
  uninstall and rollback revoke every contribution for that extension.
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

Activation selector indexing, capability grants, deadline/cancellation tokens,
runtime watchdogs and full lifecycle event streaming remain follow-up work in
issues 004, 006 and 007 or a later ADR. The current slice provides exact
activation candidate matching and dependency-topological activation, while
leaving selector expansion and runtime policy to those later contracts. It
establishes the registration owner, cleanup seam, projection and failure
isolation needed by those contracts.
