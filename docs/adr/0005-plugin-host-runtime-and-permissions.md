# ADR-0005: Plugin host runtime, permissions and lifecycle

Status: Accepted
Date: 2026-10-28

## Context

The product needs an ecosystem for format readers, previewers, inspector panels, feed connectors, AI actions and operational tools. Tauri plugins that expose host capabilities and user-installed product extensions have different trust and lifecycle requirements.

## Decision

Keep two extension layers separate:

```text
Tauri host plugins      system and desktop capabilities
Product extensions      workbench, preview and asset capabilities
```

Product extensions are declared by a versioned manifest and activated lazily. A
manifest may contain Workbench-level, WorkbenchApp-level and reusable
capability contributions, each with its own target container and permission
set. The default runtime for third-party code is an isolated WASM module or
sidecar process. Built-in extensions may use an in-process adapter, but they
must use the same public contribution interfaces.

Permissions are capability-based and zone-aware:

```text
navigation:
  workspace.open, library.read, source.connect

contextual:
  asset.read, metadata.read, preview.render, metadata.write

operations:
  jobs.create, jobs.read, logs.read, external-process, network
```

An extension receives capability handles rather than unrestricted file paths. Installation, activation, update, disable, uninstall and rollback are owned by `PluginHost`. A crashed or incompatible extension must not take down the workbench.

In version one, extensions cannot own arbitrary Workbench Parts or bypass the
fixed semantic Zones. After the `WorkbenchApp` contribution contract is
accepted, an extension may contribute a top-level `WorkbenchApp`, an
ActivityBar ViewContainer-only tool, or a reusable capability. These entries
still use host-owned layout, permissions and lifecycle rules; they do not create
an independent shell or data store.

The right `AuxiliarySidebar` is reserved for host-approved global
`CompanionView` contributions, and the bottom `OperationsPanel` is reserved for the shared JobSystem
and operation history. An app add-on does not gain access to either surface
merely because it runs inside that app.

Every extension identifier, contribution identifier and persisted facet schema is globally unique and versioned.

The canonical manifest contract is owned semantically by Rust `plugin-api` and
is mirrored explicitly by the `workbench-protocol` DTOs, the Tauri wire DTO,
the TypeScript `PluginClient` and external package manifests. A package uses a stable `ExtensionId` matching
`[A-Za-z0-9][A-Za-z0-9._-]*` (the recommended form is `publisher.name`) and a
SemVer package version. `manifestSchema`, `hostApi`, `bridgeApi` and
`runtimeProtocol` are explicit compatibility ranges. Display metadata such as
name, description, icon, categories and default locale is separate from
identity. Manifest decoding is strict: unknown fields are validation errors,
with only explicitly reserved `x-*` metadata available for forward extension.
The same fixture must round-trip through JSON, Rust, the Tauri mapping and the
TypeScript mirror.

When an extension contributes localized labels or runtime UI text, its manifest
may additionally declare a strict `localization` object with `defaultLocale`,
BCP-47 `supportedLocales`, a package-relative `resourceRoot` and a versioned
`bundleSchema`. Bundles are `<resourceRoot>/<locale>.json` (defaulting to
`l10n` only when the capability is declared); the default bundle is required,
and path traversal, duplicate ids, unsupported locale tags, malformed JSON or
schema mismatches are rejected before activation. Message ids are scoped as
`<extensionId>.<localMessageId>`, and contribution labels carry an id plus a
`defaultMessage`. The host resolves them through the same scoped localization
handle exposed to trusted extensions and the UI Bridge channel exposed to
isolated surfaces. Locale selection belongs to Profile/Session state and uses
the host fallback chain `requested exact locale → language subtag → extension
defaultLocale → defaultMessage`; a missing translation emits a diagnostic but
does not remove a contribution. Bundle caches and projections are cleaned up
with the owning extension and cannot affect core layout or Asset identity.
The effective locale is persisted by the Rust Profile owner under a revisioned
`ProfileState` protocol query/command; the desktop localization service only
hydrates and projects that state. Package/archive bundle loading is a
host-owned adapter and never grants the renderer direct filesystem access.
The directory-package adapter is now implemented as a bounded
`PluginResourceLoader`: it derives the installed package from the validated
extension id, permits only declared localization locales under `resourceRoot`,
rejects package or symlink escapes and oversized files, and returns resource
bytes through a typed CoreTransport DTO. Archive extraction and non-localization
binary resources remain future adapters over the same seam.

The runtime declaration is one of `builtin`, `wasm` or `sidecar`. Built-ins
are trusted adapters; third-party runtimes remain isolated. `extensionKind`
describes `ui`, `workspace` and/or `capability` contributions, while the host
selects the actual `desktop-core` or `isolated-runtime` location. Runtime
choice does not grant permissions or expose a frontend framework. All runtime
adapters use the same contribution descriptors, lifecycle, capability grant
and RPC contracts.

Activation is indexed by `onWorkbenchReady`, `onApp:<app-id>`,
`onView:<contribution-id>`, `onCommand:<contribution-id>`,
`onPreview:<selector>` and `onJob:<kind>`. It is lazy by default and
idempotent within a session. Hard `extensionDependencies` form a validated
acyclic graph and activate in topological order; missing or failed
dependencies block only their dependents. Optional integration uses capability
discovery without imposing activation order.

The manifest declares trusted, virtual-workspace and profile support, plus
required and optional capabilities. The host intersects those declarations
with trust/restriction mode and Profile/Workspace policy (`allow`, `prompt` or
`deny`) and sends the effective grant during activation and UI-surface
handshakes. P0 handles are namespaced context/theme/focus, declared command
dispatch and handoff, localization, extension storage, configuration and URI
access, plus explicitly approved Asset/Preview/Job registrations. Secrets,
metadata writes, filesystem, downloads, network and external processes are
separate capabilities. `ExtensionContext` exposes opaque scoped handles,
session cancellation, logging and localization; it never exposes unrestricted
paths, database access or host framework objects.

Built-in, WASM and sidecar adapters use one versioned RPC envelope containing
`protocolVersion`, `sessionId`, `requestId`, message `kind`, `method`, typed
`payload`, `deadline` and `sequence`. Typed errors, cancellation, bounded
timeouts and health messages are mandatory. Reconnect and restart create a
fresh session, re-send authoritative context/capabilities and re-project
validated contributions without replaying side-effecting commands. A crash,
timeout, malformed response or failed restart marks only that extension
failed, removes its active projections and leaves the Workbench and core data
usable. Disable and uninstall stop the runtime before idempotently removing
all host projections and UI sessions; extension-owned durable data follows its
retention/migration policy.

The first runtime integration keeps the process boundary behind two host-owned
interfaces: `PluginRuntime` starts an extension with an explicit
`RuntimeLaunchSpec` and `RuntimeProcess` reports health, shutdown and command
invocation. The production adapter uses a child process with the canonical
versioned JSONL envelope and passes only the extension id and the validated
permission grant as environment metadata. Built-in extensions use the same
lifecycle through a no-op process; WASM extensions provide their declared WASM
runner as the launch program. Invocation payloads and responses are typed by
the negotiated runtime protocol; they are not an untyped second manifest or
transport contract.

Declared Job contributions use the same runtime process through a distinct
`invoke-job` message with `method: "job.run"`. The host sends the Job input as
`args` and a bounded context containing Job id, kind, attempt and provenance.
The response contains a typed result (`contentType` plus JSON `payload`) with
bounds-checked input/result projections and a bounded list of `progress`
events. `PluginHost` exposes this process through a
thread-safe `PluginJobHandle`; `ApplicationCore` adapts the handle to the
Rust-owned `JobRunner` registry. Job cancellation is cooperative at the host
boundary in protocol v1: the runner checks cancellation before invocation and
after each returned progress event/result, and stale attempt guards prevent a
late response from committing. The host does not kill a shared runtime process
for an in-flight cancellation; an explicit runtime cancel message remains a
future protocol extension.

The host applies a bounded invocation timeout. A plugin-level rejection is
returned to the caller while the extension stays active. A closed or crashed
runtime, malformed response or timeout tears down only that process, marks the
extension failed and leaves it recoverable through the normal lifecycle path.

Permission sets are checked against the declared semantic zone before a
manifest is registered. Startup failure marks only that extension as failed;
health polling removes a crashed extension from active contributions, and an
explicit recovery call may create a fresh runtime. A crash or failed restart
does not disable the Workbench or unrelated extensions.

The native composition root scans the application data directory's `plugins`
folder for JSON manifests at startup. Discovery is deterministic by filename,
and each parse or registration failure is returned in a report while valid
manifests remain registered for activation. Package updates, signing and
marketplace policy remain outside this local discovery seam. Relative launch
arguments beginning with `./` or `../` resolve from the
manifest directory, which lets a sidecar package move as one directory without
embedding an absolute executable path.

The local install flow is a two-phase transaction. `inspect-plugin` parses the
manifest and returns the requested permissions, target app, runtime and host API
compatibility before any files are copied. `install-plugin` stages the package
under this directory; an existing extension is accepted only when its numeric
version increases. The host keeps the previous package as a hidden rollback
backup until the client has projected and activated the new extension, then
`commit-plugin-install` removes the backup. `rollback-plugin-install` removes a
new package or restores the previous package and runtime state. The native
Tauri composition root registers committed manifests and indexes their
activation events during client startup; it does not activate every extension
merely because it is installed. It scans for abandoned `.staging-*` and
`.rollback-*` directories left by a client crash and holds affected extensions
inactive until the host presents
resume, commit, rollback or discard actions. Other embedders may retain lazy
activation by calling the lifecycle commands explicitly.
Recovery failures are logged with an attempt number and remain retryable. Startup
removes only stale or invalid transaction directories that are not part of a
recoverable pair, records those cleanups, and exposes a bounded recovery log to
the native client. The desktop projects that log into OperationsPanel; filtering
is transient, while clear and JSON export remain typed host commands. Export
uses a user-selected native destination and does not make the frontend a second
persistence owner.

## Alternatives considered

1. Load arbitrary third-party Rust dynamic libraries into the Tauri process. Rejected because ABI, memory safety and crash isolation cannot be controlled by the host.
2. Treat all extensions as frontend bundles. Rejected because decoders, indexing, filesystem access and long jobs need a controlled Rust-side capability model.
3. Give every plugin the same unrestricted permission set. Rejected because a metadata panel should not gain network or process permissions by default.

## Consequences

Positive:

- Plugin installation can be explained by zone and capability.
- Heavy decoders and untrusted code are isolated from the desktop host.
- The host can enforce compatibility and recover from extension failure.

Costs:

- Sidecar and WASM boundaries add serialization and packaging work.
- Permission prompts and compatibility checks become part of the product UX.

## Deferred decisions

- Which extension packages may be signed, and who is trusted to sign them?
- Is there a marketplace in the first release or only local installation?
- What is the minimum supported host API version and deprecation window?
