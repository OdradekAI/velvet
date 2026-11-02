# Workbench foundation plan

Status: In progress; topology and desktop renderer decisions accepted, first React shell slice implemented; business-independent Workbench foundation is the active first-stage scope
Date: 2026-10-28

This plan establishes seams and ownership for the accepted foundation and the
proposed native-hardening records. It distinguishes deterministic kernel
evidence from platform and release qualification; a code seam is not a user
facing guarantee until the corresponding evidence level passes. The broad
architecture decisions are tracked under `.scratch/workbench-foundation/`; the
active first-stage shell decisions are tracked under
`.scratch/workbench-shell-foundation/`.

## Active first-stage scope

The immediate destination is the business-independent Workbench foundation, tracked in `.scratch/workbench-shell-foundation/` and its completion route in `.scratch/workbench-foundation-completion/`. It covers fixed Part docking and semantic Zones, VS Code-like visibility, collapse, resize and within-Zone ordering controls, Rust-owned layout persistence and migrations, host commands and overlays, selectable `WorkbenchApp` surfaces, shared `WorkspaceContext` projections, scoped plugin namespaces, reserved global companion/operations surfaces and the framework-neutral ViewContainer/View contribution seam. Built-in apps remain stable Activity Bar entries while accepted extensions may contribute app entries under the same host rules. Preview and Job are P0 capability seams with host-owned lifecycle and fallback; product-specific providers, indexing and marketplace policy remain later decisions.

The P0 foundation is editorless. It does not require an editor document
model, language provider registry or LSP layer. A future text-oriented
`WorkbenchApp` may opt into a separate text-language capability pack; that
pack must not become a hidden dependency of the Workbench shell, AppSurface,
Preview or Job infrastructure.

The AppSurface contract is explicit: trusted and isolated adapters share the
same negotiated lifecycle and typed capabilities; a new surface must be ready
before the host commits a switch; failures preserve a healthy current surface
and expose diagnostics/retry; reconnect resynchronizes the latest authoritative
context/theme/size/focus snapshot by session and sequence.

The View/Command/Overlay host contract is also fixed. View providers use one
host lifecycle for generic app content, with host-owned loading/empty/welcome/
error/badge/toolbar/menu/focus and versioned state restoration. Commands and
menus use typed DTOs, request ids, cancellation, typed errors and versioned
context expressions; Command Palette and Quick Input stay host-owned.
Keybindings are normalized multi-chord sequences resolved through layered
Profile/Workspace/Session configuration with app precedence and conflict
diagnostics. Dialogs, popovers, context menus, command search, quick input,
toasts, alerts and NotificationCenter share one host Overlay stack with focus
trap/restoration and live-region behavior. View movement and drag-and-drop
remain Rust `LayoutService` commands, and contribution removal repairs layout,
focus and dependent overlays without a second persistence owner.

The state/transport structure is fixed around one owner per durable scope:
Rust owns Profile, Workspace, Session/Window, Job and extension lifecycle
state; `PluginHost` owns namespaced extension data; capability owners own
derived Preview/facet caches; persistence crates are storage adapters; and the
desktop only consumes projections. `workbench-protocol` is the canonical DTO
layer, `tauri-adapter` is a thin mapping, and renderer/workbench modules cannot
create alternate persistence or transport owners. Every boundary uses
versioned schema, migration, invalidation, recovery and fake-adapter tests.
The envelope carries protocol/features, session/request ids, kind, typed
payload, revision/sequence and deadline. Stale writes reject with the current
snapshot; sequence gaps resync the latest snapshot; reconnect is bounded and
degraded-but-diagnosable; corrupt stores use owner safe defaults. Foundation
completion requires the failure matrix in the state/transport decision ticket,
not only a rendered-shell smoke test.

Protocol major versions must match while minor versions may add optional
fields/features. Payload schemas and feature ids are negotiated separately
through a typed handshake that assigns a fresh session and returns the
authoritative revisions/sequences. `requestId` is the idempotency key; unknown
mutation outcomes require an authoritative query before retry. Reconnect keeps
the last valid projection read-only, creates a new session and performs a full
snapshot resync without replaying side-effecting commands.

Plugin and Job execution uses device-global → owner → Job budgets. A versioned
resource policy selects platform- and workload-aware hard limits for memory,
disk, file descriptors, child processes and concurrency; CPU and network
throughput are soft budgets enforced by scheduling, byte accounting and
backpressure. Accepted Job claims are persisted in snapshots and exposed to
plugins as read-only context. Cancellation is cooperative: a healthy shared
runtime is not killed for one cancelled Job, while timeout or runtime failure
receives a bounded grace period before process-group termination. Typed
transient failures use finite exponential backoff. Suspend/wake and
offline/metered-network transitions persist Job checkpoints and pause/resume
eligible work without busy retries. macOS/Linux host signal delivery uses fixed
`scutil`/`nmcli` adapters, Tauri resume events and a bounded wake-gap fallback;
unknown evidence remains typed. Until a cross-profile coordinator exists, a
process-local device tier must be reported as such rather than claimed as a
strict cross-process limit.

Native integration uses a versioned Tier A/B platform matrix. Tier A covers
startup/recovery, WebView, keyboard/IME, accessibility, high-DPI windows,
file open/save and CoreTransport; Tier B capabilities degrade with diagnostics.
One Profile has one writer, multiple windows share one `ApplicationCore`, and a
second process forwards validated activation requests. Rust owns path,
symlink, volume, file-association, drag/drop and deep-link validation; missing
removable/network volumes preserve Asset identity and only make source work
temporarily unavailable.

Release integrity is part of the native foundation. The `preview` and `stable`
channels use signed `ReleaseManifest` metadata and platform-signed packages;
the pinned release root key authorizes key rotation, and the manifest carries a
supported data-schema range, minimum client version, monotonic release
sequence and freshness metadata. An offline installer uses
the same signature, hash, target, release-floor and downgrade checks as the
online updater. The host-owned Tauri updater adapter supplies transport and package bytes while
`ApplicationCore` verifies the manifest and owns the transaction policy. An
`UpdateTransaction` pairs the staged package with owner snapshots and a
copy-on-write migration: the previous package and committed generations stay
available until the new version reaches `ready`. Before that checkpoint a
failed or interrupted update returns to the old package/data generation; after
commit, recovery never starts an old binary against a newer schema. Uninstall
retains Profile, Workspace, Asset metadata, snapshots and diagnostics until the
user explicitly confirms data deletion.

## Native lifecycle/recovery gate

The accepted [Native Lifecycle and Recovery ADR](../adr/0013-native-lifecycle-and-recovery.md)
adds a host-level recovery gate to the foundation. It is ordered before
product-specific persistence, preview and indexing work so a corrupt optional
component cannot become a startup dependency.
The post-research [decision review](../research/native-lifecycle-recovery-review.md)
is part of this gate: it records the counterexamples that prevent the plan from
turning a helper process, fixed resource constants or a renderer-only recovery
screen into an accidental platform contract.

The implementation priority is:

1. **P0 startup safety**: preflight, checkpointed startup phases, lifecycle
   journal, two-failure Safe Mode, migration-failure Safe Mode, the minimal
   `RecoverySurface` and the standalone native recovery helper for fatal
   WebView preflight failures.
2. **P0 persistence safety**: owner-scoped rolling `StateSnapshot` generations,
   atomic replacement, migration staging, quarantine and restore of the last
   validated generation.
3. **P0 failure isolation**: PluginHost and JobSystem recovery, interrupted Job
   state, provenance checks and derived-cache invalidation.
4. **P0 diagnostics**: structured, bounded redacted local records, host
   diagnostics, cross-process crash correlation and user-exported
   `DiagnosticBundle` with separate upload consent.
5. **P0 native capability boundary and package wiring**: deny-by-default Tauri
   capabilities, explicit CSP, typed allow-listed commands, a minimal recovery
   capability set and target-specific launcher packaging.
6. **P1 protocol and reconnect**: feature/schema negotiation, typed
   incompatibility errors, bounded reconnect and stale projection handling.
7. **P1 platform/resource/performance policy**: sleep and network behavior,
   versioned hard/soft budgets, OS/WebView fallbacks, cold-start/idle baselines
   and lifecycle tests on each supported OS.
8. **P1 crash evidence and release recovery**: local crash-capture adapter,
   pinned-root manifest verification, durability-aware package/data staging,
   health-check commit and rollback rules that preserve user data. The
   host-owned `UpdateCoordinator` and Tauri updater adapter now provide typed
   check/download/install wiring; release activation now has target manifest
   generation/injection and fixed channel metadata wiring, but still gates on
   protected CI keys, platform signing/notarization, production key rotation
   and installer qualification.
   The development config intentionally has no updater endpoint or public key,
   so an unconfigured build reports `disabled` and cannot install.
9. **Optional platform adapter**: an out-of-process `RecoverySupervisor` or
   native relaunch adapter only after measurements and a proven requirement
   justify the extra artifact.

The default P0 topology does not ship a separate supervisor process. The
out-of-process implementation remains an adapter option behind the same
`RecoverySupervisor` contract, while crash evidence is a separate
`CrashCaptureAdapter` seam.

The plugin-specific lifecycle gate is recorded in the [plugin ecosystem
review](../research/plugin-ecosystem-lifecycle-and-recovery-review.md) and
ADR-0005. It is ordered as follows so SDK convenience does not outrun the
host's trust and recovery boundary:

1. **P0 contract shape**: strict manifest/handshake fixtures, declarative
   no-runtime contributions, runtime trust/sandbox policy, capability grants
   and per-extension failure-domain projection cleanup.
2. **P0 state and work**: namespaced `ExtensionStorage`, host-owned
   `JobSystem` handles, bounded `PluginDiagnosticSink`, extension checkpoint
   and migration participants, and quarantine of extension state without core
   reset.
3. **P0 runtime hardening**: package-relative/allow-listed launch targets,
   platform lifecycle containment and orphan sidecar reconciliation. The
   alpha adapter now binds discovered packages to a host-only package root,
   rejects absolute/package-escape launch targets and unlisted host commands,
   captures bounded stderr and exit evidence, and cleans Unix process groups;
   Linux also arms a parent-death signal before exec. macOS records
   `proc_pidinfo` process identity and uses process-group cleanup plus
   next-start reconciliation; Windows assigns each runtime to a host-owned Job
   Object with kill-on-close and records process creation time. These safeguards
   do not make a sidecar a security sandbox.
4. **P1 package trust**: signed extension package envelope, publisher trust
   record, revocation/freshness/anti-downgrade checks and developer-mode
   unsigned-install policy. Provenance/SBOM references are separate metadata.
5. **P1 ecosystem operations**: update/disable/uninstall retention, offline
   package admission, marketplace review/block-list hooks and SDK simulator
   fixtures.
6. **Deferred P1/P2 entitlement**: only after a marketplace/account service is
   selected, add a read-only host-verified `EntitlementHandle` with bounded
   offline grace. Package signatures never become payment proofs.

### Native hardening evidence levels

The acceptance matrix uses three evidence levels:

1. **Kernel pass** — deterministic Rust/Tauri/TypeScript contract tests and
   fault-injection fakes prove the owner, schema, typed error and recovery
   action.
2. **Platform-qualified pass** — the supported OS/architecture matrix exercises
   native process, filesystem, volume, WebView and power behavior. A
   cross-target compile or `not-configured` result is not a runtime pass.
3. **Release-qualified pass** — protected keys, signed artifacts, installer/
   updater channels, proxy/offline behavior, rollback and package wiring pass
   for the exact target and channel being claimed.

An existing `Partial` row becomes complete only when its required evidence
level passes for every declared target/channel, the user recovery action works
without the failed optional component, and the negative tests for duplicate
owners, broad Tauri capabilities, untyped commands, replayed updates, partial
restores and unbounded retries remain green.

### Current skeleton versus target contract

The two lifecycle reviews changed the implementation route, not the ownership
model. The current repository has a tested recovery/storage/runtime kernel and
several native adapters; extension package trust, logical portability,
operation safety and platform/release qualification are still incomplete. The
following matrix is the baseline for sequencing work. “Partial” means that a
useful seam exists but it must not be advertised as the target guarantee.

| Target contract | Current skeleton evidence | Remaining gap | Priority |
| --- | --- | --- | --- |
| `RecoverySupervisor`, startup phases and two-failure Safe Mode | `application-core` now records a versioned launch journal, phase/checkpoint/deadline state, shutdown evidence, platform preflight evidence and core-migration/optional-component/platform-initialization failures; Tauri probes WebView/GPU before the builder, catches builder/setup/run-loop failures into a bounded marker, correlates crash receipts to launch/runtime identities, consumes bounded OS crash-artifact metadata, and the desktop has a host-owned renderer RecoverySurface plus localized in-process and standalone multi-page native repair flows; the release artifact has a fixed-path launcher wrapper with a pre-spawn `LauncherAttempt`, bounded loader/exec/exit evidence and wrapper integration fixtures; target-specific package wiring is exercised by the release script and macOS debug bundle | A failure before the wrapper image can execute remains an OS/package-manager boundary; native minidump decoding, platform signing/notarization and installer rollback qualification remain | P0 (partial) |
| Owner snapshots, `RecoverySet` and migration rollback | Native Tauri composition wraps Layout/Profile/Job with `StateSnapshotStore`, publishes a core `RecoverySet` after owner changes, imports legacy files through composition-owned migration participants before Core construction, exposes `RecoverySetTransaction` for related owner writes, restores the previous set and quarantines an exact corrupt generation; atomic-only/required-fsync modes produce a persisted durability report projected through recovery and diagnostics; the release-qualification suite covers staged-write/pointer crash boundaries, unreadable-directory path collisions and the platform capability matrix; the native macOS/Linux harness adds abrupt-process and ACL/read-only probes | Physical power interruption, operator-supplied removable/network mounts and Windows runtime qualification remain release gates; the Linux Docker gate requires a running daemon; generic embedded constructors remain intentionally in-memory/legacy adapters for tests and embedders | P0 (pass, device qualification open) |
| Extension-owned data with host-controlled backup/migration | `PluginHost` exposes manifest-declared namespaced `ExtensionStorage` over `StateSnapshotStore`, bounded key/blob transactions with generation conflict detection, typed `BackupParticipant`/`MigrationParticipant` seams, internal `invoke-storage` runtime RPC, host validation and isolation tests, plus a metadata-only recovery projection and typed restore/quarantine actions; corrupt/future-schema generations can be quarantined from filename metadata without decoding payloads; no raw Tauri storage command is exposed | The generic release-qualification seam is covered; extension-specific ACL/volume and abrupt-process qualification remains a device gate | P0 (partial) |
| Per-extension runtime failure domain | `PluginHost` tracks one process per extension, binds discovered/installed packages to a host-only package root, rejects absolute/package-escape launch targets and unlisted host commands, cleans contributions and Jobs, and persists structured diagnostics; sidecars capture bounded stderr and exit evidence, clean a Unix process group on shutdown, and Linux arms a parent-death signal before exec. Native composition persists bounded starting/running/quarantined runtime leases, verifies Linux `/proc` identities, macOS `proc_pidinfo` identities and Windows process creation times, automatically closes confirmed stale leases, durably quarantines uncertain identities before optional activation, and exposes a typed query plus explicit close action that never terminates a process. Windows Job Objects kill descendants when Core disappears; macOS uses process-group cleanup and next-start reconciliation. | Direct in-memory registration remains a trusted composition seam; platform release qualification must exercise macOS abrupt-loss reconciliation, Windows Job Object descendants, permission-denied identity reads and PID reuse | P0 (partial) |
| Resource-governed, restartable Jobs | `JobSystem` persists snapshots/results and the accepted device→owner→Job `JobBudgetRequest` claim, enforces hard memory/disk/child-process/concurrency admission, exposes bounded soft CPU/network pressure, and projects policy/checkpoint/environment through Rust, Tauri and TypeScript. `PlatformSignalController` feeds macOS/Linux network state and resume/wake-gap transitions through the one typed environment command. | Runtime usage telemetry, platform-specific metering confidence, pre-suspend notification and native release qualification remain; pressure is intentionally bounded backpressure rather than OS-level throttling. | P1 (implementation pass; qualification partial) |
| Host diagnostics and user recovery | Plugin install recovery log, renderer recovery operations panel, bounded `RecoveryDiagnosticBundle` export, host-owned `PluginDiagnosticSink`, persistent redacted diagnostic history, `CrashCaptureAdapter`, launch instance/attempt correlation, runtime nonce evidence, bounded system artifact metadata, typed platform dump header metadata and launcher-attempt/failure evidence exist | A failure before the wrapper image can execute remains an OS/package-manager boundary; richer renderer crash UI remains | P0 (partial) |
| Versioned protocol handshake and reconnect | Rust/TypeScript/Tauri expose a typed major/minor handshake with stable `clientId`, replacement `sessionId` lifecycle, independent feature/schema negotiation and session validation; `query-snapshot`, monotonic event sequence/gap recovery, bounded session/client-scoped outcome records, declared-command replay and `query-request-outcome` are wired; the Workbench applies the authoritative Layout/Profile/Recovery/Job/Extension snapshot without replaying mutations; transport status drives read-only degraded presentation, tears down plugin/Preview sessions during recovery and recreates them after the snapshot | Non-Tauri transports remain responsible for adopting the same session, status and resync contract; richer per-command mutation gating remains a follow-up as new renderer commands are added | P1 (integration pass; non-Tauri adoption open) |
| Deny-by-default native boundary and package wiring | Tauri command handlers map typed requests; the main window capability explicitly lists event subscription, required child-WebView operations, dialogs and shortcuts and excludes `core:default`, filesystem, shell and opener permissions; a reserved `recovery-capability` is scoped to a future `recovery` window and grants only event subscription; explicit CSP keeps scripts self-only, IPC allow-listed, objects disabled and plugin frames bounded; Rust policy fixtures reject broad defaults and assert CSP/`externalBin`; the target-specific launcher is built by `beforeBuildCommand` and embedded beside the main executable | Per-platform package signing/notarization, installer permissions/rollback and Windows runtime qualification remain; any added capability or frame/network source still requires a negative policy test | P0 (partial) |
| Signed release staging and startup rollback | `ReleaseManifest` verification, private package staging, `RecoverySet` anchoring, health-check commit, typed rollback and launcher/Tauri pre-ready marker handling are implemented; a pending transaction is rolled back once on the next Core launch and the resulting `release-rollback` failure keeps the app in Safe Mode | Binary rollback before the application image can execute, platform installer signing/notarization, production key rotation and device qualification remain | P1 (kernel pass; updater host seam pass) |
| Tauri updater adapter and GitHub Release distribution | Host-owned `UpdateCoordinator` calls the official Tauri updater Rust API, matches the selected `latest.json` download/platform entry and reads its `velvetManifests[tauriPlatform]` (with base target fallback), validates the manifest before staging bytes, installs only after the Velvet journal is pending, and the next native launch commits `workbench-ready`; release config generation enables `createUpdaterArtifacts`, pinned public key, channel-specific HTTPS GitHub endpoint and signed version checks | The alpha CI lane now generates and signs one manifest per standard updater platform entry, injects the map after Tauri Action matrix publication, and publishes a fixed `preview` channel metadata tag; the same isolated path is reserved for a protected `stable` lane. Protected Tauri/Velvet keys, real signed package installation, proxy/offline and installer rollback qualification remain release gates; renderer updater permissions remain intentionally absent | P1 (implementation pass; publication qualification open) |
| Extension package trust and release recovery | Manifest/schema/host API checks and application release signing are separate seams | No extension signature envelope, publisher trust record, revocation/freshness/anti-downgrade or visible unsigned developer mode | P1 |
| Platform lifecycle and single-writer profile | `ProfileInstanceCoordinator` acquires a `create_new`/`sync_all` profile lock before WebView/Tauri/Core startup; the record carries PID, launch time, nonce and a platform start token; macOS/Linux verify `/proc`/`proc_pidinfo` identity before reclaiming an absent or reused owner, forward bounded schema-validated `ActivationRequest` frames over a Unix socket, store them in a bounded native queue and emit a typed wake event, and the desktop drains that queue through a typed command before handing requests to the Workbench; native activation normalizes existing regular files and `file://` URIs before forwarding; the Asset source owner owns canonical paths, Unix volume identity and a persisted `ApprovedSourceRoot` registry exposed through typed query/add/remove commands; the host Source Folders dialog provides refresh, native directory selection, explicit path approval and confirmed revocation; Workbench derives bounded, deduplicated `WorkspaceRoutingIntent` values without opening files or mutating layout and can activate a ready workspace before delegating an intent, retaining failed work for retry; `PlatformSignalController` maps macOS/Linux network probes and Tauri resume/wake-gap signals into the typed Job environment; `JobSystem` now owns versioned `StorageAvailability`/`PowerPressure` facts and pauses background work conservatively on non-writable/unknown storage or critical/unknown power | Real storage/power probes, removable/network mount qualification, Windows forwarding/identity verification and Tier A/B runtime qualification remain; denied or ambiguous identity inspection stays fail-closed | P1 (partial) |

This matrix also prevents a common false completion signal: an install
transaction rollback protects package bytes, but it does not protect extension
state; a Job snapshot marked `Interrupted` explains restart state, but it does
not make the work resumable; and a sidecar process boundary limits availability
failures, but it does not grant a security sandbox. The in-process
`RecoverySupervisor` remains the P0 implementation. A helper process, full
WASM component runtime, marketplace entitlement service and remote connector
are later adapters with separate evidence, not prerequisites for this kernel.

### Initial foundation build route (historical baseline)

The next work should follow dependency order and land as vertical slices with
fault-injection tests:

1. **P0 lifecycle kernel**: add the `RecoverySupervisor` trait, launch receipt,
   startup phase/deadline state machine, Safe Mode decision and the minimal
   recovery protocol/surface. Keep the journal failure-tolerant and make the
   recovery surface usable without the normal Workbench or a plugin.
2. **P0 durable generations**: introduce storage primitives for committed
   generations, checksums, atomic publication and quarantine. Build the
   recovery index and `RecoverySet` barrier, migrate Layout/Profile/Job stores
   through composition-owned participants, persist the durability report, and
   prove exact-generation restore/quarantine before adding new product data.
3. **P0 extension state and runtime evidence**: integrate the host-owned
   `ExtensionStorage` seam already backed by namespaced snapshots into the
   internal `storage.export`/`storage.migrate` runtime RPC and keep the
   restore/quarantine UX on the typed host seam. The host seam and desktop
   confirmation actions are now present. The runtime RPC sends only a bounded source snapshot;
   the host validates the response and commits against the source generation,
   so a malformed or failed participant cannot publish extension or core
   state. The sidecar adapter already captures bounded
   package-relative/allow-listed launch resolution, bounded stderr, exit
   evidence and Unix process-group cleanup; Linux also arms a parent-death
   signal before exec. The host now persists a bounded runtime lease before
   launch, commits the child identity after spawn, closes leases on normal or
   failed cleanup, and exposes conservative orphan outcomes that never kill
   by PID reuse alone. macOS and Windows identity/termination adapters are now
   present; keep the macOS next-start fallback and Windows Job Object
   descendant cleanup in platform release qualification. Wire
   failure events to projection/Job cleanup and late-event rejection.
4. **P0 Job governance**: extend Job submission and `JobSystem` with owner budgets,
   bounded deadline and cancellation-grace semantics, typed retry classes,
   checkpoint payloads and lifecycle/network pause signals. The `job-system`
   ledger now reserves device/owner/Job hard resources before admission and
   releases them on terminal or cancelling state; the Rust/Tauri/TypeScript
   policy and environment seams are wired and covered by contract tests. Keep
   CPU/network as bounded soft scheduling budgets. The accepted claim is now
   persisted and passed to plugin `job.run`; macOS/Linux `PlatformSignalController`
   feeds fixed network probes plus Tauri resume/wake-gap signals without adding
   a second scheduler. Keep OS metering confidence and pre-suspend behavior in
   release qualification rather than fabricating stronger guarantees.
5. **P0 recovery UX and native ACL**: keep the bounded `DiagnosticBundle`
   correlated with crash-capture evidence; the main window now uses an explicit
   allow-list, explicit CSP and a reserved host-only recovery capability set
   with command-negative policy tests. The target-specific launcher is built by
   `beforeBuildCommand`, embedded through `bundle.externalBin` and checked by
   the package qualification script before Tauri packaging. The launcher
   attempt marker, stale-attempt correlation, bounded macOS `.crash`/`.ips`
   metadata parser and actual wrapper integration script remain part of the P0
   seam; retain the OS-before-wrapper limit and package signing/notarization as
   explicit release boundaries.
6. **P1 canonical transport**: the Rust/Tauri/TypeScript slice now negotiates
   major/minor, features and schemas, retires replacement sessions, resyncs the
   authoritative owner snapshot after event gaps and handles bounded
   idempotency/query-before-retry outcomes. The Workbench now exposes transport
   status, enters renderer read-only mode during recovery, disposes plugin/UI
   Bridge and Preview sessions, and recreates them only after snapshot apply.
   Non-Tauri transports still need to adopt this status/session contract.
7. **P1 trust, platform and release**: the profile lock, macOS/Linux stale
   identity verification and activation forwarding seam now exists with a
   typed Rust/Tauri/TypeScript contract. The native seam now canonicalizes
   existing regular-file activation targets and file URIs. The Asset source
   owner now persists an `ApprovedSourceRoot` registry and exposes host-only
   query/add/remove operations; the native Source Folders dialog now provides
   visible refresh, browse, approval and revocation actions. Real
   removable/network mount qualification remains deferred for this slice;
   continue with workspace intent consumers,
   Windows forwarding/volume-identity verification, extension package
   signatures and trust records, crash evidence, staged package/data update
   rollback qualification and Tier A/B probes. The application release kernel
   is now implemented: pinned-root manifest verification, private package
   staging, `RecoverySet` data anchoring, health-check commit and typed rollback
   are covered by the acceptance rows below. Platform installer
   signing/notarization, production key rotation, real signed package
   installation/proxy qualification and Windows runtime qualification remain
   open.
8. **Deferred ecosystem services**: only after the local contract is stable,
   select a marketplace/account provider for read-only entitlement status,
   implement a production WASM/component adapter, and evaluate an optional
   out-of-process supervisor from measured crash/relaunch requirements.

The initial route above records how the current kernel was built. The remaining
native-hardening route below supersedes it for new work; passed slices should be
referenced rather than reimplemented.

### Remaining native-hardening route (2026-11-02)

1. **Contract and vocabulary (planning slice complete):** keep ADR-0014/0015,
   the ADR-0006/0007/0013 amendments and the glossary terms for logical
   bundles, operation receipts, safety notices and typed environment facts as
   the gates for implementation. Label kernel, platform and release evidence
   separately; do not promote a Proposed ADR from prose alone.
2. **P0 data and operation safety:** add typed read-only/full-storage policy,
   logical bundle inspect/plan/restore, selected-core `RecoverySet` import,
   extension quarantine, `OperationPlan`/`OperationReceipt` and unknown-outcome
   reconciliation. Add the duplicate-owner, partial-restore, stale-preview,
   untyped-command and unbounded-retry negatives.
3. **P0 identity and Job environment:** finish fail-closed lock/source actions,
   versioned storage/power environment facts and monotonic suspend/wake policy;
   preserve one Job scheduler and keep missing volumes owner-scoped.
4. **P0 boundary audit:** make capability/CSP/path/package/retry negative tests
   mandatory in CI. Keep sidecar availability isolation distinct from security
   sandboxing and keep Tauri commands path-free.
5. **P1 protocol and extension trust:** adopt the handshake/session contract
   across every transport, attach schema ids to open payloads, implement
   publisher signatures/trust records/revocation/developer mode and retain the
   old extension package/data pair until health commit.
6. **P1 platform and release qualification:** run Windows/macOS/Linux identity
   and runtime gates, real mount/power/proxy/offline tests, protected signing
   keys, installer retention, updater rollback and performance baselines for
   each claimed package/channel.
7. **P2 ecosystem adapters:** only after the local route is green, add
   authenticated remote authority, shared projections, marketplace
   entitlements, richer WASM sandboxing or an external supervisor.

Each step needs its acceptance row and user recovery action before the next
boundary is claimed. P0 is the alpha safety gate; P1 is per-target/channel
production qualification; P2 is deferred ecosystem scope.

The current React shell is a renderer slice, not the completed foundation. The
Part/mode, three-scope layout state, WorkbenchApp/AppSurface/shared context,
contribution scope/container, ViewContainer/View, namespace/lifecycle,
command/menu/overlay, UI Bridge and canonical transport decisions are accepted.
The desktop and Rust skeletons now have framework-neutral app and contribution
registries, lazy surface/plugin activation, app-scoped context commands,
host-container descriptors, namespace validation, target API checks,
contribution cleanup, host-owned command/menu/context-key/overlay registries,
isolated surface sessions, revisioned layout commands and protocol envelopes.
The host now also has framework-neutral keybinding resolution and focus
restoration state, leaving keyboard events and DOM focus as renderer adapters.
ViewContainer order/visibility and View moves are represented in the same
revisioned layout state, so extension removal can detach contributions without
destroying the host's remaining order.
The framework-neutral `TauriCoreTransport` bridge owns request/event
correlation and typed native transport errors. The real Tauri application under
`apps/desktop/src-tauri` now implements the command/event serialization seam,
publishes authoritative layout snapshots and forwards native global shortcuts
into the host keybinding registry. The React renderer selects this composition
root inside Tauri and keeps the browser DemoApp as a local fallback. The shell
now exposes roving Activity Bar focus, semantic resize separators with bounded
Arrow/Home/End keyboard behavior, stable Part ids and labeled contributed view
regions. The DOM focus adapter observes target lifecycle changes and restores
the previous host focus when a view or Part is hidden or unmounted. The pure
keyboard resize contract is covered alongside the existing
Activity Bar navigation tests. The native renderer gate is covered by the
ReactDOM contribution-panel regression suite below, including isolated
activation failure, retry and cleanup. A failed app or contributed surface is
kept inside its host with an explicit retry path; it does not trigger an
automatic retry loop or replace the Workbench shell. Plugin view
contributions now map to the framework-neutral `ViewRegistry` with stable
extension-prefixed ids. A
`view` contribution must name its local `ViewContainer` and isolated UI Bridge
entrypoint; global `companion-view` and `operation-view` contributions receive
host-owned synthetic containers in their reserved Parts. The React adapter
creates a `PluginSurfaceSession` only while a contributed view is mounted,
activates it after the bridge handshake begins, forwards context/theme/size
updates and disposes it before the host unregisters the contribution. Adapter
binding remains an isolated runtime concern, so an unavailable adapter leaves
the view in a recoverable waiting state.

The native Tauri composition now injects a JSON layout store at
`app_data_dir/workbench-layout.json`, and a restart test proves that the Rust
core restores the persisted revisioned snapshot. Missing files start from the
default layout; malformed files are rejected by the storage adapter and the
core starts from its safe default until a later layout write replaces them.
Runtime layout command failures now stay inside the mounted Workbench as an
announced, dismissible error; the renderer refreshes from the Rust snapshot
after a failed mutation instead of replacing the whole shell with a startup
failure page.
The React shell now mounts registered ViewContainers and Views by semantic Zone
and Scope, uses persisted order/visibility when present and falls back to the
descriptor defaults. Trusted React renderers have a separate adapter registry;
isolated plugin surfaces remain behind the UI bridge.
After a layout snapshot is loaded, the Workbench idempotently registers missing
ViewContainers and Views with Rust through `upsert` commands. Existing layout
entries are preserved, so plugin activation or renderer restart does not reset
user visibility, order or compatible View moves.
The Activity Bar now projects contributed navigation ViewContainers for the
active WorkbenchApp. Selecting one reveals its persisted container and the
Primary Sidebar, moves focus to that Part and uses roving keyboard navigation;
selection itself remains transient so restart recovery stays owned by the Rust
layout snapshot.
Contributed ViewContainers and Views now expose host-owned right-click actions
for visibility and within-zone ordering. Those actions dispatch the existing
revisioned layout commands, so hidden or reordered contributions remain owned by
the Rust layout service and recover through the same persistence path.
Context-menu and dialog contributions now use the host `MenuRegistry` and
`OverlayRegistry`; dialog surfaces are mounted through the isolated UI Bridge
only while an overlay is open, command handlers bind through the plugin
runtime seam, and namespace/lifecycle cleanup is tested.
Plugin-owned `WorkbenchApp` manifests now register extension-prefixed app
descriptors in `WorkbenchAppRegistry`. Their Activity Bar entries are projected
through the same registry as built-in apps, while the selected AppSurface is
created as an isolated UI Bridge session, suspended on app switch and disposed
before deactivation removes the app namespace. Namespace ownership and full
app cleanup are covered by the plugin lifecycle tests. The native WorkspaceArea
now renders a real AppSurface host, including measured resize/context
forwarding, app-switch suspension/reactivation, disabled-app fallback and
UI-Bridge session restart coverage.
The default surface adapter now mounts a sandboxed iframe in browser mode and
uses a positioned Tauri child WebView for HTTP/`tauri-webview://` entrypoints,
with explicit capability permissions and cleanup on deactivation.
Built-in trusted apps now pass through the same AppSurface registry lifecycle:
the React adapter projects context revisions and measured size, keeps failed
surfaces retryable and suspends the previous surface during app switches.
The desktop regression suite now mounts `WorkbenchShell` through ReactDOM in a
browser-like DOM and verifies contributed panel markup, focus restoration,
resize keyboard commands, context-menu actions, isolated PluginSurfaceHost
mount/dispose behavior and WorkbenchAppSurfaceHost app switching, suspension,
reactivation, activation failure isolation, retry and plugin deactivation
cleanup. The native surface adapter also exposes a narrow Tauri runtime seam
for regression tests covering child WebView creation, bounds updates, bridge
events, focus races, failed creation cleanup and session restart recovery.

The external plugin runtime seam is now connected to the Rust application core.
`PluginHost` validates zone permission sets before registration, starts
declared WASM runners or sidecars through `ProcessPluginRuntime`, tracks
registered/starting/active/disabled/failed states, and exposes health polling
plus explicit recovery. A failed startup or crashed process removes only that
extension's active contributions; a later recovery creates a fresh process.
The Tauri setup scans `app_data_dir/plugins` for JSON manifests at startup and
registers valid manifests lazily; malformed or rejected files are reported
individually without blocking the other plugins. The Tauri response exposes
the same runtime state through the extension query.
The versioned CoreTransport now exposes plugin manifest registration, activation,
disable, health-check and recovery commands. `PluginClient` keeps the
authoritative Rust extension summary in a separate runtime projection and the
native renderer performs a best-effort inventory query during startup, leaving
local contribution and UI-bridge registration explicit.
Rust-discovered active contributions now have a projection query carrying
ViewContainer/View metadata, namespaces and UI Bridge entrypoints. The native
renderer reuses `PluginClient` to project those entries into `ViewRegistry` and
`PluginUiRegistry`; disable/failure removes them and recovery re-registers the
same validated projection. Invalid namespaces remain isolated at the frontend
validation boundary.
Command, context-menu and dialog contributions use the same projection and
cleanup boundary, entering `CommandRegistry`, `MenuRegistry` and
`OverlayRegistry`. Projected command execution now returns through the
versioned CoreTransport as `invoke-plugin-command`; Rust validates the active
extension and declared command before forwarding a versioned JSONL request to
the isolated runtime. Runtime rejection stays local to the command, while a
closed, malformed or timed-out runtime is failed and recoverable without
removing unrelated extensions.

The canonical plugin contract is now fixed across Rust `plugin-api`, the
versioned `workbench-protocol`, the TypeScript `PluginClient` and external
package manifests. It includes strict schema and host API ranges, stable
extension identity and metadata, explicit builtin/WASM/sidecar runtime
adapters, `ui`/`workspace`/`capability` kinds, event-indexed lazy activation,
acyclic dependency ordering, target-app and trust/Profile/virtual-workspace
constraints, required/optional capability grants, and namespaced
`ExtensionContext` handles. `plugin-api` owns the semantic Manifest; the
protocol exposes an explicit `PluginManifestDto`/projection DTO, Tauri maps its
wire DTO explicitly, and TypeScript keeps a checked-in mirror. All adapters
use the same typed RPC envelope with session/request ids, sequence, deadlines,
cancellation, health, fresh-session recovery and idempotent cleanup. Canonical
JSON round-trip fixtures cover Rust, protocol, Tauri and desktop projection
paths; this work does not add an Editor or LSP dependency.

The repository now includes a runnable `examples/plugins/echo-sidecar` fixture.
Its Rust integration test covers manifest discovery, relative sidecar launch,
command JSONL invocation, UI metadata projection, intentional crash isolation
and recovery. The desktop regression test consumes the same manifest, invokes
the real Node sidecar, mounts its data URL through the UI Bridge, removes the
surface on failure and remounts it after recovery.
The native Tauri composition root now exposes a preflight and transactional
package flow through CoreTransport. `inspect-plugin` parses and validates a
manifest before copying anything, so the native Workbench `Extensions` dialog
can show requested permissions, runtime, target app, host API compatibility and
whether the operation is a new install or an upgrade. `install-plugin` stages a
newer package in `app_data_dir/plugins`; the frontend activates and projects it,
then sends `commit-plugin-install`. If activation or projection fails,
`rollback-plugin-install` restores the previous package and runtime state (or
removes a new package). The composition root discovers committed packages again
after restart. Startup scans for abandoned staging and rollback directories;
affected extensions stay registered but inactive until the dialog offers resume,
keep new, restore previous or discard actions. The dialog also uses native
directory selection and confirmation and exposes enable, disable, recovery and
uninstall actions. Failed recovery actions remain retryable, and startup cleans
orphaned transaction directories while the host exposes a bounded recovery log
for the dialog to display. OperationsPanel consumes the same log projection and
supports transient plugin/status/action filters, idempotent clearing and native
JSON export through the CoreTransport boundary.

The current shell slice also renders the accepted Part constraint table: fixed
Parts use stable dimensions, resizable Parts enforce bounded sizes in Rust, and
visibility/collapse commands are exposed through the host-owned workspace
context menu. This is an interaction seam for the eventual Tauri adapter, not a
second layout persistence owner.

## Target shape

```text
apps/
  desktop/
    src/
      app/
        desktop-app.ts
        index.ts
        renderer/
          main.tsx
          demo-app.tsx
          workbench-shell.tsx
          recovery-surface.tsx
        ui/
          velvet-ui.tsx
          tokens.css
      platform/
        protocol.ts
        index.ts
      workbench/
        workbench.ts
        parts/
          titlebar/
          activitybar/
          primary-sidebar/
          workspace-area/
          auxiliary-sidebar/
          operations-panel/
          statusbar/
          overlay-layer/
        zones/
          navigation/
          contextual/
          operations/
        layout/
          layout-model.ts
          layout-service.ts
          layout-state.ts
          layout-migrations.ts
        recovery/
          recovery-client.ts
        views/
          view-descriptor.ts
          view-container.ts
          view-registry.ts
          view-pane.ts
        preview/
          preview-host.ts
          preview-context.ts
          preview-intents.ts
          preview-registry.ts
          preview-resolver.ts
          preview-fallback.ts
        commands/
          command-registry.ts
          keybinding-registry.ts
          menu-registry.ts
        services/
          selection-context.ts
          workspace-context.ts
          extension-context.ts
      workspaces/
        index.ts
        asset-manager/
          descriptor.ts
        feed-workbench/
          descriptor.ts

packages/
  workbench-sdk/
    src/
      views.ts
      commands.ts
      preview.ts
      jobs.ts
      permissions.ts
  plugin-sdk/
    src/
      manifest.ts
      protocol.ts
      capability-handles.ts
  ui-sdk/
    src/
      host-element.ts
      message-channel.ts
      theme.ts

crates/
  application-core/
    src/
      lib.rs
  workbench-protocol/
    src/
      command.rs
      query.rs
      event.rs
      handle.rs
      error.rs
  layout-model/
    src/
      parts.rs
      zones.rs
      layout_state.rs
      migrations.rs
  asset-core/
    src/
      asset.rs
      content_kind.rs
      facet.rs
      source.rs
  preview-core/
    src/
      intent.rs
      provider.rs
      augmentation.rs
      resolver.rs
      context.rs
  plugin-api/
    src/
      manifest.rs
      contribution.rs
      permissions.rs
      version.rs
  plugin-host/
    src/
      registry.rs
      lifecycle.rs
      activation.rs
      isolation.rs
      capability.rs
  job-system/
    src/
      job.rs
      scheduler.rs
      progress.rs
      cancellation.rs
  persistence/
    src/
      repositories.rs
      migrations.rs
      facet_store.rs
      snapshot.rs
      recovery_set.rs
      owner_snapshots.rs
      durability.rs
  tauri-adapter/
    src/
      lib.rs

extensions/
  builtin/
    image-preview/
    generic-file-inspector/
    asset-manager/
    feed-workbench/
  examples/
    png-metadata/
    raw-inspector/

docs/
  adr/
  engineering/
  plugin-sdk/
```

`app/` is the composition root. `platform/` owns the transport-facing types and adapters without importing Vue or React. `workbench/` contains spatial infrastructure and host registries. Built-in `WorkbenchApp` descriptors live under the sibling `workspaces/` directory; external apps register through a host-owned registry. An app surface and its ViewContainers must never take ownership of `parts/`; they are registered through host registries.

The frontend boundary intentionally stays inside `apps/desktop` while there is one client. Extract a named root package only after a second frontend or independently versioned SDK consumes the same capability. This keeps the vben-inspired application/package distinction without creating placeholder packages.

The `packages/` and `extensions/` entries in the target shape are deferred extraction targets. They describe the public SDK and isolated extension packaging that will be introduced after the host contracts have a second consumer; they are not required directories in the current skeleton.

The first code skeleton keeps each Rust crate's public seam in `src/lib.rs` so the dependency graph stays small while the interfaces are being validated. Split a crate into the finer-grained files shown above only when a second implementation or a meaningful internal seam appears.

The desktop renderer currently uses React with TypeScript and Vite. The
framework-independent Workbench remains the host contract; `app/renderer` and
`app/ui` are React adapters. The first-party UI layer owns Velvet wrappers and
semantic tokens, while external extensions use the framework-independent plugin
SDK and an isolated UI surface.

Avoid generic `common/`, `shared/`, `utils/` and `services/` dumping grounds. A module earns a directory when it owns a meaningful interface, state model or adapter.

## Interface skeletons

These are shape sketches for the first implementation, not final public APIs.

### Layout model

```rust
pub enum WorkbenchAppId {
    ResourceManager,
    FeedWorkbench,
}

pub enum WorkbenchZone {
    Navigation,
    Contextual,
    Operations,
}

pub enum WorkbenchPart {
    TitleBar,
    ActivityBar,
    PrimarySidebar,
    WorkspaceArea,
    AuxiliarySidebar,
    OperationsPanel,
    StatusBar,
    OverlayLayer,
}

pub struct LayoutState {
    pub schema_version: u32,
    pub revision: u64,
    pub active_app: WorkbenchAppId,
    pub parts: PartState,
    pub containers: ViewContainerState,
    pub views: ViewState,
}

pub trait LayoutService {
    fn snapshot(&self) -> LayoutState;
    fn apply(&mut self, command: LayoutCommand) -> Result<LayoutState, LayoutError>;
    fn migrate(&self, state: LayoutState) -> Result<LayoutState, LayoutError>;
}
```

`LayoutService` is the only module that writes layout persistence. Views receive a read-only projection and dispatch typed layout commands.

### View contributions

```rust
pub struct ViewContainerDescriptor {
    pub id: String,
    pub title: LocalizedText,
    pub icon: IconRef,
    pub zone: WorkbenchZone,
}

pub struct ViewDescriptor {
    pub id: String,
    pub container_id: String,
    pub title: LocalizedText,
    pub default_visible: bool,
    pub when: Option<ContextExpression>,
}

pub trait ViewRegistry {
    fn register_container(&mut self, descriptor: ViewContainerDescriptor) -> Result<(), RegistryError>;
    fn register_view(&mut self, descriptor: ViewDescriptor) -> Result<(), RegistryError>;
    fn move_within_zone(&mut self, view_id: &str, index: usize) -> Result<(), RegistryError>;
}
```

The registry validates that a contribution stays inside its declared zone.

### Preview protocol

```rust
pub enum PreviewIntent { Quick, Inspect, Analyze }

pub enum PreviewSurface { Workspace, Thumbnail, Inspector }

pub struct PreviewRequest {
    pub asset_id: AssetId,
    pub intent: PreviewIntent,
    pub surface: PreviewSurface,
    pub profile: ProfileId,
}

pub struct PreviewCandidate {
    pub provider_id: String,
    pub content_kinds: Vec<ContentKindId>,
    pub intents: Vec<PreviewIntent>,
    pub priority: i32,
}

pub trait PreviewResolver {
    fn resolve(&self, request: &PreviewRequest) -> Result<ResolvedPreview, PreviewError>;
}

pub trait PreviewProvider {
    fn describe(&self) -> PreviewCandidate;
    fn open(&self, context: PreviewContext) -> Result<PreviewSession, PreviewError>;
}

pub trait PreviewAugmentation {
    fn attach(&self, context: PreviewContext) -> Result<Vec<Contribution>, PreviewError>;
}
```

The preview protocol is a P0 host capability. `Quick` now has a host-owned
overlay entry point, while `Inspect` and `Analyze` may render in an
`AppSurface`, inspector or another declared preview surface. Rust filters
providers by content kind, intent and target surface, orders candidates
deterministically and continues through a failing provider to a lower-priority
or generic fallback. The desktop `PreviewHost` projects typed resolving,
loading, ready, failure, close and dispose states. A provider owns primary
format content; the host owns the frame, lifecycle, fallback, theme, focus and
failure states. This protocol does not imply an Editor.

The accepted PreviewHost contract uses a session id, opaque Asset/Context
handles, theme/locale/size/focus inputs, cancellation and deadline. Trusted
built-in and isolated plugin providers return the same framework-neutral
session descriptor. Resolver selection is deterministic; augmentations attach
to declared slots without replacing the primary provider; decode and analysis
flow through JobSystem; and provider/capability caches carry provenance and
invalidate on content/provider/decoder/settings changes. A provider or
augmentation failure leaves generic fallback and unrelated Workbench state
usable.

The current implementation slice proves the isolated-plugin path: the
canonical Rust manifest carries preview selectors and slots, `PluginHost`
dispatches `preview.open` through the same timeout/crash boundary as commands,
`ApplicationCore` performs deterministic candidate selection and descriptor
projection, Tauri carries renderer/session metadata, and the desktop
`PreviewRegistry` cleans provider and augmentation registrations on deactivate
or crash. A provider still cannot replace the host-owned frame; actual
format-specific decoding and long-running analysis remain product JobRunner
consumers of the JobSystem seam.

The renderer mount seam is also present in the desktop skeleton:
`PreviewRendererRegistry` selects a host renderer factory and
`PreviewRendererHost` mounts a transient UI Bridge session inside the
host-owned overlay. Renderer factories are injectable and disposable, so a
future built-in image/model renderer can be added without changing
`PreviewHost`, plugin lifecycle or the static UI contribution registry.

The Rust preview core now also defines the `PreviewAssetSource` seam for
built-in providers. It clamps read requests to a host limit and returns typed
text/bytes content or a typed source error. The application core installs the
adapter, while preview providers receive no path or persistence handle.

The current `application-core` composition includes a bounded in-memory source
adapter that a composition or contract test can inject, and trusted
text/image providers as the first executable consumers. Their renderer
projections are explicit Rust → Tauri → TypeScript DTOs: text is rendered by
the host as bounded text, image bytes are rendered through a host-owned Blob
URL, and a truncation flag remains visible. A missing source or provider error
falls back to the generic preview with a diagnostic. The versioned persistent
source-location resolver now registers canonical regular files by opaque
`AssetId`, persists declared content kind and optional source fingerprints,
and exposes an AssetId-only Preview query; paths remain inside the Rust source
adapter and corrupt or missing locations stay isolated. Format-specific
decoding, richer source reconciliation and product-specific cache consumers
remain follow-up work.

The source seam now also exposes an optional complete-content
`ContentFingerprint`. Asset/source owners provide it without exposing paths;
PreviewResolver only forwards the query, and preview-created Jobs copy it into
`JobProvenance.inputFingerprint`. A missing value is a deliberate cache miss,
and replacing source content through the in-memory adapter clears an old
fingerprint unless the caller supplies a new one.

Preview augmentation execution now uses a separate `preview.augment` runtime
method. Rust resolves matching capability augmentations independently from the
Primary provider, preserves one diagnostic per failed augmentation and projects
their optional sessions through `augmentations[]`. The desktop overlay keeps a
failed or waiting augmentation inside its declared slot while the Primary
preview stays mounted; UI Bridge augmentation sessions reuse the same transient
renderer host as Primary sessions.

Preview session command ids now use the existing host CommandRegistry. The
renderer sends a typed `PreviewActionContext` through the dedicated
`invoke-preview-action` request; Rust invokes the plugin runtime through
`preview.action`, validates the typed accepted/job result and checks
`jobs-create` before submitting a deferred action to the shared JobSystem.
Job snapshots and events remain Rust-owned and are consumed by the existing
JobClient/OperationsPanel projection. Rust binds provider, asset, content kind,
intent and surface to the active session. Action failures, malformed results
and permission denials stay local to the action and do not tear down the
preview session or Workbench shell; an actual runtime crash follows the normal
plugin failure path. PreviewHost releases its bounded session
authorization when a session is replaced, closed or disposed, preventing late
action invocations from reusing an old asset projection.

### Optional text-language capability

Language is not a P0 Workbench service. A future text-oriented application may
opt into a capability pack whose first known need is syntax highlighting.
Language identity is stable: core languages use reserved ids, plugin-defined
languages use `extensionId/localId`, and plugins may explicitly augment an
existing id. Aliases are localized labels and language resources are owned by
the contributing extension.

The highlighting resource format, selector grammar and JSON/Markdown fixture
remain deferred. The editor document model, provider registry, diagnostics,
completion, hover, definition, formatting, snippets, schema validation,
semantic tokens, document versions, cancellation rules and LSP are explicitly
outside the current foundation and must not become hidden dependencies of the
Workbench, AppSurface, Preview or Job seams.

### Plugin host

```rust
pub struct ExtensionManifest {
    pub id: ExtensionId,
    pub version: Semver,
    pub host_api: VersionRange,
    pub contributions: Contributions,
    pub permissions: PermissionSet,
}

pub trait PluginHost {
    fn discover(&mut self) -> Result<Vec<ExtensionManifest>, PluginError>;
    fn activate(&mut self, id: &ExtensionId) -> Result<ExtensionSession, PluginError>;
    fn disable(&mut self, id: &ExtensionId, reason: DisableReason) -> Result<(), PluginError>;
    fn uninstall(&mut self, id: &ExtensionId) -> Result<(), PluginError>;
}
```

### Framework-independent UI SDK

```ts
export interface ViewDescriptor {
  id: string;
  containerId: string;
  zone: 'navigation' | 'contextual' | 'operations';
  title: LocalizedText;
  defaultVisible?: boolean;
}

export interface PreviewContext {
  assetId: string;
  contentKind: string;
  intent: 'quick' | 'inspect' | 'analyze';
  surface: 'workspace' | 'thumbnail' | 'inspector';
}

export interface CommandDescriptor {
  id: string;
  title: LocalizedText;
  when?: ContextExpression;
  requiredPermissions: Permission[];
}
```

### Plugin localization

Localization is a host capability, separate from display-language packs and
the optional text-language capability. An extension that contributes localized
labels declares a strict versioned manifest object:

```json
{
  "localization": {
    "defaultLocale": "en",
    "supportedLocales": ["en", "zh-Hans"],
    "resourceRoot": "l10n",
    "bundleSchema": 1
  }
}
```

Bundles are package-relative `<resourceRoot>/<locale>.json` files; the default
bundle is required. The host validates locale tags, paths, schema, duplicate
message ids and argument shapes before activation. The desktop
`LocalizationService` now owns locale revision, bundle registration and the
typed message handle; package/resource loading is injected through a loader
seam, and active isolated UI Bridge sessions receive the selected bundle and
locale revision through `host.locale`. Message ids are scoped as
`<extensionId>.<localMessageId>`. Manifest labels carry an id and
`defaultMessage`, and runtime strings use a scoped `l10n.t(id, defaultMessage,
args)` handle. Trusted UI receives that handle directly; isolated UI receives
the same semantic service through the UI Bridge with a bundle/message channel,
session id and locale revision.

Profile/Session owns the effective locale. The host service resolves
`requested exact locale → language subtag → extension defaultLocale →
defaultMessage`, records missing/invalid translations as diagnostics and keeps
the contribution visible. The initial message format is a constrained ICU-like
subset with named arguments, number/date formatting and `plural`/`select`;
invalid argument shapes fall back per message. Locale changes re-project
manifest labels and notify active Views, AppSurfaces, Previews, Overlays and
Job UI. Bad bundles or bridge updates are isolated to the affected extension,
bundle or message; caches are cleaned on disable, upgrade and uninstall.

## Implementation sequence

1. **Documentation and vocabulary**: complete `docs/GLOSSARY.md`, accept the topology ADR, and add the decision link to the local Wayfinder map. **Done for the current skeleton.**
2. **Protocol types**: implement `workbench-protocol`, `layout-model`, `asset-core` and error envelopes without a UI. **Done for the current layout slice; native wire fixtures cover the Tauri mapping.**
3. **Layout service**: implement versioned layout state, migrations, fixed zones and persistence tests. **Done for the current file-backed Tauri store and restart recovery slice.**
4. **Workbench parts**: render ActivityBar, Resource Manager, Feed Workbench, PrimarySidebar, WorkspaceArea, AuxiliarySidebar, OperationsPanel and StatusBar with placeholder views. **Initial renderer slice done.**
5. **View registry**: add built-in views and one example external view per zone. Verify that cross-zone registration is rejected. **Host registry and rejection tests done; the React shell mounts registered ViewContainers/Views with a renderer adapter seam, PluginClient projects extension views plus isolated UI Bridge sessions into that seam, and the Activity Bar now selects contributed navigation containers.**
6. **Preview core**: implement generic image/file fallback, `PreviewResolver`, Quick/Inspect/Analyze intents and a fake provider used by contract tests. **Host slice done:** surface-aware deterministic resolution, provider-failure fallback, typed Tauri/TypeScript projection, `PreviewHost` state machine, Quick overlay and unregisterable provider/augmentation slots. The bounded source adapter, versioned persistent AssetId source-location registry and trusted text/image providers are implemented; format-specific plugin execution, augmentation renderer adapters and product-specific cache consumers remain follow-up work.
7. **Plugin host**: load manifests, validate permissions and localization bundles, activate lazily, isolate failures and expose the UI SDK boundary. **Local discovery, install preflight, permission enforcement, host API compatibility checks, numeric version upgrades, transactional commit/rollback, crash-recovery scanning, deferred activation, retryable recovery, orphan cleanup and bounded recovery logs are implemented; application release signing is a native release gate, while extension publisher signing/revocation remains a separate PluginHost policy and is not inherited from the application signer. The target plugin gate additionally requires explicit runtime trust/sandbox policy, declarative no-runtime contributions, namespaced `ExtensionStorage`, host-orchestrated backup/migration participants, the internal `storage.export`/`storage.migrate` runtime RPC, structured diagnostic events, process-group/orphan cleanup and signed package trust records before a public SDK is released. The host localization service, fallback formatter, diagnostics, loader/plugin lifecycle seam, isolated UI Bridge locale projection, Rust-owned revisioned Profile locale persistence, bounded directory-package localization resource loader, bounded ZIP archive package adapter and native directory/ZIP package picker are implemented; other archive formats and non-localization binary resources remain follow-up adapters.**
8. **Jobs and facets**: the shared Rust JobSystem lifecycle seam is wired to OperationsPanel with persisted progress, cancellation, retry, failure and restart recovery. It now exposes a Rust JobRunner registry, attempt-safe cooperative cancellation, panic isolation, persisted JobResult payloads, JobProvenance projection, exact provenance reuse that refuses cache hits without an input fingerprint, and an isolated plugin `invoke-job`/`job.run` adapter with typed input, result and bounded progress events. Owner cleanup revokes runners and cancels queued/running Jobs before runtime shutdown; late results are ignored, and extension-owned retries fail with a diagnostic when the owner runner is unavailable instead of using the core fallback. Provider/version/settings changes are covered as cache misses through exact provenance; no generic artifact cache is added before a concrete provider or Facet owner needs one. Product-specific thumbnail, metadata and analysis runners remain follow-up consumers.
9. **Tauri adapter**: connect typed commands, queries and events to the desktop client. No feature should bypass the protocol. **Done for the current layout slice, including native command/event serialization and global shortcut entry.**

## Acceptance scenarios

### Native lifecycle/recovery acceptance matrix

The detailed decision record is [Define native recovery acceptance scenarios and
priority route](../../.scratch/native-lifecycle-recovery/issues/09-define-native-recovery-acceptance-and-priority.md).
The foundation gate below keeps its priority, semantic owner, test seam and
user recovery action visible beside the existing Workbench scenarios.

| Priority | Scenario that must pass | Owner crate(s) | Test seam | User recovery action |
| --- | --- | --- | --- | --- |
| P0 | Two pre-ready failures or a core startup deadline breach enter Safe Mode without clearing core data; optional activation timeout remains degraded. | `application-core` | Injected in-memory `RecoverySupervisor` journal, monotonic-clock and startup harness | Retry, disable the recent optional component, restore a snapshot, reset optional state or export diagnostics |
| P0 | A Workbench mount failure still mounts an offline, localized, keyboard-accessible `RecoverySurface` with failed owner/reason/actions. | `application-core`, `apps/desktop` | Host mount-failure fixture plus renderer/accessibility smoke test | Retry, quarantine the failing owner, restore or export diagnostics |
| P0 | After Tauri host setup succeeds, a renderer/WebView mount failure still exposes a localized host-native repair flow; it uses no plugin or renderer code and offers continue, restore validated `RecoverySet`, exact extension-generation quarantine and explicit close of quarantined runtime lease evidence. A failure before the binary can execute remains a separate launcher/release gate. | `tauri-adapter`, `apps/desktop/src-tauri`, `workbench-protocol` | `NativeRecoveryState`/host-copy fixtures, Tauri dialog composition compile/test, startup-marker fault injection and typed query/command dispatch fixtures | Continue Safe Mode, restore validated state and restart, quarantine damaged extension state, forget reviewed lease records or use the renderer recovery surface for diagnostic export |
| P0 | A Tauri builder/setup error or run-loop panic is classified as `platform-initialization`, stored in a bounded marker without arbitrary payloads, and consumed on the next successful launch into the recovery projection; marker corruption does not overwrite the recovery journal. | `apps/desktop/src-tauri`, `application-core`, `persistence` | Tauri startup probe marker fixture, panic/build error harness, bounded-message and next-launch journal integration test | Relaunch, enter Safe Mode, retry platform setup or export diagnostics |
| P0 | A pre-builder WebView/GPU probe records typed platform evidence; fatal WebView evidence starts an independent native recovery process without constructing Tauri/WebView, while GPU degradation remains a software-fallback diagnostic and unknown capability is non-fatal. | `apps/desktop/src-tauri`, `workbench-protocol` | Host probe override matrix, no-WebView helper-mode fixture, bounded request/intent round-trip and platform evidence projection | Retry platform setup, restore the last validated state or continue in Safe Mode |
| P0 | A recent OS crash artifact is attached after an unclean Core or stale launcher attempt, is bounded and streamed-hashed, platform `.crash`/`.ips` headers are parsed into path-free metadata, is consumed once, and never exposes a raw path or dump payload in the protocol. Missing permissions, malformed headers or unsupported crash stores degrade to ordinary exit evidence. | `apps/desktop/src-tauri`, `application-core`, `workbench-protocol` | Artifact metadata fixture, macOS text/IPS parser fixtures, recent/old/oversize/name-mismatch matrix, stale `LauncherAttempt` fixture, one-shot receipt consumption and wire round-trip | Export local diagnostics, retry or continue without native artifact |
| P0 | The fixed-path release wrapper writes a pre-spawn `LauncherAttempt`, captures child loader/exec, wait and non-zero exit failures with phase, exit code and bounded stderr, preserves the attempt on failure, and removes it after a successful child exit. | `apps/desktop/src-tauri`, `application-core`, `tools/release-qualification` | Launcher fake executable, spawn/exit/wait fixtures, bounded marker and `test-launcher-wrapper.sh` integration run | Relaunch, enter Safe Mode or export diagnostics |
| P0 | The release renderer uses explicit CSP and deny-by-default capabilities: scripts remain self-only, IPC uses Tauri origins, objects/forms are disabled, plugin frames are host-validated HTTPS surfaces, and no generic filesystem/shell/opener command is exposed. | `apps/desktop/src-tauri`, `tauri-adapter`, `workbench-protocol` | `capability_policy` exact-config fixture, generated capability inspection and `test-package-wiring.sh`; macOS debug bundle policy inspection | Continue with typed host actions, repair the package or export diagnostics |
| P0 | The release package builds a target-specific `velvet-launcher`, embeds it beside the main executable through `bundle.externalBin`, and Cargo selects `velvet-desktop` as the default binary even though the package contains two binaries. | `apps/desktop/src-tauri`, `tools/release-qualification` | `tools/build-release-launcher.sh`, `test-package-wiring.sh`, CI build/release wiring and macOS `npm run tauri build -- --debug` bundle inspection | Rebuild the matching target package, use the previous package generation or enter release repair |
| P0 | The standalone no-WebView repair helper reads a backward-compatible typed page list, shows Overview/Evidence/Actions pages and writes only a fixed recovery intent; closing a page defaults to Safe Mode. | `apps/desktop/src-tauri`, `workbench-protocol` | Native request page round-trip, legacy empty-page fixture and helper-mode intent fixture | Retry, restore validated state or continue in Safe Mode |
| P0 | A crash receipt from the previous Core process or an isolated runtime is consumed once and correlated to launch instance, attempt, process identity and optional runtime nonce; the evidence is bounded, local and included in a user-exported bundle. | `apps/desktop/src-tauri`, `application-core`, `workbench-protocol`, `plugin-host` | Active-process receipt, external receipt, one-shot consumption, runtime context/env and diagnostic export fixtures | Relaunch, inspect/export diagnostics, retry or disable the affected runtime |
| P0 | Forced core termination is classified on next launch and recovers without a helper process; leftover shutdown intent is reported as expected-or-uncertain rather than proof of a clean exit. | `application-core` | Shutdown-intent/completion and crash-receipt process harness | Relaunch, enter Safe Mode or export diagnostics |
| P0 | Interrupted/truncated writes preserve the previous `StateSnapshot`; unknown schemas and a corrupt recovery index become `QuarantinedState`, corrupt extension generations can be identified by committed filename metadata and quarantined without decoding, journal/index failure falls back to an in-memory receipt, and a read-only/unavailable state volume never causes silent default overwrite. | `persistence`, `application-core`, `plugin-host` | `crates/persistence/tests/release_qualification.rs` staged-write/pointer crash-boundary, unreadable-directory path-collision, ACL/read-only, fsync and checksum fixtures; `tools/release-qualification/run-macos.sh` and `run-linux-docker.sh`; tracked result matrix | Restore, retry migration, quarantine exact damaged generation, repair/move/export or discard quarantined state |
| P0 | Legacy Layout/Profile/Job files are imported through owner migration participants as one bootstrap transaction; a participant or commit failure leaves no new `RecoverySet`, rerunning is idempotent, and missing legacy files become explicit default owner snapshots. | `persistence`, `apps/desktop/src-tauri`, `application-core` | Legacy-file fixtures, participant failure injection, commit-barrier assertions and second-run idempotency test | Retry migration, enter Safe Mode, restore the last validated set or export diagnostics |
| P0 | A corrupt core snapshot directory reports the exact owner/scope/generation, publishes the previous `RecoverySet` before quarantine, moves only that generation, and restarts with the previous validated state; no default state silently overwrites damaged bytes. | `persistence`, `application-core`, `workbench-protocol`, `apps/desktop` | Corrupt-generation store/file fixture, typed projection assertion, restore/quarantine command test and restart reload | Quarantine the exact generation, restore the previous set, retry migration or export diagnostics |
| P0 | The persisted durability report round-trips with the requested mode and effective sync capabilities, appears in recovery projection and diagnostic export, and never claims crash durability when directory sync is unavailable. | `persistence`, `application-core`, `workbench-protocol`, `apps/desktop/src-tauri` | Release-qualification capability matrix, JSON report schema/corruption fixture, protocol wire fixture and native composition mapping | Continue with degraded durability, move the state directory, retry or export diagnostics |
| P0 | A related multi-owner change publishes a validated `RecoverySet`; a fault cannot expose mixed Profile/Workspace/Asset generations. | `application-core`, `persistence` | Commit-barrier and reference-validation fault injection | Restore the recovery set, retry or export diagnostics |
| P0 | Failed core migration is copy-on-write and returns to the previous generation. | `application-core`, `persistence` | Migration fixture failing at each stage and reference validation | Retry migration, restore the last validated generation or export diagnostics |
| P0 | Failed extension migration quarantines only that extension store. | `plugin-host`, `persistence` | Fake extension store plus manifest/activation fixture | Retry, disable, restore or discard extension state |
| P0 | Plugin hang/panic/restart loop removes only its projections and Jobs. | `plugin-host` | `FakeRuntime`/`FakeProcess` timeout and rolling-failure plans | Retry, restore package, disable or uninstall extension |
| P0 | Restarted Jobs become interrupted/retryable; cancellation and late-result rejection work. | `job-system` | Fake `JobRunner`, `JobStateStore`, attempt/provenance fixtures | Resume/retry, cancel or discard Job |
| P0 | A Job deadline uses a monotonic per-attempt timer; cooperative runners receive a typed timeout, an unresponsive runner is fenced after the configured cancellation grace and follows the bounded retry/terminal policy, and no late progress/result from the old attempt is accepted. | `job-system`, `application-core` | Cooperative and unresponsive runner fakes, deadline watchdog and stale-attempt fixtures | Retry with a bounded policy, cancel or export diagnostics |
| P0 | User cancellation is observable as `Cancelling`, gives a runner the configured grace period, releases the execution reservation, and becomes `Cancelled` without terminating a healthy shared plugin runtime. | `job-system`, `plugin-host` | Cooperative runner, cancellation watchdog, budget-ledger and runtime-isolation fakes | Wait for cancellation, retry or continue with the plugin |
| P0 | Typed `Transient` and timeout failures retry only within the bounded attempt/backoff policy; permission, schema, invalid-input, deterministic and explicit-cancel failures remain manual actions. | `job-system`, `application-core` | Retry-policy fixture with attempt counter, delay cap and typed error matrix | Retry explicitly, fix input/permissions or discard the Job |
| P0 | A runner checkpoint is schema-versioned, size-bounded and persisted only for its current attempt; plugin `job.run` receives the current checkpoint and may return a bounded checkpoint event; unsupported/oversized data is discarded on load, and a paused Job resumes from its last valid checkpoint. | `job-system`, `plugin-host`, `persistence` | In-memory/file `JobStateStore`, plugin invocation/checkpoint wire fixture, checkpoint-size/schema fixtures and restart/resume runner | Resume from checkpoint, retry from the beginning or discard the Job |
| P0 | `set-job-execution-environment` pauses background or policy-blocked Jobs, persists `Paused` plus a reason, and resumes eligible work after an online/wake signal without creating a second scheduler. | `workbench-protocol`, `application-core`, `tauri-adapter`, `job-system` | Rust/Tauri/TypeScript wire fixtures, suspend/offline environment fake and resume event assertions | Resume, wait for connectivity, retry or cancel |
| P0 | Diagnostic export is bounded/redacted and user initiated. | `application-core`, `tauri-adapter` | Deterministic bundle and export-adapter fixtures | Export locally; upload only after separate consent |
| P0 | Tauri capabilities and native commands are deny-by-default, typed and scoped; the main window has no broad core default or filesystem/shell/opener permission, and recovery cannot invoke arbitrary filesystem, shell, URL or process actions. | `tauri-adapter`, `apps/desktop/src-tauri`, `application-core` | Capability policy fixture, generated ACL inspection and command-negative tests | Use the typed fallback, retry with a permitted action or export diagnostics |
| P0 | Declarative, WASM, sidecar and trusted built-in forms negotiate the same typed contract; installed external packages resolve executables only from the committed package root or the host allow-list; native registration cannot activate an unscoped external launch; runtime trust/sandbox policy is explicit, and a sidecar is never presented as a security sandbox. | `plugin-api`, `plugin-host`, `workbench-protocol`, `application-core` | Manifest/package-root fixtures, escape/absolute/unlisted-command negatives, native `register-plugin` negative, trust-policy matrix and capability-negative tests | Inspect permissions, enable only with explicit trust, disable or use a safe alternative |
| P0 | `ExtensionStorage` is namespaced and transactional; host quota and generation conflicts are enforced, direct and runtime `BackupParticipant` exports are bounded, and direct or `storage.migrate` participant failure publishes no new extension generation or core `RecoverySet` pointer. External activation migrates an older committed extension schema before exposing the runtime as ready. Malformed runtime output and a concurrent writer are typed failures. Metadata-only recovery query exposes current/previous generations; restore requires same schema, commits a new generation, disables the active extension and does not auto-reactivate; quarantine uses an exact-generation compare-and-swap, including a corrupt or unknown-schema generation discovered from committed filename metadata, and leaves unrelated extensions usable. | `plugin-host`, `persistence`, `application-core`, `tauri-adapter`, `apps/desktop` | In-memory snapshot store, manifest/protocol fixtures, fake JSONL runtime, participant failure/success/schema/quota/conflict and activation-gate tests, typed Rust/Tauri/TypeScript wire fixtures, desktop recovery-action test, corrupt-payload filename-repair test, checksum and power-loss fixtures | Retry, restore the previous extension generation, quarantine the exact damaged generation, explicitly enable later or discard extension state |
| P0 | One extension runtime crash, WASM trap, timeout, malformed response or budget kill removes only that extension's projections and Jobs; a second extension continues. Sidecar exit evidence is bounded and Unix descendants are cleaned during shutdown. | `plugin-host`, `job-system` | One-process-per-extension fake runtimes, bounded-stderr/exit-evidence and process-group fixtures, late-result fixtures | Retry/recover the extension, cancel/retry its Jobs or continue with the fallback |
| P0 | A Core crash or forced power loss marks all child runtimes and in-flight extension Jobs interrupted; committed extension stores remain recoverable, confirmed stale leases are closed, and uncertain orphan sidecars are durably quarantined so they cannot be repeatedly probed or killed after restart. | `application-core`, `plugin-host`, `job-system` | Forced-termination/process-group fixtures; bounded runtime-lease persistence and starting/running/quarantined commit tests; confirmed/uncertain orphan reconciler fakes; Linux `/proc` start-token/process-group scan and parent-death probe; macOS `proc_pidinfo` identity and confirmed process-group termination release test; Windows Job Object and creation-time fixtures on a Windows CI target | Relaunch, resume/retry safe Jobs, review/close quarantined runtime evidence, quarantine the affected extension or export diagnostics |
| P0 | Plugin logs and runtime failures produce bounded structured events correlated to extension and Job attempt; sidecar stderr and exit status/signal evidence are bounded before projection; no synthetic session id is emitted before the runtime session contract exists; secrets, asset payloads and unrestricted paths are redacted. | `plugin-host`, `application-core` | Diagnostic sink fake with bounded stderr/exit/trap/secret fixtures | Export a redacted bundle; retry or disable the extension |
| P1 | Major protocol mismatch fails during the typed handshake before any command; compatible minor versions select the lower minor, unknown features are ignored, and schema versions are negotiated independently. | `workbench-protocol`, `tauri-adapter`, `apps/desktop/src-tauri` | `negotiate_protocol` major/minor/schema fixture, Tauri handshake command fixture, TypeScript `TauriCoreTransport` handshake/reconnect test and typed error mapping | Update, disable incompatible feature or open recovery UI |
| P1 | A stable `clientId` owns at most one active session; reconnect retires the old `sessionId`, rejects old envelopes before dispatch, and preserves bounded outcome evidence for the new session's query. | `tauri-adapter`, `apps/desktop/src-tauri`, `apps/desktop` | Client/session replacement fixture, wire envelope round-trip, Rust client-scoped outcome test and TypeScript reconnect assertions | Reconnect, inspect the authoritative outcome or refresh projection before retrying a mutation |
| P1 | Transport loss or an event sequence gap keeps the last projection visible but read-only while the desktop creates a fresh session, queries the authoritative Layout/Profile/Recovery/Job/Extension snapshot and applies it without replaying mutations; plugin/UI Bridge and transient Preview sessions are disposed during recovery and recreated after the snapshot. | `workbench-protocol`, `tauri-adapter`, `apps/desktop` | Monotonic event sequence fixture, fake Tauri event sink, `query-snapshot` wire mapping, TypeScript gap/status test, Workbench status seam, UI Bridge suspend/resume fixture and Preview recovery fixture | Retry reconnect, continue from the resynced projection or enter the recovery surface |
| P1 | A declared idempotent command with the same `(clientId, requestId)` returns its bounded original result without duplicating side effects; a mutation with an unknown result uses `query-request-outcome` and never blindly replays an external effect. | `workbench-protocol`, `tauri-adapter`, `apps/desktop` | Idempotency declaration fixture, bounded cache/eviction test, duplicate event assertion and typed unknown/completed outcome query | Query authoritative state, retry only the declared idempotent command or leave the mutation unresolved |
| P1 | Device → owner → Job budgets enforce hard admission and soft backpressure. | `job-system`, `plugin-host` | Persisted `JobBudgetRequest`, hard ledger, bounded pressure delay, plugin read-only claim projection, cancellation release and isolated-owner tests; OS-level runtime telemetry remains a qualification gate | Cancel, retry later, evict derived cache or discard Job |
| P1 | Platform sleep/network adapters deliver suspend, wake, offline, metered and unknown signals to the typed Job environment seam; wake recovery is rate-limited and does not spin retries. | `tauri-adapter`, `application-core`, `job-system`, `apps/desktop/src-tauri` | Fake source/controller test, fixed-command parsers, Tauri `RunEvent::Resumed` wiring, monotonic wake-gap fixture and Rust/Tauri/TypeScript `unknown` contract; real device qualification remains open | Resume, retry, cancel or wait for connectivity |
| P1 | Tier A mismatch prevents `ready` and reaches host-native/`RecoverySurface` update action; Tier B mismatch degrades only its feature. | `tauri-adapter`, `apps/desktop` | OS/architecture/WebView/GPU/file-IO/IME capability fixtures | Retry/update runtime or use fallback |
| P1 | Profile lock admits one writer before Tauri/Core startup; macOS/Linux second instances verify the recorded process identity, reclaim only an absent/reused owner, otherwise forward bounded schema-validated activation frames and exit after acknowledgement. The primary's pending queue plus wake event prevents startup-time event loss. Malformed or ambiguous-owner cases fail closed. | `apps/desktop/src-tauri`, `workbench-protocol`, `tauri-adapter`, `apps/desktop` | `ProfileInstanceCoordinator` Unix lock/socket harness, `/proc`/`proc_pidinfo` identity seam, stale-record compare-and-remove fixture, pending-queue/drain command fixture, Tauri activation wake event and desktop runtime payload guard | Focus the existing instance; reclaim only after verified stale identity, otherwise export diagnostics or retry |
| P1 | Native activation canonicalizes existing regular-file paths and percent-decoded `file://` URIs before forwarding; the Rust/Tauri/TypeScript register-preview-asset contract carries optional `approvedRoot`, while a separate typed query/add/remove contract manages persisted `ApprovedSourceRoot` policy. The Asset source owner canonicalizes and persists the root, rejects escapes or unapproved registrations, and revalidates canonical path/root/volume, returning retryable `source-unavailable` on disconnect, canonical-path replacement or volume change and typed `PermissionDenied` for access or root denial. The host Source Folders dialog exposes refresh, native browse, explicit approval and confirmed revocation without giving paths to plugins. Workbench derives bounded, deduplicated intents for Asset Manager from file targets and `velvet://asset/<id>`, activates a ready workspace before delegating, and retains failed consumption for retry; Native Workbench consumes `open-asset` by updating the host-owned Asset selection and consumes `open-file` through typed registration with an activation-scoped AssetId, while unknown Velvet links remain unrouted. | `apps/desktop/src-tauri`, `persistence`, `application-core`, `workbench-protocol`, `tauri-adapter`, `apps/desktop` | Rust approved-root containment/root-replacement/revocation fixtures, root registry round-trip and atomic save, Tauri root query/add/remove and optional-field wire fixtures, TypeScript `SourceRootService` load/mutation/failure-isolation fixture, `source-root-dialog.test.mjs` UI/picker/revocation fixture, workspace-routing consumption/failure-isolation/asset-selection/file-registration fixture; real removable/network mount probes are explicitly deferred | Choose an allowed root, add or re-approve a directory, revoke it after confirmation, repair permissions or reconnect the source, or retry workspace consumption |
| P1 | Signed release staging accepts only a pinned-root `ReleaseManifest` for the selected channel/target and a matching artifact digest; a package is copied into a private durability-aware staging generation, paired with the current `RecoverySet` anchor, and becomes authoritative only after the typed `workbench-ready` health checkpoint. Invalid, stale, interrupted or failed updates keep the committed package/data generation and preserve a bounded last-transaction record. | `application-core`, `persistence`, `workbench-protocol`, `tauri-adapter`, `apps/desktop/src-tauri` | Ed25519 canonical-manifest fixture, pinned-key/target/expiry/anti-rollback matrix, SHA-256 package staging with temp-directory fault injection, required-fsync durability seam, typed Rust/Tauri/TypeScript release wire fixture, health-check commit barrier and rollback audit fixture | Retry/offline install, commit after a healthy restart, roll back the staged generation or export diagnostics |
| P1 | A consumed launcher/Tauri pre-ready failure automatically rolls back the one pending release transaction through `ApplicationCore`, removes only private package staging, records `release-rollback`, enters Safe Mode and is idempotent on the next launch; a committed transaction is never changed. | `apps/desktop/src-tauri`, `application-core`, `persistence`, `workbench-protocol` | Launcher/Tauri marker fixtures, pending/committed journal matrix, `ApplicationCore` rollback-on-startup test, `RecoverySupervisor` forced-safe test and Tauri wire/recovery projection assertions | Inspect diagnostics, retry the previous package, restore a validated `RecoverySet`, install a compatible package or continue in Safe Mode |
| P1 | The Tauri updater adapter remains host-owned: GitHub Release `latest.json` discovery and signed artifact installation pass through `UpdateCoordinator`, which requires the selected `velvetManifests[tauriPlatform]` entry, stages the package before install and commits only after post-restart `workbench-ready`; no renderer updater permission is granted. Tauri Action supplies platform signatures and URLs, while the release finalizer supplies Velvet's exact per-platform Ed25519 map and fixed channel asset. | `apps/desktop/src-tauri`, release pipeline, `application-core`, `persistence` | `test-updater-release.mjs` real Ed25519 fixture, static endpoint schema fixture, exact platform/download map injection fixture, separate Tauri/Velvet key fixture, offline/proxy check fixture, updater permission negative test, host command/TypeScript status fixture and package/data health-commit integration test; protected CI secrets and real package install/rollback remain open | Check later, install an offline package, retry a failed download or use the recovery surface |
| P1 | Release CI builds each target with Tauri's updater signature, generates a canonical Velvet `ReleaseManifest` from the exact bytes for every standard updater platform entry (including installer-specific entries), uploads `velvet-manifest-<platform>.json`, then injects all entries into one channel-specific `latest.json`; only the fixed channel asset is configured as the endpoint. | release pipeline, `tools/release-qualification` | Deterministic Ed25519 fixture, artifact-suffix selection, exact platform/download binding, missing/invalid signature and channel mismatch fixtures; protected-secret dry run and a real draft GitHub Release are required before enabling auto-update | Keep the current version, inspect the release diagnostics or retry publication |
| P1 | Offline and proxy discovery failures are bounded, diagnosable and non-blocking; a downloaded package is staged only after Tauri and Velvet signatures plus digest pass, and a simulated install failure invokes one typed rollback while retaining the committed package/data generation. | `apps/desktop/src-tauri`, `application-core`, `persistence`, release pipeline | Endpoint config fixture with preview/stable isolation, unavailable/proxy fixture, `UpdateCoordinator` install-error rollback test, pending-journal idempotency test and one real signed-package runner per supported OS | Check later, configure a proxy, install an offline package, retry, or continue in Safe Mode |
| P1 | Uninstall removes binaries but retains user data until explicit deletion. | `application-core`, `tauri-adapter` | Retention fixture with external-source sentinel | Reinstall/recover or explicitly delete selected data |
| P1 | Crash evidence, when the platform provides it, is local, bounded, redacted and correlated to the launch/runtime identity; lack of native dump capture still leaves journal/exit diagnostics. | `application-core`, `tauri-adapter` | CrashCaptureAdapter fake with OOM/signal/exception fixtures, active-process and runtime receipt fixtures | Export diagnostics, retry or continue without upload |
| P1 | Cold start, idle memory/disk/CPU and hidden WebView lifecycle stay within the versioned platform baseline; suspended surfaces do not run undeclared work. | `application-core`, `tauri-adapter`, `apps/desktop` | Monotonic-clock startup harness, resource probes and WebView lifecycle fake | Close/suspend a surface, retry, update runtime or continue degraded |
| P1 | Stable/preview extension packages verify digest, publisher identity, target, capabilities, freshness, revocation and anti-downgrade before activation; unsigned local packages require visible developer mode. | `plugin-api`, `plugin-host`, `application-core`, `tauri-adapter` | Signed-package verifier fake, key-rotation/revocation/offline fixtures and developer-mode policy test | Keep the current package, inspect trust issues, install a valid offline package or disable the extension |
| P1 | Staged extension upgrade keeps old package/data generations until the new runtime reaches a health checkpoint; failed activation or migration restores the previous pair without touching core data. | `plugin-host`, `persistence`, `application-core` | Upgrade transaction, crash/power-loss and schema-range fixtures | Resume, commit, rollback or discard the staged extension transaction |
| P1 (marketplace) | A paid extension receives only host-verified entitlement status with bounded offline grace; package signature and publisher trust remain separate, and no payment credential crosses the plugin boundary. | `plugin-host`, `workbench-protocol`, `tauri-adapter` | Fake entitlement issuer with expiry, revocation, clock-skew, sign-out and offline fixtures | Continue in grace, renew/sign in, disable paid feature or keep data and uninstall |

### Native hardening additions (2026-11-02)

These rows extend the existing lifecycle matrix with the decisions from
ADR-0014/0015 and the identity/environment review. They are intentionally
separate from the existing shell and plugin rows so a host seam cannot be
mistaken for a release qualification.

The first implementation slice now provides the Rust/protocol/desktop
`JobExecutionEnvironment` schema and conservative background admission for
storage and power pressure. The native signal adapter still supplies only the
network/suspend observations; real storage, battery and thermal probes remain
platform qualification work. The protocol now also defines the bounded
`OperationPlan`/`OperationReceipt` shape and stale-target/reconciliation
validation. `persistence` now provides a bounded atomic
`OperationReceiptStore` for restart/reconciliation evidence; provider
execution, per-item journaling policy and undo tokens remain the next P0 slice.

| Priority | Scenario that must pass | Owner crate(s) | Test seam | User recovery action |
| --- | --- | --- | --- | --- |
| P0 | A versioned `PortableBundle` is inspected and staged without exposing paths or commands; selected Profile/Workspace/Asset core sections commit atomically through one `RecoverySet`; a required section failure publishes no partial core state; an absent/untrusted extension section is quarantined and diagnostics remain export-only. | `application-core`, `persistence`, `plugin-host`, `workbench-protocol` | Bundle manifest/schema/size/integrity fixtures, participant failure and migration fakes, recovery-set commit barrier and extension-quarantine fixtures | Review `ImportPlan`, retry migration, restore the pre-import set, install the extension before retrying or discard quarantined data |
| P0 | Delete/overwrite/move/batch Job/remote actions use typed `OperationPlan` and `OperationReceipt`; stale revisions/fingerprints return `plan-stale`; `started`/`unknown` outcomes require reconciliation; only provider-declared idempotency and undo tokens permit retry/compensation. | `application-core`, `persistence`, `job-system`, `workbench-protocol` | Fake local/remote owners, stale-preview, unknown-IPC, per-item partial, non-idempotent retry and irreversible-delete fixtures | Re-preview, query authoritative outcome, reconcile, use a valid undo token or explicitly discard |
| P0 | Full/read-only/unavailable Profile storage keeps the last authoritative projection readable when possible, blocks persistence-dependent mutation/Jobs with typed errors and never overwrites state with defaults; missing source volumes remain `SourceAvailability::degraded`. | `application-core`, `persistence`, `job-system`, `apps/desktop/src-tauri` | Read-only/full/permission-denied volume fixtures, source disconnect/reconnect and volume-identity fixtures | Move/repair storage, continue read-only, reconnect/re-approve the source or export diagnostics |
| P0 | Negative boundary suite rejects a second owner/scheduler, broad Tauri capability, arbitrary path/URL/process command, replayed/lower release manifest, partial required restore and uncapped retry/late completion. | `persistence`, `application-core`, `tauri-adapter`, `workbench-protocol`, `job-system` | CI policy diff, protocol command negatives, release replay matrix, recovery fault injection and retry-attempt fixtures | Use the typed host action, keep the committed generation, restore/retry with bounds or enter Safe Mode |
| P0 | Profile lock and transport session never grant authority by PID or `clientId` alone; stale/reused/ambiguous locks fail closed, old sessions reject mutations and source roots/volumes are revalidated before access. | `apps/desktop/src-tauri`, `tauri-adapter`, `workbench-protocol`, `application-core` | Lock identity/reuse/permission fixtures, session replacement and stale-envelope tests, approved-root/volume fixtures | Focus the existing instance, retry, continue read-only or repair/re-approve the source |
| P1 | Extension packages verify publisher signature, digest, target, capabilities, freshness, revocation and anti-downgrade offline; unsigned packages require visible Profile-scoped developer mode; old package/data remains until extension health commit. | `plugin-api`, `plugin-host`, `application-core`, `tauri-adapter` | Signed-envelope fake, key-rotation/revocation/offline/developer-mode and upgrade crash fixtures | Keep previous package, inspect trust, install a valid offline package or disable/quarantine the extension |
| P1 | Application release staging passes protected-key, target artifact, installer retention, proxy/offline, staged rollout and package/data rollback qualification; signed `SafetyNotice` can block a known-bad release without network-dependent startup. | `application-core`, `persistence`, `apps/desktop/src-tauri`, release pipeline | Real signed package per target/channel, key rotation/replay/freshness, installer/uninstall and pending-transaction fixtures | Keep current package, retry/offline install, roll back pending staging or use native recovery |
| P1 | Storage/power/thermal/suspend/wake/network/endpoint facts reach the typed Job environment seam with bounded backpressure; monotonic timing survives wall-clock changes; Windows identity/Job Objects and real removable/network mounts pass on claimed targets. | `job-system`, `tauri-adapter`, `apps/desktop/src-tauri`, `application-core` | Fake and real platform signal sources, long-suspend/checkpoint, storage/power/proxy and per-target native qualification | Pause/defer background work, retry with backoff, continue foreground/recovery or show typed degradation |
| P1 | Cold-start, idle/hidden WebView, memory/disk/CPU and diagnostic bundle baselines are recorded per target/channel; regressions leave the core usable and expose a recovery action. | `application-core`, `tauri-adapter`, `apps/desktop` | Monotonic startup/resource harness, WebView lifecycle fake and package diagnostics | Suspend/close nonessential surfaces, update runtime, retry or export diagnostics |
| P2 | Authenticated CLI/daemon/remote clients use profile scope, authority epoch, capability grants and key rotation; shared projections and merge/conflict semantics are explicit before any remote mutation. | future `CoreTransport` adapter and domain owners | Authenticated multi-client/authority fixtures | Revoke the client, resync or keep the local Profile authoritative |

The P0 negative tests are mandatory CI gates, not optional examples. P1 rows
cannot be promoted by cross-compilation or `not-configured` probes alone. P2
rows are deferred and must not weaken the P0/P1 ownership or fail-closed rules.

The first lifecycle implementation slice has these acceptance results:

| Slice | Result | Evidence |
| --- | --- | --- |
| Journal round-trip and unknown-schema rejection | Pass | `JsonFileRecoveryJournalStore` persistence test |
| Two unfinished pre-ready launches enter Safe Mode with recovery actions | Pass | `RecoverySupervisor` deterministic-clock test |
| Core layout migration failure enters Safe Mode and preserves the failed state | Pass | `ApplicationCore` migration fixture |
| Optional plugin activation failure degrades without entering core Safe Mode | Pass | `ApplicationCore` plugin activation fixture |
| Activated plugin runtime failure emits plugin-disabled and recovery-degraded evidence while leaving core data available | Pass | `tauri-adapter` runtime failure fixture |
| Rust recovery query, Tauri response/event and TypeScript types agree | Pass | `workbench-protocol`, `tauri-adapter`, desktop Tauri compile and TypeScript typecheck |
| Canonical transport handshake negotiates major/minor, features/schema and client/session replacement across Rust, Tauri and TypeScript | Pass | `workbench-protocol` negotiation fixtures, `tauri-adapter` session registry tests, Tauri wire handshake fixture and `apps/desktop/test/tauri-transport.test.mjs` |
| Sequence gaps trigger fresh-session authoritative snapshot resync, Workbench enters read-only degraded mode, disposes plugin/Preview sessions, then applies Layout/Profile/Recovery/Job/Extension projections and recreates sessions without replaying a mutation | Pass | Tauri event sequence/status fixture, `query-snapshot` wire mapping, `apps/desktop/test/tauri-transport.test.mjs`, Workbench status integration, UI Bridge suspend/resume and Preview recovery fixtures |
| Declared idempotent requestIds replay bounded results, while a new session can query the prior client's completed/unknown outcome | Pass | `tauri-adapter` client-scoped outcome cache/duplicate-event fixture, `query-request-outcome` Tauri mapping and TypeScript query contract |
| Desktop `RecoveryClient` loads the projection and accepts recovery events without reading native storage | Pass | `apps/desktop/test/recovery-client.test.mjs` |
| Normal Tauri shutdown records intent and completion; an unfinished launch remains classifiable after forced termination | Pass | `RecoverySupervisor` shutdown receipt test and Tauri run-event wiring |
| Core deadline breach records `startup-timeout` and enters Safe Mode; optional activation deadline remains degraded and stops scheduling more components | Pass | Injected `StartupDeadlinePolicy`/`RecoveryClock` tests and activation-loop boundary |
| Renderer `RecoverySurface` catches startup/layout/Workbench mount failure, renders the projection offline and offers a retry seam | Pass | `recovery-surface.test.mjs` plus `RecoveryBoundary` mount fixture |
| RecoverySurface exports a bounded, user-initiated `RecoveryDiagnosticBundle` through Rust/Tauri/TypeScript | Pass | Application-core export, Tauri wire fixture, `RecoveryClient` and renderer action tests |
| Generic staged/committed `StateSnapshotStore` validates and quarantines owner generations | Pass | `crates/persistence/src/snapshot.rs` file-store and in-memory fault-injection tests |
| Native Tauri Layout/Profile/Job stores load and publish through owner snapshots, while an existing `RecoverySet` keeps restored members stable across Job startup persistence | Pass | `owner_snapshots` fixtures plus `ApplicationCore` restore regression test |
| Legacy Layout/Profile/Job state migrates through composition-owned participants with an all-owner bootstrap barrier and idempotent second launch | Pass | `owner_snapshots` participant success/failure/idempotency fixtures and Tauri composition wiring |
| Corrupt core snapshot generation is surfaced as a typed recovery target; quarantine restores the previous set before moving the exact generation and restart reloads the validated state | Pass | `application-core` corrupt-generation projection/quarantine/restart test and persistence generation-aware corruption fixture |
| `RecoverySet` commit barrier publishes only complete validated members; previous-set restore and targeted quarantine select the prior set, and the renderer restarts to apply it | Pass | File/in-memory recovery-set tests, typed ApplicationCore action test and native recovery action wiring |
| Required fsync mode syncs the temporary file and parent directory before publication; unsupported platforms remain atomic-only and diagnosable; the persisted report is projected through recovery | Pass | `DurabilityMode`/`DurabilityReportStore` persistence fixtures, protocol mapping and native composition policy |
| Cross-platform persistence release qualification keeps the last committed snapshot and recovery pointer across staged-write crash boundaries, preserves evidence when the snapshot directory is unreadable, and rejects unknown durability-report schemas | Pass (CI seam) | `crates/persistence/tests/release_qualification.rs`, compiled for aarch64/x86_64 macOS, x86_64 Windows GNU and x86_64 Linux; native harness and current observed result are tracked in `docs/engineering/native-release-qualification-matrix.md` |
| ReleaseManifest verification is canonical and fail-closed: the pinned Ed25519 preview root, channel/target, updater/data schema range, minimum client version, validity window, monotonic sequence and artifact SHA-256 are checked before a transaction is staged | Pass | `application-core::release` signature/target/compatibility/expiry/anti-rollback/digest fixtures and pinned key construction in `apps/desktop/src-tauri/src/release_policy.rs` |
| Package and data staging retain the committed generation until `workbench-ready`; package copy/digest/fsync failures and an explicit rollback remove only the staged generation while the release journal preserves the last transaction | Pass (host seam) | `persistence::JsonFileReleasePackageStore` temp-directory/fault fixtures, `JsonFileReleaseUpdateStore` round-trip/unknown-schema fixtures, `application-core` stage/health/rollback tests and `RecoverySet` generation anchor; OS installer signing/notarization and external updater transport remain release gates |
| Release status and stage/health/rollback actions remain aligned across Rust, Tauri and TypeScript without exposing arbitrary package paths | Pass | `workbench-protocol` DTOs, Tauri wire mapping/serialization fixture, desktop protocol types and `apps/desktop/test/tauri-transport.test.mjs` release-status request fixture |
| A pre-ready launcher/Tauri failure rolls back a pending release once and keeps the next launch in Safe Mode without touching a committed transaction | Pass | `ApplicationCore::rollback_pending_release_after_startup_failure`, `RecoverySupervisor` `release-rollback` fixture and native setup marker wiring; binary/installer rollback before the app image remains a release gate |
| Tauri updater/GitHub Release integration stays host-owned and fail-closed | Pass (host seam; publication gate open) | `apps/desktop/src-tauri/src/update_coordinator.rs` matches the selected download URL to `velvetManifests[tauriPlatform]`, validates it through `ApplicationCore`, stages bytes before `Update::install`, rolls back install failures and commits only after the next `workbench-ready`; `generate-release-manifest.mjs`, `verify-release-manifest.mjs`, `inject-velvet-manifests.mjs`, `validate-updater-json.mjs`, `test-updater-release.mjs`, the release matrix and finalizer cover canonical signing, pinned-key verification, exact platform/download binding, channel endpoint separation and negative metadata paths. Protected CI keys, real GitHub signed artifacts, proxy/offline runs and OS installer rollback still require release qualification |
| Native release harness executes abrupt-process and ACL/read-only probes with explicit volume/deferred states | Partial (mount qualification deferred) | macOS native and Linux Docker process/ACL/read-only runs passed on 2026-11-01; real removable/network mounts are intentionally deferred for this foundation slice, physical power interruption remains deferred, and Windows is deferred without a Windows environment | Keep the deferred gates explicit; schedule real mounts before a release that claims removable/network continuity |
| Profile lock and macOS/Linux second-instance forwarding use a durable single-writer lock, start-token verified stale recovery, bounded Unix socket frame, native pending queue, typed Tauri wake event/drain command and runtime payload validation | Pass (macOS/Linux seam; Windows/path qualification open) | `apps/desktop/src-tauri/src/instance.rs`, `/proc`/`proc_pidinfo` identity fixtures, stale-record compare-and-remove tests, activation pending command/event fixture and `apps/desktop/test/tauri-transport.test.mjs`; Windows identity/forwarding, canonical path policy and workspace routing remain open | Focus the primary instance; reclaim only after verified stale identity, otherwise export diagnostics or retry |
| Native activation normalization canonicalizes regular files and `file://` URIs, Asset source resolution enforces a persisted `ApprovedSourceRoot` registry plus canonical path/volume revalidation, and Workbench consumes routing intents with retry isolation | Partial (native/source/routing/import and host Source Folders UI pass; real removable/network mount and Windows qualification remain deferred) | `apps/desktop/src-tauri/src/instance.rs` fixtures, `persistence` approved-root/volume/root-registry round-trip, `application-core` containment/root-replacement/revocation/disconnect/identity-mismatch/symlink-replacement tests, Tauri root query/add/remove plus optional-field wire fixtures, TypeScript `SourceRootService` failure-isolation fixture, `source-root-dialog.test.mjs` UI/picker/revocation fixture, bounded content-kind hint tests and `workspace-activation-routing.test.mjs`; Native Workbench selection, typed file registration, preview hints and host settings actions are wired, real mounts/Windows remain deferred | Choose or add an allowed root, revoke/re-approve policy, repair permissions, reconnect the source, or retry workspace consumption |
| `RecoverySetTransaction` keeps the previous pointer authoritative until every changed owner commits, and bootstrap does not expose partial owner generations | Pass | Persistence transaction and bootstrap visibility fault tests |
| Manifest-declared `ExtensionStorage` carries the same schema/quota through Rust `plugin-api`, `PluginManifestDto`, Tauri serialization and desktop TypeScript; unrelated extension namespaces remain readable after one participant fails | Pass | `plugin-host` host isolation/transaction tests, `workbench-protocol` manifest fixture, desktop typecheck and Tauri compile |
| Isolated runtime `storage.export`/`storage.migrate` uses the host-owned JSONL seam; bounded results are schema/quota checked, migration is compare-and-swap committed before external activation becomes ready, and runtime failure leaves the prior generation authoritative | Pass | `plugin-host` storage runtime fixtures with request-shape, schema, activation-gate, failure-isolation and generation assertions; no Tauri/TypeScript wire change |
| Plugin runtime and Job failures produce bounded, redacted, typed diagnostic events; one extension's event does not alter another extension's projection or Core lifecycle | Pass | `plugin-host` diagnostic sink tests, `tauri-adapter` failure-isolation event test, Tauri wire fixture and desktop typecheck |
| Diagnostic history is bounded, atomically persisted by the application owner, restored across Core restart and included in the user-exported bundle; corrupt history only marks observability degraded | Pass | `persistence` diagnostic store round-trip/corruption fixtures, `application-core` restart/degraded-startup tests and bundle export assertions |
| Runtime leases publish before an external launch, commit process identity only after spawn, close confirmed stale leases on reconciliation, and durably quarantine uncertain identities without repeating probes or kills | Pass | `plugin-host` lease store, lifecycle and reconciler tests; typed runtime-recovery query/close command, native composition wiring and Linux start-token/PID-reuse tests pass |
| Platform runtime supervision records macOS/Windows start identity and keeps sidecar termination behind a host-owned adapter | Partial | macOS `proc_pidinfo` identity and confirmed orphan process-group termination pass in the native release harness; Linux `/proc` identity and termination pass in Docker; x86_64 macOS and x86_64 Windows cross-target compile/clippy, Windows Job Object kill-on-close and creation-time code, native lease identity write path and release-wrapper loader/exit integration fixture are covered; Windows hardware runtime and abrupt Core-loss capture before the wrapper image loads remain deferred |
| Quarantined runtime lease evidence is queryable through Rust/Tauri/TypeScript and an explicit close action removes only the record without terminating a process | Pass | `plugin-host` close/query test, Tauri wire fixture and desktop PluginClient lifecycle test |
| Tauri builder/setup/run-loop initialization failures are bounded, persisted and consumed as `platform-initialization` evidence on the next successful launch | Pass | Native startup marker fixture, panic-safe run wrapper and recovery failure classification |
| Main and reserved recovery capability manifests exclude broad core/filesystem/shell/opener access and remain consistent with the native command boundary | Pass | `capability_policy` fixture plus generated Tauri capability inspection; the same policy fixture asserts the explicit CSP, asset-CSP mutation setting and `bundle.externalBin` |
| Release CSP keeps the main renderer self-contained: scripts are self-only, IPC uses the Tauri origins, objects/forms are disabled, plugin frames are bounded to host-controlled/TLS surfaces, and no broad network capability is added | Pass (policy + macOS bundle) | `capability_policy` exact-config fixture, `tools/release-qualification/test-package-wiring.sh`, and `npm run tauri build -- --debug`; only `style-src-attr 'unsafe-inline'` is allowed for current React style attributes, while stylesheet `style-src` remains self-only. A policy failure routes to typed host repair/diagnostics. |
| Release package wiring builds a target-specific `velvet-launcher`, embeds it beside the main Tauri executable through `bundle.externalBin`, and keeps Cargo's default binary on `velvet-desktop` | Pass (macOS/Linux Docker package wiring) | `tools/build-release-launcher.sh`, `test-package-wiring.sh`, macOS `Velvet.app/Contents/MacOS/{velvet-desktop,velvet-launcher}`, Linux Docker target-sidecar run and CI build/release wiring; Windows runtime packaging remains deferred. A mismatch requires rebuilding the target package or using the previous package generation. |
| Job admission applies device and owner hard limits, persists the accepted claim, exposes bounded soft CPU/network pressure, isolates owners and releases reservations on cancellation | Pass | `job-system` budget ledger tests with owner/device limits, typed rejection, pressure delay and cancellation release; plugin invocation and Tauri/TypeScript claim fixtures |
| Job governance applies bounded deadline/grace, typed transient/timeout retry, attempt-safe late-result rejection and cancellation reservation release | Pass | `job-system` deadline, transient retry, cancellation watchdog and stale-attempt tests; Rust protocol/Tauri wire policy fixtures |
| Job checkpoints are bounded/schema-versioned, survive restart, resume from a valid cursor and discard unsupported or oversized payloads without overwriting core state | Pass | `job-system` checkpoint/environment durability test and load normalization seam |
| Suspend/offline policy pauses and resumes Jobs through one typed environment command; the host does not create a second renderer scheduler | Pass (macOS/Linux adapter seam) | `workbench-protocol`, `application-core`, `tauri-adapter::PlatformSignalController`, Tauri `RunEvent::Resumed`, fixed `scutil`/`nmcli` parser tests, desktop `JobClient` environment fixtures; physical pre-suspend delivery and real metering confidence remain release qualification |
| Resource claims and soft CPU/network pressure stay consistent across Rust, plugin runtime, Tauri and TypeScript | Pass | `JobSnapshot.budget`, bounded `JobBudgetPressure`, `PluginJobInvocation.budget`, Tauri wire fixture and desktop `CoreJobBudgetRequest` typecheck |
| Native recovery, orphan policy, host locale and extension restore/quarantine confirmation | Partial | Localized Tauri/native multi-page repair flow, pre-builder WebView/GPU probe, explicit quarantined-lease close action, startup marker, `LauncherAttempt`/launcher failure markers, stale-attempt correlation, bounded macOS `.crash`/`.ips` metadata parsing, wrapper integration test, crash receipt correlation, bounded system artifact metadata, explicit CSP/capability policy, target-specific package wiring and filename-metadata generation repair are implemented and tested; OS failure before the wrapper image loads, native minidump decoding, package signing/notarization and installer rollback qualification remain |

All P0 rows, including the native-hardening additions, must pass at kernel
evidence level before the native foundation is considered safe for alpha users.
P1 rows must pass at platform and release evidence levels for each supported
platform and package/channel before that combination is production-ready; Tier B
failures remain diagnosable degradation. The optional out-of-process
`RecoverySupervisor` is measured separately and is not a release prerequisite.
Crash evidence is tested through the separate `CrashCaptureAdapter` seam and
is not used to justify a mandatory helper process. A row remains `Partial` when
its target/device/release gate is not configured or when a user recovery action
cannot run without the failed optional component.

- The built-in `WorkbenchApp` entries are always discoverable and do not move.
- A trusted or isolated extension can contribute an ActivityBar `WorkbenchApp`, a central `AppSurface`, a ViewContainer-only tool, or the compatible combination without owning global layout.
- An add-on registered in one WorkbenchApp namespace can contribute an app-local dialog or context-menu action without appearing in another app.
- A host-approved CompanionView can follow shared context in the AuxiliarySidebar, while global jobs remain visible in the OperationsPanel.
- A navigation plugin, contextual plugin and operations plugin can only register in their declared zone.
- A plugin can be disabled while the workbench remains usable.
- A trusted built-in and an isolated plugin can declare an English default bundle plus one translated locale; manifest labels and runtime UI strings resolve through the same namespaced `l10n` semantics.
- Locale selection and change use the host Profile/Session revision and re-project active Views, AppSurfaces, Previews, Overlays and Job UI without replacing the Workbench shell.
- Missing, malformed or argument-incompatible translations record a diagnostic and fall back through exact locale, language subtag, extension default locale and `defaultMessage`; disabling, upgrading or uninstalling the extension removes only its localization projections and caches.
- A RAW-like fake provider supports all three preview intents through one resolver.
- A metadata-only plugin augments the inspector without replacing the primary preview.
- Layout changes survive restart and pass a schema migration test.
- A long-running job reports progress, supports cancellation and leaves a diagnosable result.
- A Job policy carries a bounded monotonic deadline, cancellation grace, finite
  exponential retry policy, network class and background eligibility; the
  policy is identical across Rust, Tauri and TypeScript projections.
- A typed transient or timeout failure increments the attempt before retrying;
  permission, schema, invalid-input, deterministic and explicit-cancel errors
  do not auto-retry, and a late result from an older attempt is rejected.
- Job checkpoints are schema-versioned and size-bounded, are written only for
  the current attempt, survive restart, and let a paused runner resume without
  treating an unsupported checkpoint as core data.
- Suspend/offline state reaches `JobSystem` through one typed environment
  command; blocked Jobs show `Paused` with a reason and resume only after the
  environment becomes eligible.
- A Job's memory/disk/child-process/concurrency claim is visible in the
  persisted snapshot and plugin `job.run` context; soft CPU/network pressure
  delays new starts within a bounded window without turning into hard
  admission failure.
- On macOS/Linux, a fake platform source and the Tauri resume event drive the
  same environment command; missing network evidence is `unknown`, fixed
  command paths are used without a shell, and a monotonic wake gap briefly
  pauses background work before resuming eligible Jobs.
- Disabling a plugin while one of its Jobs is running cancels the Job before runtime shutdown; a late result cannot revive it, and retry reports a missing-owner-runner diagnostic until the plugin recovers.
- Replacing the frontend adapter does not change the Rust protocol or plugin manifest.
- No P0 acceptance scenario requires an Editor, text document model or LSP; a future text-oriented app may add that capability without changing the shell contract.
- A stale revision, sequence gap, transport disconnect, corrupt store, provider timeout, plugin crash, failed AppSurface switch, failed View/Overlay, cancelled Job and extension uninstall each leave a diagnosable usable shell and have an explicit recovery action.
- Every durable state field and cache names one owner, scope, schema version, migration, invalidation and recovery path; persistence adapters never become a second owner.
- A startup attempt records its phase and failure class; reaching `workbench-ready` writes a last-known-good checkpoint, while two pre-checkpoint failures or a core migration failure enter Safe Mode on the next launch.
- Safe Mode offers retry, disable the recently failing optional component, restore a validated owner snapshot, reset optional state and export diagnostics; it never silently clears core Asset, Workspace, Profile or Job data.
- When the Workbench cannot mount, `RecoverySurface` can display the failed owner, migration/quarantine reason, available generations and recovery actions without owning authoritative domain state.
- If the WebView/GPU path cannot mount, the same `RecoverySurface` contract is available through a minimal host-native retry/update/export action with no generic shell or filesystem capability.
- A normal owner write uses a same-directory temporary file and atomic replacement; an interrupted write leaves the previous committed generation readable.
- A related multi-owner write records a validated `RecoverySet`; readers use only the last committed recovery-index pointer, commit-barrier failure cannot publish mixed owner generations, and a single-owner restore is allowed only after reference validation.
- A core migration runs copy-on-write, validates schema/invariants/references before publication, preserves the previous generation until the startup checkpoint succeeds and quarantines failed staging data.
- A corrupt or unknown future schema is preserved as `QuarantinedState` with bounded metadata; an older binary never overwrites it with defaults.
- Restoring a Workspace/Profile owner creates a new validated generation and does not modify unrelated Job, Asset or extension state; extension restore remains inside its namespace.
- Restoring Asset/source state validates the external volume and path identity; it never copies, deletes or silently rebinds the external source file.
- An extension store migration failure disables or quarantines only that extension; built-in Workbench, core Asset/Workspace state and unrelated extensions remain usable.
- Restart recovery marks queued/running Jobs as interrupted and retryable, preserves owner/provenance metadata, rejects stale completions and treats missing provenance as a cache miss that can be recomputed.
- Session/Window recovery restores identity, bounds, focus and selection hints only; uncommitted renderer edits are not silently treated as durable work, and draft recovery requires its own owner scope.
- Derived preview/facet caches and recomputable `JobResult` payloads can be invalidated and rebuilt without restoring a global backup database.
- A diagnostic export contains bounded redacted metadata and log tails; Asset contents, credentials, secrets, full environments and unrestricted paths are excluded, and upload requires separate consent.
- Diagnostic records are structured, bounded by count/age/bytes and include monotonic timing; a separate `CrashCaptureAdapter` may add local exception/OOM evidence without owning Safe Mode or uploading data.
- The P0 lifecycle path works with the in-process `RecoverySupervisor`; an out-of-process supervisor is optional and is enabled only after cross-platform cold-start, idle-resource and crash-relaunch measurements.
- A new transport performs a typed handshake with a stable `clientId`, selects a compatible protocol major/minor, negotiates payload schemas/features and assigns a fresh `sessionId` before accepting mutations; reconnect retires the prior session.
- A protocol minor addition with optional fields remains usable; a major mismatch, unknown required field or incompatible schema returns a typed compatibility error and leaves the Workbench in a diagnosable degraded/recovery state.
- A duplicate request with the same `(clientId, requestId)` returns the bounded original result for an idempotent command; an unknown mutation outcome uses `query-request-outcome` before retry and never blindly replays an external side effect.
- Duplicate or stale events are ignored, a sequence gap triggers fresh-session authoritative snapshot resync, and a stale revisioned write returns the authoritative snapshot with a typed error.
- After transport loss the Workbench retains the last valid projection, enters bounded reconnect, marks the renderer read-only, applies the Layout/Profile/Recovery/Job/Extension snapshot without replaying commands and discards old-session envelopes.
- Plugin runtime and UI Bridge reconnects dispose stale sessions and recreate them from host descriptors after snapshot apply; an open Preview re-resolves its intent, and a failed optional feature remains isolated from the core shell.
- Resource admission applies device-global, owner and Job budgets in order through a versioned policy; an extension cannot exceed the host's memory, disk, file-descriptor, child-process or concurrency limits, and CPU/network pressure produces throttling or backpressure instead of unbounded work. A process-local device tier is not reported as cross-profile hard enforcement.
- A plugin runtime invocation respects the interactive deadline; a Job with a longer deadline must emit bounded progress heartbeats, and a timeout returns a typed result without restarting the Workbench.
- Cancelling a Job sets its cancellation path, the runner can observe the token, a healthy shared runtime is not killed for one cancellation, timeout/unhealthy runtime gets a grace period before process-group termination, and late progress/results from the old attempt are rejected.
- A retryable transient failure receives finite exponential backoff and a bounded attempt count; permission, schema, invalid-input, deterministic and explicit-cancel errors do not auto-retry.
- Repeated plugin crashes or failed recovery remove only that extension's projections, cancel its Jobs, quarantine it after the rolling failure limit and keep built-in Workbench apps and unrelated extensions usable.
- Before suspend, background admission stops and Job checkpoints persist; after wake, eligible local work resumes with rate limiting, while offline or metered-network Jobs wait for policy-approved connectivity instead of spinning retries.
- If a pre-suspend checkpoint cannot be persisted, the Job becomes interrupted/retryable and does not continue as if partial state were durable.
- Battery and thermal pressure may tighten soft CPU/network admission and defer background work, but never bypass durable writes or cancellation checkpoints.
- Deadlines, retry backoff and runtime grace periods use a monotonic clock; wall-clock changes affect only display/diagnostic timestamps.
- Disk pressure can stop new derived work and evict owner caches without deleting core Asset, Workspace, Profile or Job state; the OperationsPanel shows a typed resume/retry/cancel/discard action.
- The startup capability probe records OS, architecture, WebView, GPU and display-scale information; missing Tier A capability reaches `RecoverySurface` or a typed update action, while missing Tier B capability degrades only that integration.
- Tauri capability manifests deny by default; typed commands reject arbitrary filesystem, shell, URL and process inputs, external links use an explicit scheme/domain policy, and the recovery surface has a narrower capability set than the Workbench.
- A Profile lock is acquired before WebView/Tauri/Core startup; multiple windows share the same Core, and macOS/Linux verify the recorded process start token before reclaiming an absent/reused owner or forwarding a bounded schema-validated file/deep-link/command-line activation request. The primary stores accepted requests in a bounded native queue and exposes a typed drain command; the event is only a wake signal, so startup-time listener races do not lose requests. Malformed frames, missing owners and ambiguous identity inspection fail closed. No path removes a lock based on PID alone or triggers concurrent writes.
- Window restoration clamps missing-monitor or changed-work-area bounds to a visible display and records a repair diagnostic; high-DPI, IME, RTL, reduced-motion and accessibility checks pass through host adapters.
- A source on a removable or network volume remains registered when disconnected; the Rust source owner revalidates canonical regular-file state, the persisted `ApprovedSourceRoot` registry and recorded volume identity, Preview/Job operations return typed retryable `source-unavailable` or `PermissionDenied`, and reconnection revalidates root, volume identity and path before resuming work. Revoking a root denies existing bound sources without deleting Asset identity, while re-approving the canonical directory restores an explicit user recovery path. Legacy records without identity/root remain compatibility-only.
- File-watcher events are hints; the Asset/source owner revalidates `ContentFingerprint`, invalidates derived results through `JobSystem`, and scan-on-use or explicit rescan works when watcher events are unavailable.
- Native activation resolves existing regular files before they cross the Tauri event seam, rejects malformed file URIs and unsupported deep-link schemes, and keeps workspace routing out of the renderer transport. `ApprovedSourceRoot` membership, `approvedRoot` containment, symlink-loop and volume identity checks are Rust source-owner gates; host settings owns only the typed root policy projection and actions, while workspace consumption is host-owned and failed handlers leave intents pending. Drag/drop traversal and release mount qualification remain gates; the renderer never executes raw paths or commands.
- Clipboard, notification, tray and global-shortcut denial or absence falls back to an in-app action or no-op diagnostic and does not make core startup fail.
- A release package with an invalid signature, unknown key, mismatched hash,
  target, channel or architecture is rejected before installation and the
  current version remains launchable.
- Online update discovery with stale/expired freshness metadata or a release
  sequence below the installed floor is rejected before installation; an
  explicitly selected offline package still has to pass package signature,
  target and compatibility checks; a client below the manifest minimum version
  receives a typed update/recovery action, and a lower compatible version is
  available only through an explicit recovery action.
- A clearly skewed local clock blocks online discovery with a typed diagnostic
  without deleting data; Job deadlines and migration ordering remain monotonic.
- A signed key-rotation manifest is accepted only when authorized by the
  pinned release root key; revoked or unknown keys fail closed and leave a
  bounded diagnostic.
- A preview client does not silently move to stable, and a lower version is
  rejected unless an explicit compatible recovery action authorizes it.
- An eligible staged rollout downloads and verifies the package; an ineligible
  client keeps its current version without a retry loop.
- An offline installer with a valid signature, supported architecture, channel,
  version and release floor passes through the same verification path as the
  online updater;
  an invalid offline package is rejected before touching the installed version.
- A proxy timeout, offline check, interrupted download or interrupted install
  leaves the last committed package and data generation usable; partial bytes
  are never executable and staging is resumable or discarded.
- A migration that fails before the new startup checkpoint returns to the
  previous package and `StateSnapshot` generation, while failed package and
  staging evidence remain available to `RecoverySurface` and diagnostics.
- After a committed forward migration, the old binary is never started
  against the new schema; recovery requires an explicit compatible rollback or
  owner-scoped snapshot restore.
- The current `RecoverySet` retains its binary/data compatibility floor with
  user data, so uninstall/reinstall cannot bypass anti-rollback by clearing
  package-local metadata.
- A bundled extension is covered by the application signature, while an
  unsigned or revoked external extension is blocked by `PluginHost` without
  affecting core startup or unrelated extensions.
- Uninstall retains user data and snapshots by default; explicit data deletion
  is separate, confirmed and never removes external Asset sources implicitly.
- Cold-start and idle memory/disk/CPU baselines are recorded per supported
  platform; hidden/suspended WebViews do not run undeclared work, and recovery
  diagnostics distinguish slow startup from crash or blocked migration.

## Structure review checkpoints

Before adding a new directory, ask:

1. Does it own a deep interface or is it only a pass-through?
2. Does the new responsibility belong to a `Part`, a workspace, a zone or a domain module?
3. Can a fake adapter test the behaviour through the interface?
4. What happens when the owning plugin is disabled or upgraded?
5. Does the module introduce a second persistence owner for state already owned elsewhere?

For native lifecycle work, also require that `RecoverySupervisor` remains an
`application-core` boundary with an in-process P0 implementation; persistence
only stores journal/snapshot bytes; `tauri-adapter` only translates OS,
transport and package events; `CrashCaptureAdapter` is a separate diagnostic
seam; `RecoverySet` coordination does not become a global backup owner; and
every new recovery action has a deterministic owner seam and a failure-injection
fixture before a renderer flow is added. For plugin work, require that
`PluginHost` remains the owner of trust records, capability grants, runtime
failure domains, `ExtensionStorage` publication and diagnostic retention;
`JobSystem` remains the owner of Job lifecycle; sidecar launch is mediated and
process-group aware; and `BackupParticipant`/`MigrationParticipant` cannot
write another owner's state or publish recovery pointers.
