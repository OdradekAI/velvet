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
bytes through a typed CoreTransport DTO. ZIP archive packages now pass through
a separate bounded extraction adapter and then reuse the directory-package
transaction; other archive formats and non-localization binary resources remain
future adapters over the same seam.

The runtime declaration is one of `builtin`, `wasm` or `sidecar` in the alpha
manifest. A target manifest major also admits a declarative/no-runtime package
for contributions that need no executable code; it must not use a dummy
in-process adapter. `extensionKind` describes `ui`, `workspace` and/or
`capability` contributions, while the host selects the actual
`desktop-core` or `isolated-runtime` location. Runtime choice does not grant
permissions or expose a frontend framework. All runtime adapters use the same
contribution descriptors, lifecycle, capability grant and RPC contracts.

Runtime form and trust are separate fields. A built-in adapter is trusted
application code and shares the Core failure domain; it is allowed only for
first-party code shipped inside the signed application. WASM is the preferred
untrusted computation form because the host can grant explicit WASI/component
imports rather than ambient filesystem, network or process access. A sidecar
provides a process crash boundary, but it is not a security sandbox: native
sidecar code otherwise has the user's OS permissions. Marketplace or local
policy must therefore distinguish `trusted-builtin`, `capability-sandboxed`,
`process-isolated` and `unsigned-developer` trust states. An isolated UI
surface is a renderer form layered on top of any runtime; it never receives a
generic Tauri command channel. Tauri host plugins remain bundled privileged
integrations and are not product extensions.

The default failure domain is one runtime process or WASM instance per
extension. Jobs within an extension may share that extension runtime; two
extensions do not share a process unless a future policy explicitly advertises
the larger failure domain and passes instance-containment tests. A crash, trap,
timeout or resource kill therefore removes only the affected extension's
projections and Jobs by default.

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
permission grant as environment metadata. The host resolves launch targets
from the committed package and an allow-list; an external manifest cannot
choose an arbitrary absolute executable, inherited environment or unrestricted
working directory. The process is attached to a process group/parent-death
policy, has bounded stdin/stdout/stderr and reports exit code or signal.
Built-in extensions use the same lifecycle through a no-op process; WASM
extensions provide their declared component/module through a host-selected
runner. Invocation payloads and responses are typed by the negotiated runtime
protocol; they are not an untyped second manifest or transport contract.
The alpha `ProcessPluginRuntime` now captures a bounded stderr tail, reports
exit status and Unix signal evidence, and starts sidecars in a dedicated Unix
process group so shutdown can terminate descendants. These are availability
and diagnosis safeguards, not a security sandbox. Production qualification
still requires package-relative executable allow-lists, parent-death policy,
orphan reconciliation and equivalent process-group semantics on every
supported platform. The current manifest seam still accepts a launch program
directly so those trust checks remain an explicit follow-up rather than an
implicit guarantee.

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

`PluginHost` emits structured runtime events for start, ready, protocol
violation, timeout, WASM trap, budget rejection, exit code/signal and process
group termination. The event carries extension id, package version, runtime
session, request id and (when applicable) Job id/attempt. A bounded
`PluginDiagnosticSink` accepts extension logs and redaction hints; the host
validates, truncates and redacts them before a `DiagnosticBundle` can include
them. An extension cannot cause an asset payload, secret, environment dump or
unbounded path list to be exported by writing a log line.

Permission sets are checked against the declared semantic zone before a
manifest is registered. Startup failure marks only that extension as failed;
health polling removes a crashed extension from active contributions, and an
explicit recovery call may create a fresh runtime. A crash or failed restart
does not disable the Workbench or unrelated extensions.

The native composition root scans the application data directory's `plugins`
folder for JSON manifests at startup. Discovery is deterministic by filename,
and each parse or registration failure is returned in a report while valid
manifests remain registered for activation. Application package signing and
update integrity follow ADR-0013; extension publisher signing, revocation and
marketplace policy remain separate from this local discovery seam. The
application signer is not implicitly an extension publisher, and an unsigned
local extension requires an explicit developer-mode policy before activation.
Relative launch
arguments beginning with `./` or `../` resolve from the
manifest directory, which lets a sidecar package move as one directory without
embedding an absolute executable path.

Package trust is recorded independently from application release trust. A
stable or preview product-extension package must carry a signature binding its
digest, `ExtensionId`, publisher key id, version, target/runtime, manifest
schema, host API range and declared capabilities. The application signer is
not an extension publisher. Built-in extensions inherit the signed application
package; installed extensions require a separate publisher trust decision.
Developer mode may admit an unsigned local package only when it is explicit,
visible and profile-scoped. Key rotation, revocation, freshness and
anti-downgrade checks are enforced before activation; a valid old signature
does not override a revoked key or a newer retained generation. SLSA/in-toto
provenance and SPDX SBOM references are separate metadata and never replace
capability checks.

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

### Extension data, backup and migration

`PluginHost` owns one namespaced `ExtensionStorage` owner per extension. The
storage API is transactional and quota-bound; it exposes opaque key/blob
handles rather than the application database or an arbitrary path. An
extension may implement a `BackupParticipant` to export a bounded logical
snapshot of its own namespace and a `MigrationParticipant` to transform a
staged source generation into a staged destination generation. The host owns
the snapshot index, checksum, quota, conflict check and publication of the
resulting extension `StateSnapshot`. Core `RecoverySet` publication remains
owned by the core persistence path; extension storage never becomes a member
of that set.

The alpha implementation exposes this boundary through `plugin-host` over the
shared `StateSnapshotStore`. A manifest must declare `storage.schemaVersion`
and `storage.quotaBytes`; the host validates the declaration, derives the
owner namespace from the extension id, and offers transactional key/blob
updates with generation conflict detection. Backup export and migration
participants run before a destination generation is committed. The Rust
protocol DTO and desktop TypeScript projection carry the declaration, but
there is deliberately no raw Tauri storage command and no filesystem path in
the extension API. Runtime RPC invocation, user-facing extension
restore/quarantine actions and automatic disable policy remain follow-up gates;
bounded diagnostic history is now persisted by the application owner rather
than by the extension storage API.

`PluginHost` also owns a bounded `PluginDiagnosticSink`. Runtime start/crash,
invocation, Job, shutdown, storage and migration failures become ordered,
sanitized records with extension id, code, retryability and optional Job
attempt identity. The sink is local and drainable through the host seam; it
does not upload, expose raw payloads or grant another extension visibility into
the record. The application maps records to the typed `PluginDiagnosticEvent`
transport projection and persists a bounded redacted history through its
`DiagnosticEventStore`; the user-exported cross-component `DiagnosticBundle`
includes that history. Store corruption only marks diagnostics degraded and
does not block Core startup. Native crash capture and cross-process
correlation remain outside this sink.

Extension migration receives a read-only source generation and a host-owned
destination writer. It may not write Asset, Workspace, Profile or Job state,
call external network/process capabilities or publish a recovery pointer. The
host validates the output and references before commit. A participant failure
leaves the previous committed generation authoritative; a later lifecycle
coordinator may quarantine that extension generation and disable the extension
without touching core data. Rollback uses the previous validated package/data
generation, so reverse migration is optional. An older package can open a
newer extension schema only when its manifest explicitly declares that read
range and the host validates it.

Portable export is a separate user action from internal rolling recovery. It
excludes secrets, entitlement tokens, transient logs, caches and runtime
binaries by default. Uninstall retains extension state and snapshots until the
user explicitly confirms deletion. A plugin cannot silently delete its
recoverable state during deactivate, upgrade or uninstall.

### Entitlement and paid extensions

A package signature proves artifact integrity and publisher identity; it does
not prove purchase. Velvet does not expose a generic “paid signature” API. If a
future marketplace or account service is accepted, the host may expose a
read-only `EntitlementHandle` that verifies a signed entitlement bound to
extension id, product/feature id, issuer, account or installation scope,
sequence, not-before and expiry. The plugin receives only `active`, `trial`,
`grace`, `expired` or `unknown` plus bounded display metadata. Payment,
refunds, tax, merchant-of-record and payment credentials remain outside Core.

Entitlement state is allowed to degrade offline for a bounded grace period and
must never block core startup or destroy extension data. Until a marketplace
contract exists, the entitlement interface remains reserved/disabled. A
plugin may use its own licensing service only through an explicit network
capability; its self-asserted license is not Velvet-verified.

### Resource governance and runtime recovery

`PluginHost` applies a hierarchical device → extension → Job budget through a
versioned `ResourceBudgetPolicy`. The policy selects platform- and workload-
appropriate limits for isolated runtime memory, staging/temp disk, file
descriptors, child processes, network requests and concurrency. Platform
profiles may tighten these values; a manifest cannot widen them. CPU is
throttled through Job scheduling because percentage preemption is not portable;
memory, disk, file descriptors, process and concurrency are admission
boundaries. The alpha profile must be bounded and observable, but its numeric
values are release configuration rather than a plugin ABI promise.

Interactive runtime invocation keeps the existing five-second timeout. Job
invocations carry an owner deadline, progress heartbeat and cancellation
token. User cancellation is cooperative first; protocol v1 does not tear down
a healthy shared runtime for one cancelled Job. A deadline, malformed response,
crash or unhealthy runtime receives a bounded grace period and may then be
terminated as a process group. Built-in in-process adapters cannot be
force-killed and must remain cooperative. A timeout or crash removes only that
extension's projections, cancels or interrupts its Jobs and leaves the
Workbench running.

Recovery starts a fresh runtime and re-projects Rust-owned contributions.
Transient failures use at most three attempts with bounded exponential
backoff/jitter; permission, schema, invalid-input and deterministic provider
errors do not auto-retry. Three failures in five minutes or two failed
recovery attempts quarantine the extension until an explicit user action.

Before suspend, the host stops admitting background work, requests checkpoints
and persists Job state. After wake, local work resumes when its owner and
storage are valid; offline or metered-network work waits for an explicit
network-available policy event. Wake recovery is rate-limited and cannot
replay side-effecting commands.

When an isolated runtime crashes, its running Jobs become typed
`Interrupted`/`Failed` outcomes with a retryable decision and the host rejects
late progress/results by extension session, Job id, attempt and provenance.
Another extension's runtime and Jobs continue because they do not share the
default process. A built-in panic can be caught at a Rust boundary, but an
abort, memory-corruption fault or Core crash affects all built-ins. A forced
shutdown or power loss terminates all child runtimes; the next launch restores
committed extension generations and Job snapshots, marks in-flight attempts
interrupted and retries only when their contract permits. Sidecars must not
survive as unsupervised writers: process-group cleanup and orphan
reconciliation are required.

## Alternatives considered

1. Load arbitrary third-party Rust dynamic libraries into the Tauri process. Rejected because ABI, memory safety and crash isolation cannot be controlled by the host.
2. Treat all extensions as frontend bundles. Rejected because decoders, indexing, filesystem access and long jobs need a controlled Rust-side capability model.
3. Give every plugin the same unrestricted permission set. Rejected because a metadata panel should not gain network or process permissions by default.
4. Treat a sidecar process as a security sandbox. Rejected because a native
   child process normally inherits the user's OS authority; sidecars provide
   availability isolation only unless a platform sandbox is added.
5. Let plugins own global backup/restore or migration pointers. Rejected
   because a failed extension could corrupt core recovery or mix owner
   generations; the host must orchestrate extension-only participants.
6. Use a package signature as a paid-license proof. Rejected because package
   integrity, publisher trust, account entitlement and payment lifecycle have
   different issuers, revocation and privacy requirements.

## Consequences

Positive:

- Plugin installation can be explained by zone and capability.
- Heavy decoders and untrusted code are isolated from the desktop host.
- The host can enforce compatibility and recover from extension failure.

Costs:

- Sidecar and WASM boundaries add serialization and packaging work.
- Permission prompts and compatibility checks become part of the product UX.
- A public SDK needs explicit data, trust, diagnostics and entitlement types;
  convenience APIs cannot hide the fact that sidecars and built-ins have
  different security guarantees.

## Deferred decisions

- Is there a marketplace in the first release or only local installation?
- Which concrete package signature envelope (for example a COSE/DSSE-style
  envelope) and trust metadata format will the release service publish?
- Which platform adapters can enforce sidecar process groups, working-set and
  network limits, and which require admission/backpressure fallbacks?
- What is the minimum supported host API version and deprecation window?
- Which marketplace/account service, if any, will issue and revoke
  entitlements, and what is the offline grace policy?

## References

- [Plugin ecosystem lifecycle and recovery review](../research/plugin-ecosystem-lifecycle-and-recovery-review.md)
- [WASI security](https://wasi.dev/security)
- [Tauri security and capabilities](https://v2.tauri.app/security/)
- [VS Code extension runtime security](https://code.visualstudio.com/docs/configure/extensions/extension-runtime-security)
- [Eagle plugin review criteria](https://developer.eagle.cool/plugin-api/plugin-review/criteria/security-and-privacy)
- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)
