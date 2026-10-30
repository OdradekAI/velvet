# ADR-0001: System topology and core ownership

Status: Accepted
Date: 2026-10-28

## Context

Velvet is a desktop resource and information workbench. Its first built-in WorkbenchApps are:

- `asset-manager` (`Resource Manager`): asset libraries, collections, inspection and preview.
- `FeedWorkbench`: subscriptions, messages and reading workflows.

The application must keep a Rust core stable while the Tauri workbench, built-in features and third-party extensions evolve independently. The term C/S is still ambiguous: it may mean a desktop client with a local Rust service, or a multi-user client connected to a remote server.

## Decision

Use a local-first, single-deployment-unit topology for the first milestone. The Tauri webview and the Rust application core ship in one desktop process, but they communicate through an explicit transport interface:

```text
Vue/React workbench
        │ TauriTransport: typed commands, queries and events
        ▼
Rust ApplicationCore (same process in v1)
        ├── asset domain and persistence
        ├── layout state and migrations
        ├── preview resolver and capability registry
        ├── job system and indexing
        └── plugin host manager
                ├── built-in adapters (trusted, in-process)
                └── external extensions (WASM or sidecar process)
```

The frontend owns rendering, transient pointer/drag state and user input. Rust owns assets, sources, collections, facets, layout persistence, preview selection, plugin lifecycle, permissions, jobs and durable settings. The frontend never reads the database, filesystem or plugin runtime directly.

Durable state has one Rust owner per scope. Profile state owns shell Parts,
global ViewContainer/Companion/Operation state and user preferences; Workspace
state owns the active WorkbenchApp, app-scoped Views and recoverable AppSurface
state; Session/Window state owns focus, selection and restoration hints;
`PluginHost` owns namespaced extension storage and extension facets; the host
security store owns secrets; `Preview`/capability owners own their derived
caches; and `JobSystem` owns Job results, progress, cancellation and logs.
Persistence crates are adapters for these owners, not alternate domain owners.
Each durable field has a schema version, migration, invalidation rule and
recovery path. Extension removal cannot invalidate core Asset, Source,
Collection, layout or Workspace identity.

`CoreTransport` is the seam. `TauriTransport` is the first adapter; a future local-socket or remote adapter may implement the same commands, queries, events and opaque handles without changing the domain modules or plugin SDK.

The transport envelope is versioned and carries protocol/features,
`sessionId`, `requestId`, message kind, method, typed payload, revision or
sequence and deadline. Rust rejects stale revisioned writes with the current
authoritative snapshot. Duplicate requests are idempotent; duplicate events
are discarded and sequence gaps trigger bounded snapshot resynchronization.
Reconnect retains the last valid projection as stale, disables core-dependent
writes, and enters a diagnosable degraded state after bounded retries. Missing
or corrupt stores recover through safe defaults and owner diagnostics; the
frontend never repairs persistence directly.

Failure boundaries are part of the topology: a plugin, View provider,
AppSurface, Preview provider, Job, transport session or persistence document
may fail without taking down the shell or unrelated state. Each boundary has
a host-owned recovery action and a contract test through a fake adapter.

Do not split the Rust core into a separate local daemon in v1. A process split would add packaging, startup, crash recovery and IPC versioning before the product has a second client. The plugin runtime remains independently isolatable because untrusted extensions do not run inside the Tauri process.

Remote synchronization, authentication, tenancy and server deployment remain future adapters and require their own decisions.

## Alternatives considered

1. Put asset and plugin logic in the Vue/React frontend. Rejected because preview, indexing and permissions would be duplicated across clients and become difficult to test.
2. Start with a remote server as the source of truth. Rejected for the first skeleton because offline asset access and local file permissions are core use cases.
3. Put all Rust code in `src-tauri` without domain crates. Rejected because it makes the Tauri adapter indistinguishable from the domain implementation.

## Consequences

Positive:

- Rust provides one authoritative state model for desktop, CLI and future remote adapters.
- UI framework changes do not change the domain or plugin protocol.
- Long-running work can be cancelled and tested without driving a browser window.
- A future daemon or remote server can implement `CoreTransport` without changing domain modules.

Costs:

- The protocol needs explicit command, query and event types.
- Local-first storage must later reconcile with remote synchronization if that feature is added.

## Deferred decisions

- What exact remote C/S deployment is required after the local-first milestone?
- Which data is allowed to leave the device when a remote adapter is introduced?
- Are multiple desktop windows separate sessions or views of one core session?

## References

- [VS Code workbench layout service](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/layout/browser/layoutService.ts)
- [Spacedrive architecture](https://github.com/spacedriveapp/spacedrive)
