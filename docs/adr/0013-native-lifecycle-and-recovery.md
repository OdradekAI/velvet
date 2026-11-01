# ADR-0013: Native lifecycle and recovery

Status: Accepted
Date: 2026-10-31

## Context

Velvet is a local-first Tauri desktop workbench. Rust `ApplicationCore` owns
durable domain state, layout, plugin lifecycle and Jobs; the desktop renderer
projects that state through `CoreTransport`. A startup failure must not make
the user's assets or workbench inaccessible, and an extension, cache, WebView
surface or background Job must not take down the core.

The current skeleton already has owner-specific stores, schema fields,
atomic temporary-file replacement, plugin install rollback and restart-aware
Job snapshots. It does not yet have one lifecycle contract for startup
checkpoints, Safe Mode, migration rollback, quarantine, diagnostics and
recovery UI. VS Code provides useful evidence for lifecycle phases, atomic UI
state and extension-host isolation, but its thresholds, Hot Exit model and
process topology are not Velvet's contract.

## Decision

### Lifecycle owner and startup states

`ApplicationCore` owns lifecycle decisions. The Tauri adapter decodes commands,
creates the host surface and maps typed results; it does not own recovery or
persist business state.

Startup is checkpointed through these phases:

```text
preflight
  → recovering-state
  → core-ready
  → workbench-ready
  → optional-activation
  → ready
```

Each phase has a monotonic deadline from the startup policy. A deadline breach
is recorded as `startup-timeout`: core phases stop at a safe boundary and route
to Safe Mode/`RecoverySurface`, while optional activation stops scheduling
further components at the next safe boundary and remains degraded. A timeout
never force-kills the core or publishes staged state; it either completes the
owner transaction or quarantines it for the next attempt. Runtime-specific
cancellation remains the responsibility of `PluginHost`/`JobSystem`; the
lifecycle deadline is a host coordination bound.

The externally visible states are `starting`, `safe`, `degraded`, `ready` and
`terminal-failure`. A launch attempt records its id, version, phase and failure
classification. Reaching `workbench-ready` writes the last-known-good
checkpoint and clears the pre-checkpoint failure count; optional activation
after that point is not allowed to become a core startup dependency. An exit
before that checkpoint increments the count; two consecutive failures enter
Safe Mode on the next launch. Any core migration failure enters Safe Mode
immediately. An unattributed crash during optional activation is recorded with
the active component and is handled as a component failure before the host
considers a broader startup recovery action.

Safe Mode offers retry, disable the most recently failing optional component,
restore a validated owner snapshot, reset optional state and export a
diagnostic bundle. It never resets core Asset, Workspace, Profile or Job data
automatically. If the core is usable, the normal Workbench remains mounted and
shows host-owned diagnostics and Operations actions. If the Workbench cannot
mount, the Tauri host displays a minimal `RecoverySurface` that does not own
authoritative domain data.

### Recovery supervision

`RecoverySupervisor` is the only stable lifecycle recovery concept. It is a
recovery policy and evidence boundary, not a process that can supervise itself
after it has died. P0 uses an in-process implementation: `ApplicationCore`
records a bounded launch receipt, successful-start checkpoint, shutdown intent
and shutdown-complete attempt through the existing persistence adapter. The
completion record is written only after owner writes are flushed, but an
in-process component cannot certify that the process actually reached the OS
exit boundary. A launch without matching shutdown evidence is abnormal; a
leftover shutdown intent is classified as expected-or-uncertain and remains
diagnosable rather than being treated as proof of a clean exit.

The lifecycle journal and recovery index are not startup dependencies. If their
store is unavailable or corrupt, `RecoverySupervisor` starts with an in-memory
receipt, enters a diagnosable degraded/Safe Mode path and lets
`RecoverySurface` offer repair or export. It never replaces core data with
defaults merely because its own evidence store is damaged.

Crash evidence is a separate `CrashCaptureAdapter`. It may record a platform
exception code, signal, minidump reference or OOM classification when the
platform provides it, but it never uploads data or decides Safe Mode. The
adapter correlates evidence with a launch `instanceId`, `attemptId`, process
identity and optional runtime nonce. Native composition consumes bounded local
receipts once and stores them in the lifecycle journal; an external launcher or
platform crash handler may append the same typed receipt file without gaining
access to business state. An out-of-process `RecoverySupervisor` implementation
may be added later when a supported platform or product requirement proves that
automatic relaunch is necessary.

### Alpha implementation slice

The first implementation slice now provides an in-process
`RecoverySupervisor` in `application-core` backed by the persistence crate's
versioned `RecoveryJournalStore`. It records a launch receipt, startup phase,
last good checkpoint, abnormal pre-ready failure count and shutdown evidence
through the supervisor API;
two unfinished pre-ready launches enter Safe Mode, and core migration failure
forces Safe Mode without overwriting the failed owner state. The projection is
available through the canonical Rust protocol and the Tauri wire adapter, so a
renderer can inspect recovery state without reading the journal directly.
When Safe Mode is active, startup does not auto-activate discovered plugins;
the core and recovery path remain available for an explicit user action.

The Tauri composition root now records `ExitRequested` as shutdown intent and
`Exit` as shutdown completion through the same core-owned supervisor. This
keeps normal close evidence out of command handlers and leaves an unfinished
launch receipt available to classify forced termination on the next start.

`StartupDeadlinePolicy` is injected into `RecoverySupervisor` and uses the
monotonic `RecoveryClock`. The alpha defaults are deliberately conservative
(10 seconds for preflight/core/workbench phases, 30 seconds for state recovery
and optional activation); tests use shorter policies. Core phase breaches
enter Safe Mode immediately and retain their `startup-timeout` evidence, while
an optional activation breach records `startup-timeout` without making the
core unsafe. The activation loop checks the deadline between components and
`mark_ready` performs the final boundary check.

The desktop host now owns a renderer `RecoverySurface` and `RecoveryBoundary`.
Startup/layout failures and descendant Workbench mount exceptions render the
same projection without loading plugin UI or reading native storage. Retry
recreates the desktop composition and re-runs startup. The surface exposes the
host action list. Retry, previous-set restore, targeted quarantine and
user-initiated export of a bounded `RecoveryDiagnosticBundle` are enabled
through typed host commands; optional-state reset still waits for a separate
owner contract. The export contains only the recovery projection, a schema and
timestamp, truncates failure text to a bounded length, and never uploads data.
The Tauri composition also provides a minimal host-native recovery surface
through the native dialog plugin. Once Tauri host setup has completed, it is
shown for Safe Mode, orphan runtime evidence, platform evidence and repairable
extension storage damage, uses no renderer or plugin code, and offers only typed
restore/quarantine/close-evidence/continue actions. Its copy follows the
Rust-owned Profile locale with a bounded English fallback. The repair flow is
sequenced so a user can quarantine damaged extension state, then explicitly
forget a quarantined runtime lease; forgetting evidence never terminates a
process.

Before constructing the Tauri builder, the host runs a platform preflight probe
for WebView and GPU prerequisites. WebView unavailability starts the same
installed executable in `--velvet-recovery-window` helper mode. That mode is a
separate process, does not construct Tauri or a WebView, presents only fixed
typed recovery actions through the OS-native dialog facility, and writes a
bounded intent file consumed by the next Core startup. GPU degradation is
recorded and permits software fallback; an unverified capability is not treated
as a fatal failure. Tauri build/setup failures and panics from the run loop are
caught by the host, written as a bounded startup-failure marker and consumed
into the next launch's typed recovery projection. A failure before the binary
can execute still depends on the OS launcher or release wrapper.

The release artifact also contains an optional `velvet-launcher` wrapper. It
uses a fixed sibling application path (or an explicit test-only override),
passes through the original arguments, captures only a bounded stderr tail and
writes a typed `LauncherFailure` marker for loader/exec, wait and non-zero
child-exit failures. The next successful Core startup consumes the marker once
and records it as platform-initialization/crash evidence. The launcher has no
access to business state and never executes a shell command. A failure while
the launcher itself is being loaded remains an OS/package-manager boundary.

When an unclean receipt is present, the platform crash adapter scans only known
OS crash stores and recent files whose names identify Velvet. It accepts at most
four files and 64 MiB per file, hashes them while streaming, and projects
`CrashArtifactAttachment` metadata (format, size, digest and collection time)
without exposing paths or raw dump bytes. The metadata is consumed with the
receipt and is therefore one-shot. Permission errors, unsupported stores and
unparseable formats degrade to existing process/signal evidence; they never
block startup or trigger upload.

The persistence crate now provides a framework-independent `StateSnapshotStore`
primitive. It validates owner and scope segments, bounds payloads, records a
schema, generation, checksum and committed marker, publishes staged snapshots
through same-directory atomic replacement and can quarantine a staged or
committed generation. Its in-memory implementation is used by fault-injection
tests. This is a storage seam only; `ApplicationCore` has not yet migrated
its generic embedded constructors to it, and no cross-owner `RecoverySet` is
published by those constructors.

The native Tauri composition now wraps Layout, Profile and Job stores with that
primitive. A wrapper reads the committed generation named by the current core
`RecoverySet` when one exists, falls back to the legacy owner store only while
that owner has no snapshot, and writes the owner payload as a new staged
generation. A successful owner write proposes a new set by replacing only the
changed owner; all other members are copied from the current set. This prevents
startup persistence of one owner from reintroducing a generation that the user
just restored. The set pointer is published only after every referenced
generation validates.

`RecoverySetStore` supports a previous-set restore. The typed
`restore-recovery-set` action publishes that previous set for the next startup;
it does not mutate the live in-memory domain graph. A targeted
`quarantine-recovery-failure` action first points the index at the previous
set, then quarantines the failed owner generation. The recovery projection
includes the owner, scope and generation target only when the failure can be
quarantined, so the renderer never invents a path or generation.

The native file stores expose a durability mode. Unix Tauri builds request file
and parent-directory `fsync` before advertising a committed snapshot or set;
platforms without directory sync support use atomic rename only and remain
diagnosable as weaker durability. A failed required sync does not publish a
committed file. This is a crash-durability seam, not a claim that power-loss
survival has been proven on every filesystem.

Runtime failure in an activated optional plugin also records a degraded
component failure and clears that owner's projections and Jobs; it does not
increment the core pre-ready Safe Mode counter.

The native helper now supports a backward-compatible typed multi-page flow. Each
page is an evidence projection and the final action is still one of the fixed
retry/restore/Safe Mode intents. The bundle carries bounded crash evidence,
cross-process correlation, optional system artifact metadata and persisted
plugin/Job history. A successful
renderer `RecoverySet` restore or quarantine action requests a fresh startup so
the selected set becomes live; extension-store actions leave only that
extension disabled and do not restart the core.

`PluginHost` now provides the first extension-state slice on top of the same
snapshot adapter. Each manifest-declared store uses an owner namespace of
`extension.<id>` and scope `storage`, has a bounded quota, and commits through
generation compare-and-swap semantics. `BackupParticipant` receives a typed
logical view and its output is revalidated by the host; `MigrationParticipant`
receives a read-only source and a destination transaction. Neither participant
can publish a core `RecoverySet`, and a failed participant leaves the previous
extension generation and all other owners untouched. Isolated runtimes now use
the same contract through the internal JSONL `invoke-storage` protocol:
`storage.export` and `storage.migrate` receive only a bounded source schema,
generation and entry map, then return a bounded candidate entry map. The host
validates the response schema and quota, commits migration against the source
generation with compare-and-swap, and never publishes a core `RecoverySet`
pointer. A runtime rejection leaves the extension active but records a typed
storage diagnostic; a timeout, crash or malformed response fails and isolates
only that extension. The host now exposes a metadata-only extension storage
recovery projection and typed restore/quarantine actions. Restore requires a
same-schema previous generation and commits it as a new generation; quarantine
uses an exact-generation compare-and-swap. Both actions stop an active runtime
first and leave the extension disabled until explicit user activation. Raw
payloads, paths and core `RecoverySet` members stay inside Rust. Unknown or
corrupt current generations remain diagnostic evidence rather than being
silently deleted. Unknown/corrupt extension generations are repaired by
quarantining the exact generation discovered from the snapshot filename
metadata; a later restore can use the previous validated generation. Automatic
orphan policy now persists uncertain identities as `Quarantined` leases so a
future launch will not repeatedly inspect or kill them; the typed
`query-plugin-runtime-recovery` projection exposes only bounded evidence and
`close-quarantined-runtime-lease` removes one record after user review without
terminating a process. When an
external extension is activated after an installed upgrade and its committed
store has an older schema, activation runs `storage.migrate` before exposing
the extension as ready; a failed migration leaves the extension failed and
the prior generation authoritative.

Runtime and Job failures now also cross a typed diagnostic seam. `PluginHost`
keeps a bounded local queue with sequence, extension, failure kind/code,
redacted message, retryability and optional Job attempt; `ApplicationCore`
adds failure evidence from its Job boundary, persists a bounded event history
through an optional `DiagnosticEventStore`, and includes that history in the
user-initiated diagnostic bundle; `tauri-adapter` forwards
`PluginDiagnosticEvent` without owning retention. A corrupt or unavailable
diagnostic store marks observability degraded but never blocks Core startup.
This makes one extension's failure observable and recoverable while leaving
other extensions and Core available. Native crash capture is consumed through
the separate local receipt adapter; missing platform dump support degrades to
exit/signal, phase, process and bounded log evidence.
Optional plugin activation is recorded as a degraded component failure and does
not block the core-ready checkpoint. The file journal is an evidence
store, not yet a replacement for owner snapshots or a proof that writes are
crash-durable on every platform; the persistence durability report remains a
follow-up acceptance test. Shutdown completion is only evidence from the Tauri
event loop; it is not a proof against power loss or an OS-level kill.

Plugin crashes and timeouts remain inside `PluginHost`; plugin-owned Jobs
remain inside `JobSystem`; a WebView or AppSurface failure is handled by the
desktop host adapter. None of these failures restart the whole application.

### Durable state, snapshots and migrations

Every durable field names one owner, scope, schema version, invalidation rule
and recovery action. `persistence` is a storage adapter and recovery-index
adapter. It does not interpret domain schemas, run migrations or become a
second owner. `RecoverySupervisor` owns the lifecycle journal and recovery
index semantics; `persistence` stores their bytes and exposes atomic commit
primitives.

Each owner writes bounded rolling `StateSnapshot` generations. A snapshot
contains owner id, scope, schema version, compatibility metadata, generation,
timestamp, checksum and a committed marker. Payloads remain separate; the
shared recovery index records discovery and startup validation without
duplicating domain data.

Initial ownership is:

| Scope | Owner | Recovery rule |
| --- | --- | --- |
| Lifecycle journal and recovery index | `RecoverySupervisor` | Preserve the bounded launch evidence and committed generation metadata; corrupt index metadata is quarantined and rebuilt only from validated owner snapshots |
| Asset and source registration | Asset/source owner | Restore identity and metadata only after reference and volume validation; external source files are never copied, deleted or silently rebound by recovery |
| Profile | Profile state service | Restore the last validated generation or use safe defaults after preserving the failed state |
| Workspace and layout | Workspace/Layout service | Restore the last validated generation; core migration failure enters Safe Mode |
| Session/Window | Session service | Repair or discard optional restoration hints while keeping core usable |
| Job | `JobSystem` | Preserve lifecycle metadata and provenance; mark in-flight work interrupted and retryable |
| Extension | `PluginHost` | Migrate independently; quarantine only the failing extension store and disable that extension |
| Derived cache | Capability owner | Invalidate or delete and recompute through `JobSystem` |

Writes use a same-directory temporary file, bounded payloads and atomic
replacement. Where supported, the adapter flushes the file and parent
directory before publication. The adapter reports whether the platform can
provide crash-durable publication; a store that cannot make that guarantee
must not advertise its generation as committed. The previous committed
generation remains until the next startup checkpoint succeeds.

Owner writes are coalesced into bounded rolling generations. A migration or
application update always forces a pre-change generation. The default recovery
profile retains at least one prior validated generation and enforces count,
age and byte limits; exact values are release policy and are observable in
diagnostics. These recovery snapshots are not a user backup/export product.

When a change spans related owners, the recovery index records a `RecoverySet`
containing the participating owner generations and a commit barrier. A restore
may publish one owner alone only after reference validation proves that its
dependent owners remain compatible; otherwise it stages and commits the whole
set. `RecoverySetTransaction` is the persistence seam for the latter case: it
stages every changed owner, commits those bytes, and publishes the new pointer
last. Until the first complete set exists, snapshot-backed owner adapters keep
the legacy owner state visible; they never expose an individually committed
bootstrap generation. Individual owner bytes may be written before the barrier,
but readers use only generations referenced by the last committed
recovery-index pointer; unreferenced bytes remain staging/quarantine evidence.
The recovery index never copies domain payloads or changes their owners.

Migrations are owner-specific and copy-on-write:

```text
read previous committed generation
  → stage migration
  → validate schema, invariants and references
  → publish new owner generation
  → complete startup checkpoint
  → retire old generation by retention policy
```

Interrupted transactions never overwrite the previous generation. Startup
either completes a fully validated staged transaction or removes/quarantines
the staging directory and opens the previous generation. A corrupt or unknown
future schema is preserved as `QuarantinedState` with reason, checksum and
timestamp. Older binaries never overwrite an unknown future schema with
defaults. If the recovery index is damaged, startup rebuilds only from
validated committed markers and enters a diagnosable degraded state when it
cannot prove a consistent recovery point.

Core restore is an explicit owner-scoped operation that stages a new
generation, validates references and requires a restart checkpoint before it
becomes current. Extension restore cannot modify core Asset, Workspace,
Profile or Job data. Export and deletion are explicit and operate by owner
scope. Session/Window restoration hints recover identity, bounds, focus and
selection only; transient renderer state or uncommitted edits are not silently
promoted to durable data. A feature that promises draft recovery must give its
own domain owner a versioned draft scope and snapshot policy.

Recomputable caches and derived `JobResult` payloads are outside the core
backup contract. A Job result that represents user-authored or otherwise
authoritative domain data must be owned and snapshotted by that domain owner;
it cannot be hidden under the cache rule. Job snapshots retain enough state to
explain and retry interrupted work; missing or stale provenance is a cache miss
and resubmits through the owner runner.

### Diagnostics and user recovery

Diagnostics are local-first and owned by their source component. Source records
use structured fields for component, phase, event, severity, correlation/session
or Job id, monotonic timestamp and bounded message metadata. A
`DiagnosticBundle` may contain only bounded, redacted metadata: application and
protocol versions, OS/architecture, startup phase, exit code or signal,
checkpoint, component identity, relevant Job identifiers, crash-evidence
metadata and a short log tail. Asset contents, credentials, secrets, full
environment dumps and unrestricted paths are excluded by default. Local
records, crash evidence and quarantined staging data have count, age and byte
limits; pruning is itself diagnosable. Export is user initiated; upload is a
separate explicit consent action.

The `RecoverySurface` presents the failed owner, migration or quarantine reason,
available generations and safe actions. It does not repair storage directly or
become a second persistence owner. Its copy, keyboard order, focus restoration,
contrast and live status are provided by the built-in host locale and
accessibility layer, so recovery does not depend on a plugin, network or the
normal Workbench shell.

### Protocol compatibility and reconnect

`CoreTransport` uses one versioned envelope mapped explicitly through the Tauri
adapter. Protocol major versions must match; minor versions may add optional
fields and features. Payload schemas and feature ids are negotiated
independently. The current alpha `protocolVersion: 1` is treated as major 1,
minor 0 until the explicit major/minor wire shape is introduced.

Each transport starts with a typed handshake that selects the protocol,
features and payload schemas and assigns a fresh `sessionId`. Required envelope
metadata includes protocol, session, request, kind/method, payload schema,
revision/sequence and deadline. Unknown optional fields may be ignored;
unknown required fields and incompatible versions return typed errors.

`requestId` is a logical idempotency key only for commands whose contract
declares idempotency. The core keeps a bounded recent-result record for those
commands, scoped to the command owner and retention window. Queries may retry
after reconnect; mutations with unknown outcomes first query authoritative
state and reuse the same key only when their command contract allows it.
External side effects are never replayed blindly, and a request id by itself
does not make an external API call idempotent. Events use monotonic sequence
numbers and revisioned snapshots; duplicate events are ignored and sequence
gaps trigger bounded snapshot resync.

Reconnect retains the last valid projection in a read-only degraded state,
creates a fresh session, performs a complete authoritative snapshot resync and
recreates plugin/UI Bridge sessions from Rust-owned projections. No unbounded
event replay log is required, and old-session events or handles are ignored.

### Plugin and Job resource governance

`PluginHost` owns extension runtime health, process groups, capability grants,
restart/quarantine state and extension storage. `JobSystem` owns Job admission,
concurrency, cancellation, retry, progress, results and provenance. Both use
device-global → owner → Job budgets; persistence remains only their storage
adapter. A versioned `ResourceBudgetPolicy` supplies platform- and workload-
appropriate values. Memory, disk, file descriptors, child processes and
concurrency are hard admission limits; CPU and network are governed by
scheduling, capability, byte accounting and backpressure. Extensions cannot
widen the policy. Until a cross-profile coordinator exists, a process-local
budget must be reported as such rather than presented as a strict device-wide
limit.

The alpha `JobSystem` now owns a bounded ledger for the hard admission seam.
`JobBudgetPolicy` contains device, default-owner, per-owner and per-Job limits;
`submit_with_budget` and `retry_with_budget` reserve a typed request before a
Job becomes visible, and terminal/cancelled attempts release the reservation.
Restarted `Queued`/`Running` snapshots become `Interrupted` without inheriting
old in-memory reservations. The default request counts one Job of concurrency;
callers that know memory, disk or child-process needs must provide those
claims explicitly. CPU/network values are retained as soft pressure only and
are not used to reject admission. This is a process-local ledger, not a claim
of cross-process device enforcement, and it is intentionally not a new Tauri
or TypeScript scheduler contract.

The alpha policy must provide a bounded per-extension runtime, Job, staging,
event/log queue and free-space profile, but the numeric values are release
configuration rather than this architecture's compatibility promise. The
profile is selected from platform capabilities and measured workload classes;
large media Jobs must be able to request an explicit class instead of being
silently rejected by a desktop-sized constant.

User cancellation is cooperative. A healthy shared runtime is not terminated
merely because one Job was cancelled; protocol v1 marks the attempt cancelled,
the runner observes its token and late results are rejected. A deadline,
malformed response, crash or unhealthy runtime may receive a bounded grace
period and then be terminated as its own process group. Core in-process runners
cannot be force-killed and must observe deadlines and cancellation tokens.
Typed transient failures receive a finite attempt count with bounded
exponential backoff and jitter; permission, schema, invalid-input and
deterministic failures require explicit user retry. A repeated extension crash
or failed recovery quarantines only that extension.

The plugin-specific recovery contract is defined by ADR-0005: the default
failure domain is one runtime process or WASM instance per extension,
`ExtensionStorage` migrations run in staging, and extension backup participants
cannot publish global recovery pointers. A main-process crash or power loss
interrupts every child runtime; the next launch restores committed extension
generations and Job snapshots, then marks in-flight attempts interrupted. A
sidecar left behind after the Core exits is an orphan to reconcile, never an
independent writer of extension or core state. The current sidecar adapter
already bounds stderr, records exit evidence and cleans Unix process groups on
shutdown; Linux additionally arms a parent-death signal before exec. Package
launch allow-lists are enforced for discovered and installed packages;
the host now persists a bounded per-runtime lease with a starting-to-running
commit barrier, child process identity and atomic close on normal shutdown or
failure cleanup. The platform supervisor keeps this lifecycle seam explicit:
macOS records `proc_pidinfo` start time and process-group identity, then uses
the existing process-group cleanup plus next-start reconciliation because macOS
does not provide the Linux parent-death signal used here; Windows assigns the
child to a host-owned Job Object with `KILL_ON_JOB_CLOSE`, records process
creation time through `GetProcessTimes`, and terminates the Job Object as a
tree. The Windows Job Object is a lifecycle containment primitive, not a
security sandbox or a substitute for manifest capabilities. Linux, macOS and
Windows all reject an unreadable identity as uncertain and quarantine rather
than kill by PID alone.

The planned reconciler uses a host-owned per-runtime lease rather than a
plugin-visible callback. The lease binds extension id, committed package
generation/digest, runtime nonce, process identity and creation time. Startup
scans leases before optional activation, verifies a platform process
start-token/instance identity, and kills only a matching process group or
Job Object. PID reuse or an unreadable identity produces a quarantined
recovery item and no kill. A confirmed orphan removes its projections,
interrupts its in-flight Jobs and leaves committed extension snapshots
available for restore. The lease is atomic, bounded and local; it contains no
plugin payload or credentials. `PluginHost::reconcile_runtime_orphans`
consumes a replaceable platform reconciler and applies conservative outcomes:
confirmed matching identity can be terminated, a missing/different process is
merely closed, and an uncertain identity is quarantined with no kill. Native
composition wires the local lease store and the Linux platform reconciler. A
quarantined lease is persisted with `RuntimeLeaseState::Quarantined`, is marked
failed for the affected extension and is inert on later launches until an
explicit lifecycle action closes it; this prevents repeated startup probes and
preserves the evidence without making the process a repeated kill target.
The macOS and Windows identity and termination adapters are now implemented.
macOS intentionally retains the next-start reconciliation fallback for an
abrupt Core loss; platform release qualification must exercise that path,
Windows descendant cleanup, permission-denied inspection, PID reuse and
identity races. Tauri initialization failures
are recorded before the normal Workbench exists and become a
`platform-initialization` recovery failure on the next successful launch.
The platform adapter is private to `plugin-host`; `RuntimeProcessIdentity` is
stored only inside the host lease, so this change adds no Rust/Tauri command,
TypeScript envelope or plugin-visible capability. The existing typed recovery
projection continues to expose only bounded lease evidence and explicit close
actions.

Before system suspend, background admission stops, runners checkpoint and Job
snapshots persist. After wake, local work resumes when its owner and storage
are valid; offline or metered-network Jobs wait for a policy-approved network
event. Wake recovery is rate-limited and late results remain rejected by Job
id, attempt and provenance. Deadlines, retry backoff and grace periods use a
monotonic clock; wall-clock changes affect display/diagnostic timestamps only.
If a checkpoint cannot be persisted before suspend, the Job is marked
interrupted/retryable and is not allowed to continue as if its partial state
were durable.

### Platform compatibility and OS integration

The native platform boundary is an adapter. It decodes OS events and invokes
typed `CoreTransport` requests; Rust owns source identity, Profile locking,
activation validation and durable Session/Window consequences.

Tauri capabilities are deny-by-default and scoped per window/webview. The main
window no longer receives `core:default`; its manifest lists only event
subscription, the explicitly required child-WebView operations, dialogs and
registered shortcuts. There is no generic filesystem, shell, URL opener or
process-execution command. `tauri-plugin-fs`, shell and opener capabilities may
be used only through the smallest declared scope, and Rust repeats path, scheme
and argument validation because ACL configuration is not a substitute for safe
domain code. The standalone `RecoverySurface` helper does not create a Tauri
window or request any capability. A reserved `recovery-capability` manifest is
host-only and grants only event subscription for a future in-process recovery
window; the current no-WebView helper does not need it. The in-process surface
can only dispatch the same typed Core commands as the main window and cannot
activate a plugin or execute an arbitrary command. External-link actions use an explicit
scheme/domain policy and user confirmation; deep-link payloads still pass
through `ActivationRequest`.

Velvet uses a versioned Tier A/B support matrix. Tier A includes startup and
recovery, Rust persistence, WebView rendering, keyboard/IME, accessibility,
high-DPI windowing, file open/save and core transport. A missing Tier A
capability prevents the application from claiming `ready` and routes to
`RecoverySurface` or a typed update/repair action. `RecoverySurface` is a host
contract: a healthy WebView uses the minimal renderer, while a missing or broken
WebView/GPU path uses a native dialog or equivalent platform action for retry,
runtime repair, diagnostic export or safe launch. Tier B includes notifications, clipboard enhancements, tray, global
shortcuts, GPU acceleration, deep-link/file-association conveniences and
removable/network-volume integrations; failures degrade the affected feature,
record a diagnostic and keep the Workbench usable. Exact minimum OS, WebView,
architecture and signing versions are fixed by the release manifest and release
policy described below.

The Tauri composition catches builder/setup errors and run-loop panics instead
of allowing an initialization `expect` to become an unclassified process
failure. It stores a bounded marker beside the recovery journal and consumes it
on the next successful setup as `platform-initialization`; the marker contains
phase, bounded error text and preflight evidence, never arbitrary command or
plugin input. The preflight probe runs before the Tauri builder and routes a
fatal WebView result to the standalone native recovery helper. A failure before
the binary can execute still requires the launcher wrapper; failure before that
wrapper can execute still requires OS/package diagnostics.

A Profile has one active writer. Multiple windows share one `ApplicationCore`
and keep separate Session/Window restoration hints. A second process forwards
validated file, deep-link and command-line activation requests to the first
instance and exits after acknowledgement. A stale lock is recoverable only
after process identity verification using a recorded process start time and
instance nonce rather than a PID alone; concurrent Profile writes are never
merged.

If the Profile/state volume is read-only or unavailable, the core does not
silently switch to a fresh writable default. It enters read-only degraded mode
when the last validated state is available, otherwise opens `RecoverySurface`
with repair, move-data or export actions; new durable writes are rejected until
the owner can prove a committed destination.

Rust canonicalizes source paths before registration. Symlinks are allowed only
inside approved roots with loop/escape detection, and archive extraction
rejects symlink entries. Removable and network volumes preserve Asset identity
when disconnected; source operations return typed `source-unavailable` and
resume only after volume and path revalidation. Temporary absence never
deletes a source registration.

OS file-watcher notifications are hints, not authoritative mutations. The
Asset/source owner revalidates path, volume identity and `ContentFingerprint`,
then invalidates affected derived results through `JobSystem`. If a watcher is
unavailable or loses events, scan-on-use and explicit rescan remain valid
fallbacks; the renderer never becomes a second file-change owner.

File associations, drag/drop, command-line input and deep links use one typed
`ActivationRequest`. The adapter decodes the OS payload and Rust validates
scheme, path root, file type, capability and idempotency before queuing it
until startup reaches `ready`. Clipboard, notification, tray and shortcut
integrations are capability-scoped adapters with in-app or no-op fallbacks.
They do not resolve commands or mutate layout directly.

Window restore clamps missing-monitor or changed-work-area bounds to a visible
primary work area and records a repair diagnostic. IME, RTL, reduced-motion
and accessibility are host projections, and capability probes record WebView,
GPU, architecture, display scale and OS information. Sleep/wake and
offline/network changes follow the Job resource policy and do not create a
second persistence owner. Battery, thermal and metered-network signals may
tighten soft CPU/network admission and defer background work, but they never
bypass durable state writes or cancellation checkpoints.

### Performance and lifecycle evidence

Cold start, time to `workbench-ready`, idle memory, disk growth, background CPU,
WebView creation and hidden-window behavior are measured per supported
platform/profile. The ADR does not freeze unmeasured millisecond or megabyte
limits; a release profile records a baseline and a regression threshold before
calling a platform production-ready. Hidden or suspended AppSurface/WebView
sessions do not keep background work or network activity alive unless their
declared capability and Job policy permit it. The lifecycle journal and
diagnostics include enough monotonic timing data to distinguish a slow start
from a crash or a blocked migration.

### Release integrity and update recovery

Application release artifacts are trusted through a signed `ReleaseManifest`
and a platform-signed package. The manifest binds channel, semantic version,
target OS/architecture, artifact hash, updater schema, supported data-schema
range, minimum supported client version, monotonically increasing release
sequence, issuance/expiry metadata and signing key id. The installation records
the highest accepted release sequence and version floor. A client below the
manifest's minimum supported version receives a typed update/recovery action and
never rewrites user data. The public release root key is pinned in the
application. A key rotation is accepted only when a currently trusted key signs the new key
set during an overlap window; an unknown key, invalid signature, expired or
stale discovery metadata, hash mismatch, target mismatch or unsigned package
fails closed. Tauri's updater is the transport and artifact verifier; it does
not own rollout, migration or recovery policy.

The committed recovery-index metadata also records the minimum binary/data
compatibility floor for the current `RecoverySet`. Uninstalling package files
does not erase that floor while user data is retained, so reinstalling an older
binary cannot bypass anti-rollback checks by losing installation-local state.

Freshness checks use wall-clock time only for manifest validity and report an
untrusted or clearly skewed clock as a typed update diagnostic; they do not use
that clock for Job deadlines or migration ordering. A clock problem blocks
online discovery without deleting data, while an explicitly selected offline
package still requires signature, target and data-schema compatibility.

Velvet has `stable` and `preview` release channels. The selected channel is an
installation-scoped release setting. Updates do not silently switch channels
or downgrade. A signed manifest may limit a release to a deterministic rollout
cohort; an ineligible client keeps its current version and records a diagnostic.
The alpha distribution uses `preview`. Offline installers follow the same
manifest, signature, hash, target, release-floor and downgrade checks as online
updates. Discovery metadata expiry cannot be bypassed by an online client,
while an explicitly selected offline package still has to satisfy package
signature and compatibility checks.

Application package trust is separate from product-extension trust. A bundled
extension inherits the signed application package, but an installed extension
must satisfy `PluginHost` publisher and capability policy before activation.
The application signer is never a generic extension publisher. Unsigned local
extensions require an explicit developer-mode policy and are visibly
diagnosed; the production channel rejects them.

Updates use an `UpdateTransaction` journal and keep application and data
generations paired:

```text
discover manifest
  → verify channel, target, version/floor, sequence/freshness, signature, hash and compatibility
  → download to a private staging area (bounded timeout/proxy and retry)
  → snapshot owner state and record pending release
  → atomically stage/install the package while retaining the previous package
  → first launch preflight and copy-on-write data migration
  → validate schema, invariants, references and startup health checkpoint
  → commit the new package/data generation and retire old generations by policy
```

The previous package and last committed `StateSnapshot` generations remain
available until the new version reaches `ready` and writes its checkpoint.
Before that commit, a failed launch, interrupted installer or migration error
can abort the transaction and return to the old package plus old data
generation. Failed packages, staging data and new generations are retained as
bounded quarantined evidence for diagnostics. After the new generation is
committed, the old binary is not launched against the new schema: package
rollback becomes an explicit recovery operation and is allowed only when the
release declares backward compatibility; otherwise Safe Mode offers an
owner-scoped snapshot restore or a compatible reinstall. Data rollback never
rewrites unrelated owners.

Update checks and downloads use bounded timeouts, the configured proxy and
diagnosable offline behavior. A failed check does not block startup. A partial
download is never executable and is discarded or resumed only through the
updater adapter. An interrupted install is reconciled from the journal on the
next launch; it never deletes user data or silently replaces a committed
generation.

Uninstall removes application binaries and package-local staging data by
default but retains Profile, Workspace, Asset metadata, snapshots and
diagnostics. A separate, explicit user action deletes selected data after a
clear confirmation. External Asset sources are never deleted as a side effect
of uninstall.

## Implementation priority

1. **P0 startup safety**: lifecycle journal, checkpointed startup phases, Safe
   Mode thresholds, `RecoverySurface` and no automatic core-data reset.
2. **P0 persistence safety**: owner-scoped snapshots, atomic writes, migration
   staging, quarantine and restore of the last validated generation.
3. **P0 failure isolation**: plugin and Job recovery, interrupted Job state,
   provenance validation and derived-cache invalidation.
4. **P0 diagnostics**: redacted local records, host diagnostics/Operations
   projection, bounded retention and user-exported `DiagnosticBundle`.
5. **P0 native capability boundary**: deny-by-default Tauri capabilities,
   typed allow-listed commands and a minimal capability set for recovery.
6. **P1 compatibility and reconnect**: handshake negotiation, typed
   incompatibility errors, idempotent request handling, sequence-gap resync
   and bounded degraded reconnect.
7. **P1 platform, resource and performance policy**: Tier A/B capability
   probes, Profile lock and activation forwarding, path/volume validation,
   sleep/network behavior, versioned resource budgets, cold-start/idle
   baselines, cross-platform enforcement and OS integration fallbacks.
8. **P1 crash evidence and release recovery**: local crash-capture adapter,
   package verification, signing/key rotation, manifest freshness, staged
   updates and application rollback that never rewrites user data.
9. **Optional platform implementation**: an out-of-process supervisor or
   native relaunch adapter only after measurements and a platform-specific
   requirement justify the extra artifact.

The executable acceptance matrix is maintained in the [Workbench foundation
plan](../engineering/workbench-foundation-plan.md) and its decision ticket
[`Define native recovery acceptance scenarios and priority route`](../../.scratch/native-lifecycle-recovery/issues/09-define-native-recovery-acceptance-and-priority.md).
All P0 rows must pass before the native foundation is safe for alpha users.
P1 rows must pass for each supported platform, release channel and package
format before that combination is production-ready. The matrix requires owner
crate seams and failure-injection tests; a renderer smoke test cannot replace
the Rust owner test.

## Consequences

Positive:

- Core startup remains recoverable even when optional components or stores fail.
- Every durable state has one owner and an independently recoverable generation.
- Related owner generations can be restored as a validated `RecoverySet` without
  creating a global backup database.
- Migration failures preserve the previous state and avoid silent data loss.
- Derived data can be discarded and recomputed without blocking core recovery.
- A standalone recovery process reuses the signed application binary and does
  not add a second packaged supervisor or business-state owner.
- The recovery contract does not depend on a WebView being healthy, because the
  host can fall back to a minimal native recovery action.
- Signed release metadata and paired application/data generations make update
  failures recoverable without launching an old binary against a newer schema.

Costs:

- The recovery index, snapshot retention and quarantine need explicit disk
  budgets and cleanup rules.
- Related-owner commit barriers add coordination and reference-validation work
  without changing owner boundaries.
- Owner migrations must be tested as transactions, including interrupted
  writes and failed validation.
- Release engineering must maintain signing keys, channel manifests, old
  package retention and platform-specific installer rollback paths.
- Automatic relaunch, OS-before-launcher capture and platform-specific dump
  parsing remain release work; crash artifact metadata needs its own privacy
  and retention policy.

## Alternatives considered

1. **Mandatory external supervisor**: rejected for P0 because next-launch
   recovery does not require another packaged process and the helper would add
   signing, update, single-instance and lifecycle coordination cost.
2. **One global backup database**: rejected because it couples unrelated owners,
   makes extension failures block core startup and complicates selective restore;
   a `RecoverySet` supplies only the coordination needed for related owners.
3. **Automatic reset on migration failure**: rejected because it can silently
   discard user state; Safe Mode preserves the failed generation and offers an
   explicit recovery action.
4. **Back up every cache and derived result**: rejected because stale outputs
   consume disk and can be recomputed from provenance.
5. **Trust every package with the application signer**: rejected because a
   signed application must not silently widen extension publisher or capability
   trust.
6. **Always roll back only the binary**: rejected because an old binary may not
   understand a forward-migrated data schema; binary and data generations stay
   paired until the health checkpoint.
7. **Use fixed memory/disk constants as the architecture**: rejected because
   workload and platform capabilities differ; the versioned budget profile is
   the compatibility boundary.
8. **Treat a log tail as crash reporting**: rejected because it does not provide
   native crash evidence, structured redaction or a consented export boundary.

## Deferred implementation policies

These items remain release and platform engineering work. They do not reopen
the ownership, state-machine or recovery guarantees in this ADR:

- Which supported OSes can enforce runtime working-set and process-group limits
  directly, and which require scheduler/admission fallbacks?
- Which platform crash-capture APIs can provide useful local evidence without
  requiring an always-on helper, and what redaction/retention policy applies?
- What cross-profile coordinator, if any, is needed before the device tier of
  `ResourceBudget` can be a strict cross-process limit?
- Which release service owns rollout-cohort assignment and key revocation
  distribution, while keeping the client fail-closed when it is offline?
- Which supported OS installers can atomically retain the previous package,
  and what platform-specific adapter is needed when they cannot?

## References

- [ADR-0001: System topology and core ownership](0001-system-topology-and-core-ownership.md)
- [ADR-0002: Asset domain and derived data](0002-asset-domain-and-derived-data.md)
- [ADR-0005: Plugin host runtime, permissions and lifecycle](0005-plugin-host-runtime-and-permissions.md)
- [ADR-0006: Host protocol and framework-independent UI SDK](0006-host-protocol-and-ui-sdk.md)
- [ADR-0007: Jobs, indexing and cache invalidation](0007-jobs-indexing-and-cache-invalidation.md)
- [Native app and Tauri practices research](../research/native-app-tauri-practices.md)
- [Native lifecycle and recovery decision review](../research/native-lifecycle-recovery-review.md)
- [Native lifecycle/recovery Wayfinder map](../../.scratch/native-lifecycle-recovery/map.md)
