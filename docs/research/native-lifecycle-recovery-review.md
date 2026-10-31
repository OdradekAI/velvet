# Native lifecycle and recovery decision review

Date: 2026-10-31

This review re-checks the decisions in [ADR-0013](../adr/0013-native-lifecycle-and-recovery.md)
against [Native app and Tauri practices](native-app-tauri-practices.md), the
local VS Code checkout at `/Users/luiz/Odradek/vscode`, and Velvet's existing
ownership seams. The purpose is to record the consequences and counterexamples
before implementation, not to copy VS Code's Electron topology.

## Decisions that remain sound

| Decision | Why it survives review | Boundary or counterexample kept explicit |
| --- | --- | --- |
| `ApplicationCore` owns lifecycle and recovery policy | It keeps durable state and recovery decisions in one Rust owner; Tauri remains an adapter. This matches the existing topology and prevents renderer-specific repair paths. | A WebView failure may prevent the renderer from mounting, so the recovery contract also needs a host-native fallback action. |
| Two pre-checkpoint failures enter Safe Mode; core migration failure enters Safe Mode | The threshold is deterministic, conservative and user-selected. It protects against crash loops without resetting user data. | Safe Mode is a next-launch policy; it cannot restore transient in-memory UI state that was never persisted. |
| P0 uses an in-process recovery journal | The next launch can classify an abnormal exit without adding a signed, updateable helper to every package. | It cannot relaunch a dead process or produce a native minidump. Those are separate, optional adapters. |
| Owner-scoped snapshots, quarantine and copy-on-write migration | They preserve failed bytes, isolate extension damage and prevent an old binary from opening a forward schema. | Related owners need a coordinated commit/recovery set; independent “latest” generations can otherwise create cross-owner skew. |
| Fresh-session handshake and full snapshot resync | It avoids replaying side effects and invalidates stale handles after a WebView or transport restart. | A `requestId` is not magic idempotency; only explicitly idempotent commands may use the bounded dedupe record. |
| Plugin and Job failure isolation | It follows VS Code's extension-host lesson while preserving Velvet's Rust-owned Job and data semantics. | Healthy runtime cancellation must remain cooperative; process termination is for timeout or unhealthy runtime, not ordinary user cancel. |
| Tier A/B platform policy and one Profile writer | It gives each OS integration a visible failure contract and prevents concurrent Profile writes. | Tier A recovery must include a native/host fallback when the WebView itself cannot load. |
| Signed channels and paired package/data update generations | A valid package alone cannot make a forward data migration safe. Keeping old package and data generations together gives an actual rollback point. | A signed manifest also needs freshness and anti-rollback metadata; a valid old signature must not permit replay or silent downgrade. |
| Local-first, redacted diagnostics with explicit export/upload | It protects asset contents and credentials while still making failures actionable offline. | Structured fields, retention bounds and optional crash evidence must be defined; a log tail by itself is not a crash pipeline. |

## Decisions changed or narrowed

### Recovery supervision and crash evidence are separate

`RecoverySupervisor` remains the lifecycle policy and journal boundary. It does
not claim to supervise a live process from inside that process. A future helper
or platform crash handler may implement `CrashCaptureAdapter` for local crash
evidence, and a separate helper may implement automatic relaunch, but neither
owns business state or changes the Safe Mode policy. This avoids reintroducing
the overloaded `watchdog` concept and follows the research distinction between
restart supervision and crash capture.
The journal and recovery index are also non-blocking infrastructure: if their
store is damaged, startup uses an in-memory receipt and exposes repair/export
rather than turning recovery metadata into a new startup dependency.

### Snapshots use a coordinated recovery set when owners are related

Owner-specific stores remain the data owners, but the recovery index also
records a `RecoverySet`: a vector of owner generations committed under one
barrier for a related mutation or migration. A restore may publish one owner
alone only when its reference validator proves that dependent owners remain
compatible. Otherwise it stages the complete set and commits it atomically from
the recovery point of view. This avoids restoring a Workspace generation that
references a newer Profile or Asset registration.

Rolling snapshots are recovery copies, not a user backup product. Every
committed owner write is coalesced into a bounded generation; migrations and
updates force a pre-change generation. The implementation must retain at least
one prior validated generation and enforce count, age and byte budgets. The
exact retention profile is release policy, not a hidden persistence rule.

### Resource numbers become a versioned policy profile

The three-level budget model remains, but the former fixed values (for example,
256/512 MiB and 512 MiB staging) are not durable architecture. They can reject
legitimate RAW/video jobs on one device and still be too permissive on another.
`ResourceBudgetPolicy` therefore supplies platform- and workload-aware values.
Memory, disk, file descriptors, child processes and concurrency are hard
admission limits; CPU and network use throttling, byte accounting and
backpressure. The policy is versioned, observable and never widened by an
extension. Cross-profile, cross-process “device-global” enforcement is only a
hard guarantee once a coordinator exists; before that, the host must report the
scope honestly rather than pretend a process-local counter is global.

### Job cancellation is distinct from runtime failure

User cancellation first marks the Job attempt cancelled and relies on the
runner's cancellation token. A healthy shared plugin runtime is not killed
merely because one Job was cancelled. A deadline, malformed response, crash or
unhealthy runtime may terminate the affected process group after its grace
period; all affected Jobs receive typed interrupted/failure outcomes and late
results are rejected. A runner without a checkpoint resumes from the beginning
after restart rather than pretending partial progress is durable.

### The recovery surface has a host-native fallback

`RecoverySurface` is a host contract with a renderer implementation when the
WebView is healthy. If the WebView, GPU path or renderer cannot mount, the
Tauri adapter must provide a minimal native dialog or equivalent platform action
for retry, update/runtime repair, diagnostic export and safe launch. It must not
load plugins or expose a generic filesystem/shell command while in this state.
The recovery surface uses built-in localized copy and host accessibility
semantics, so a broken Workbench or offline network cannot remove the user's
only recovery path.

### The remaining native gaps are now explicit acceptance work

Startup phases have monotonic deadlines; file-watcher events are treated as
hints and fall back to fingerprint revalidation/scan-on-use; battery and
thermal pressure may defer soft background work; and wall-clock changes do not
alter retry/deadline behavior. Release metadata now includes a minimum client
version so a security or compatibility floor can produce a typed update action
without rewriting data. These are policy and test seams, not reasons to add a
new persistence owner.

### Release metadata gets freshness and anti-rollback checks

`ReleaseManifest` now binds a monotonically increasing release sequence,
issuance/expiry metadata, a supported data-schema range and a minimum client
version in addition to its signature, target and hash. The installation records
the highest accepted sequence/version floor. Offline installation uses the same
signature and target checks; an older compatible package is an explicit
recovery action and cannot silently rewrite a newer data schema. The floor is
stored with the retained data generation as well as installation metadata, so
uninstall/reinstall cannot bypass it.

## Rejected alternatives

- A mandatory watchdog/helper would add a second signed artifact and lifecycle
  failure boundary before the product needs automatic relaunch.
- One global backup database would make extension corruption and selective
  restore harder, and would still need a commit barrier for related owners.
- Fixed memory and disk constants in the ADR would turn an unmeasured alpha
  default into a compatibility promise.
- A log plugin alone is not a crash reporter; it lacks native crash evidence,
  structured redaction and a consented export boundary.
- Binary-only rollback is unsafe after a forward schema migration.

The resulting changes are incorporated into ADR-0013 and the foundation plan.

## Plugin developer re-review

The lifecycle decisions remain valid for extensions, but the plugin ecosystem
adds constraints that the first review did not spell out. The detailed
analysis is in [Plugin ecosystem lifecycle and recovery review](plugin-ecosystem-lifecycle-and-recovery-review.md).

- `builtin`, `wasm`, `sidecar` and isolated UI are different runtime forms;
  `extensionKind` and contribution kind remain independent. A sidecar is a
  process failure boundary, not a security sandbox, while a built-in adapter
  shares the Core failure domain and is therefore first-party only.
- The default failure domain is one runtime process or WASM instance per
  extension. A crash, timeout, malformed response or budget kill removes only
  that extension's projections and Jobs; a main-process crash or power loss
  interrupts every child and is recovered from committed snapshots/checkpoints
  on the next launch.
- `ExtensionStorage`, `BackupParticipant` and `MigrationParticipant` let a
  plugin preserve and evolve its own data. `PluginHost` still owns snapshot
  publication, `RecoverySet`, quotas, validation and user restore actions;
  plugins cannot restore core Asset, Workspace, Profile or Job state.
- Package signature, publisher trust, provenance/SBOM and paid entitlement are
  separate records. Velvet should not provide a generic paid-signature API;
  an eventual marketplace may expose a read-only host-verified entitlement
  handle with bounded offline grace.
- Job/runtime crashes produce structured host events with extension/session/
  Job correlation, exit or trap evidence, budget decisions and redaction. A
  plugin log file is not the crash pipeline.

The current alpha process runtime proves the intended seam but still needs
adapter hardening before production: allow-listed package-relative launch
targets, bounded stderr capture, process-group/parent-death cleanup, orphan
reconciliation and typed runtime event records. These are implementation gates,
not reasons to weaken the ownership model.

## Evidence used from VS Code

The local checkout was used as an implementation reference, not as a
compatibility target:

- `src/vs/code/electron-main/main.ts` and `app.ts` show staged service startup,
  lock-file/second-instance handling and crash-reporter initialization.
- `src/vs/platform/backup/electron-main/backupMainService.ts` shows why Hot Exit
  and editor backup data are a separate recovery owner rather than a replacement
  for Velvet's Asset/Workspace snapshots.
- `src/vs/platform/utilityProcess/electron-main/utilityProcess.ts` records
  component-specific exit reasons and health boundaries, supporting Velvet's
  decision to isolate plugin/runtime failures.
- `src/vs/platform/launch/electron-main/launchMainService.ts` demonstrates that
  activation forwarding and window reuse are platform policy, not domain state.

The counterexamples are deliberate: VS Code's editor backup model does not
justify putting Velvet's source identity or Job results into a single hot-exit
store; Electron's helper/process topology does not justify shipping a mandatory
Velvet watchdog while the Rust core remains a single local deployment unit.
