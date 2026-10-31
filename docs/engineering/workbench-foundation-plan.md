# Workbench foundation plan

Status: In progress; topology and desktop renderer decisions accepted, first React shell slice implemented; business-independent Workbench foundation is the active first-stage scope
Date: 2026-10-28

This plan is intentionally a skeleton. It establishes seams and ownership without pretending that the unresolved domain decisions are already accepted. The broad architecture decisions are tracked under `.scratch/workbench-foundation/`; the active first-stage shell decisions are tracked under `.scratch/workbench-shell-foundation/`.

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
backpressure. Cancellation is cooperative: a healthy shared runtime is not
killed for one cancelled Job, while timeout or runtime failure receives a
bounded grace period before process-group termination. Typed transient failures
use finite exponential backoff. Suspend/wake and offline/metered-network
transitions persist Job checkpoints and pause/resume eligible work without busy
retries. Until a cross-profile coordinator exists, a process-local device tier
must be reported as such rather than claimed as a strict cross-process limit.

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
online updater. Tauri's updater is only the transport/verifier. An
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
   journal, two-failure Safe Mode, migration-failure Safe Mode and the minimal
   `RecoverySurface`.
2. **P0 persistence safety**: owner-scoped rolling `StateSnapshot` generations,
   atomic replacement, migration staging, quarantine and restore of the last
   validated generation.
3. **P0 failure isolation**: PluginHost and JobSystem recovery, interrupted Job
   state, provenance checks and derived-cache invalidation.
4. **P0 diagnostics**: structured, bounded redacted local records, host
   diagnostics and user-exported `DiagnosticBundle` with separate upload
   consent.
5. **P0 native capability boundary**: deny-by-default Tauri capabilities,
   typed allow-listed commands and a minimal recovery capability set.
6. **P1 protocol and reconnect**: feature/schema negotiation, typed
   incompatibility errors, bounded reconnect and stale projection handling.
7. **P1 platform/resource/performance policy**: sleep and network behavior,
   versioned hard/soft budgets, OS/WebView fallbacks, cold-start/idle baselines
   and lifecycle tests on each supported OS.
8. **P1 crash evidence and release recovery**: local crash-capture adapter,
   package verification, signing/key rotation, manifest freshness, staged
   update and rollback rules that preserve user data.
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
   bounded stderr, process-group/parent-death cleanup and orphan sidecar
   reconciliation. A sidecar must not be described as a security sandbox.
4. **P1 package trust**: signed extension package envelope, publisher trust
   record, revocation/freshness/anti-downgrade checks and developer-mode
   unsigned-install policy. Provenance/SBOM references are separate metadata.
5. **P1 ecosystem operations**: update/disable/uninstall retention, offline
   package admission, marketplace review/block-list hooks and SDK simulator
   fixtures.
6. **Deferred P1/P2 entitlement**: only after a marketplace/account service is
   selected, add a read-only host-verified `EntitlementHandle` with bounded
   offline grace. Package signatures never become payment proofs.

### Current skeleton versus target contract

The two lifecycle reviews changed the implementation route, not the ownership
model. The current repository is a contract-bearing skeleton: several seams,
projections and narrow failure tests exist, while the durable recovery kernel
and plugin trust/data boundaries are still absent. The following matrix is the
baseline for sequencing work. “Partial” means that a useful seam exists but it
must not be advertised as the target guarantee.

| Target contract | Current skeleton evidence | Remaining gap | Priority |
| --- | --- | --- | --- |
| `RecoverySupervisor`, startup phases and two-failure Safe Mode | `application-core` now records a versioned launch journal, phase/checkpoint/deadline state, shutdown evidence and core-migration/optional-component failures; Tauri uses the file journal and records `ExitRequested`/`Exit` | User recovery actions and host-native `RecoverySurface` are still missing | P0 (partial) |
| Owner snapshots, `RecoverySet` and migration rollback | Layout/Profile/Job/source stores use schema fields and temporary-file replacement; layout has local migrations | No generation metadata, commit barrier, recovery index, fsync/durability report, quarantine model or cross-owner restore | P0 |
| Extension-owned data with host-controlled backup/migration | Plugin install has staging/rollback directories and a bounded install recovery log | No namespaced `ExtensionStorage`, backup/migration participant handles, extension generations or selective extension-state restore | P0 |
| Per-extension runtime failure domain | `PluginHost` tracks one process per extension, health state and contribution cleanup; `JobSystem` marks restart work interrupted; optional activation failures now degrade the recovery projection without blocking Core | Sidecar launch accepts a manifest program, stderr is discarded, process groups and parent-death cleanup are absent, and runtime events are not structured or persisted | P0 |
| Resource-governed, restartable Jobs | `JobSystem` persists snapshots/results, supports cooperative cancellation, retry and provenance matching | No owner/device/Job budget ledger, deadlines, cancellation grace, exponential backoff policy, checkpoint contract or suspend/offline policy | P0/P1 |
| Host diagnostics and user recovery | Plugin install recovery log and renderer recovery operations panel exist | No common `DiagnosticEvent`, bounded redaction, `DiagnosticBundle`, crash-capture adapter or lifecycle/Job/plugin correlation | P0 |
| Versioned protocol handshake and reconnect | Rust/TypeScript envelopes carry exact `protocolVersion` and `requestId`; stale layout revisions are checked | `u16` is treated as one exact version; no major/minor negotiation, feature/schema selection, `sessionId`, sequence resync or unknown-outcome query rule | P1 |
| Deny-by-default native boundary | Tauri command handlers map typed requests and a capability file scopes the main window | `core:default`, WebView creation permissions and `csp: null` are still alpha policy; recovery has no separate minimal capability set or negative command matrix | P0 |
| Extension package trust and release recovery | Manifest/schema/host API checks and application release signing are separate seams | No extension signature envelope, publisher trust record, revocation/freshness/anti-downgrade or visible unsigned developer mode; updater/package staging is inactive | P1 |
| Platform lifecycle and single-writer profile | A single `ApplicationCore` is composed for the Tauri process and renderer surfaces can suspend | No profile lock, second-instance activation forwarding, deep-link/path/volume policy, sleep/wake signal or Tier A/B runtime probe | P1 |

This matrix also prevents a common false completion signal: an install
transaction rollback protects package bytes, but it does not protect extension
state; a Job snapshot marked `Interrupted` explains restart state, but it does
not make the work resumable; and a sidecar process boundary limits availability
failures, but it does not grant a security sandbox. The in-process
`RecoverySupervisor` remains the P0 implementation. A helper process, full
WASM component runtime, marketplace entitlement service and remote connector
are later adapters with separate evidence, not prerequisites for this kernel.

### Next implementation route

The next work should follow dependency order and land as vertical slices with
fault-injection tests:

1. **P0 lifecycle kernel**: add the `RecoverySupervisor` trait, launch receipt,
   startup phase/deadline state machine, Safe Mode decision and the minimal
   recovery protocol/surface. Keep the journal failure-tolerant and make the
   recovery surface usable without the normal Workbench or a plugin.
2. **P0 durable generations**: introduce storage primitives for committed
   generations, checksums, atomic publication and quarantine. Build the
   recovery index and `RecoverySet` barrier, then migrate Layout/Profile/Job
   stores to those primitives before adding new product data.
3. **P0 extension state and runtime evidence**: define the host-owned
   `ExtensionStorage`, backup/migration participants and `PluginDiagnosticSink`.
   Harden the sidecar adapter with package-relative allow-lists, bounded stderr,
   process-group cleanup, exit evidence and orphan reconciliation. Wire failure
   events to projection/Job cleanup and late-event rejection.
4. **P0 Job governance**: extend `JobSpec` and `JobSystem` with owner budgets,
   deadline and cancellation-grace semantics, typed retry classes, checkpoint
   payloads and lifecycle/network pause signals. Keep CPU/network as soft
   scheduling budgets until measurements justify hard enforcement.
5. **P0 recovery UX and native ACL**: implement `DiagnosticBundle`, redaction
   and export; add a host-only recovery capability set and command-negative
   tests; tighten the current Tauri capability/CSP policy before exposing more
   plugin UI or native integrations.
6. **P1 canonical transport**: evolve Rust and TypeScript together to a
   major/minor handshake with fresh sessions, feature/schema negotiation,
   authoritative snapshot resync and idempotency handling for unknown mutation
   outcomes.
7. **P1 trust, platform and release**: add extension package signatures and
   trust records, profile locking/activation forwarding, path/volume policy,
   crash evidence, staged package/data update rollback and Tier A/B probes.
8. **Deferred ecosystem services**: only after the local contract is stable,
   select a marketplace/account provider for read-only entitlement status,
   implement a production WASM/component adapter, and evaluate an optional
   out-of-process supervisor from measured crash/relaunch requirements.

Every step must add the corresponding acceptance row before moving to the next
boundary. Until steps 1–5 pass, Velvet should treat the plugin SDK and package
marketplace as alpha/internal surfaces rather than promise the full recovery or
security behavior described by the ADRs.

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
7. **Plugin host**: load manifests, validate permissions and localization bundles, activate lazily, isolate failures and expose the UI SDK boundary. **Local discovery, install preflight, permission enforcement, host API compatibility checks, numeric version upgrades, transactional commit/rollback, crash-recovery scanning, deferred activation, retryable recovery, orphan cleanup and bounded recovery logs are implemented; application release signing is a native release gate, while extension publisher signing/revocation remains a separate PluginHost policy and is not inherited from the application signer. The target plugin gate additionally requires explicit runtime trust/sandbox policy, declarative no-runtime contributions, namespaced `ExtensionStorage`, host-orchestrated backup/migration participants, structured diagnostic events, process-group/orphan cleanup and signed package trust records before a public SDK is released. The host localization service, fallback formatter, diagnostics, loader/plugin lifecycle seam, isolated UI Bridge locale projection, Rust-owned revisioned Profile locale persistence, bounded directory-package localization resource loader, bounded ZIP archive package adapter and native directory/ZIP package picker are implemented; other archive formats and non-localization binary resources remain follow-up adapters.**
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
| P0 | Forced core termination is classified on next launch and recovers without a helper process; leftover shutdown intent is reported as expected-or-uncertain rather than proof of a clean exit. | `application-core` | Shutdown-intent/completion and crash-receipt process harness | Relaunch, enter Safe Mode or export diagnostics |
| P0 | Interrupted/truncated writes preserve the previous `StateSnapshot`; unknown schemas and a corrupt recovery index become `QuarantinedState`, journal/index failure falls back to an in-memory receipt, and a read-only/unavailable state volume never causes silent default overwrite. | `persistence`, `application-core` | Temp-directory store with write/rename/fsync/index and read-only-volume fault injection | Restore, retry migration, repair/move/export or discard quarantined state |
| P0 | A related multi-owner change publishes a validated `RecoverySet`; a fault cannot expose mixed Profile/Workspace/Asset generations. | `application-core`, `persistence` | Commit-barrier and reference-validation fault injection | Restore the recovery set, retry or export diagnostics |
| P0 | Failed core migration is copy-on-write and returns to the previous generation. | `application-core`, `persistence` | Migration fixture failing at each stage and reference validation | Retry migration, restore the last validated generation or export diagnostics |
| P0 | Failed extension migration quarantines only that extension store. | `plugin-host`, `persistence` | Fake extension store plus manifest/activation fixture | Retry, disable, restore or discard extension state |
| P0 | Plugin hang/panic/restart loop removes only its projections and Jobs. | `plugin-host` | `FakeRuntime`/`FakeProcess` timeout and rolling-failure plans | Retry, restore package, disable or uninstall extension |
| P0 | Restarted Jobs become interrupted/retryable; cancellation and late-result rejection work. | `job-system` | Fake `JobRunner`, `JobStateStore`, attempt/provenance fixtures | Resume/retry, cancel or discard Job |
| P0 | Diagnostic export is bounded/redacted and user initiated. | `application-core`, `tauri-adapter` | Deterministic bundle and export-adapter fixtures | Export locally; upload only after separate consent |
| P0 | Tauri capabilities and native commands are deny-by-default, typed and scoped; recovery cannot invoke arbitrary filesystem, shell, URL or process actions. | `tauri-adapter`, `application-core` | Capability/ACL fixture plus command-negative tests | Use the typed fallback, retry with a permitted action or export diagnostics |
| P0 | Declarative, WASM, sidecar and trusted built-in forms negotiate the same typed contract; runtime trust/sandbox policy is explicit, and a sidecar is never presented as a security sandbox. | `plugin-api`, `plugin-host`, `workbench-protocol` | Manifest fixtures, trust-policy matrix and capability-negative tests | Inspect permissions, enable only with explicit trust, disable or use a safe alternative |
| P0 | `ExtensionStorage` is namespaced and transactional; a `BackupParticipant` export and `MigrationParticipant` staging failure quarantine only extension state and cannot publish a core `RecoverySet` pointer. | `plugin-host`, `persistence`, `application-core` | Fake extension store, staged migration/export, checksum and power-loss fixtures | Retry, restore the previous extension generation, disable or discard extension state |
| P0 | One extension runtime crash, WASM trap, timeout, malformed response or budget kill removes only that extension's projections and Jobs; a second extension continues. | `plugin-host`, `job-system` | One-process-per-extension fake runtimes, process-group and late-result fixtures | Retry/recover the extension, cancel/retry its Jobs or continue with the fallback |
| P0 | A Core crash or forced power loss marks all child runtimes and in-flight extension Jobs interrupted; committed extension stores remain recoverable and orphan sidecars cannot write after restart. | `application-core`, `plugin-host`, `job-system` | Forced-termination harness, parent-death/orphan reconciliation and snapshot fixtures | Relaunch, resume/retry safe Jobs, quarantine the affected extension or export diagnostics |
| P0 | Plugin logs and runtime failures produce bounded structured events correlated to extension/session/Job attempt; secrets, asset payloads and unrestricted paths are redacted. | `plugin-host`, `application-core` | Diagnostic sink fake with stderr/exit/trap/secret fixtures | Export a redacted bundle; retry or disable the extension |
| P1 | Major/required/schema protocol mismatch fails before mutation; additive minor fields work. | `workbench-protocol`, `application-core` | Versioned wire fixtures and fake `CoreTransport` | Update, disable incompatible feature or open recovery UI |
| P1 | Transport loss keeps the last projection read-only and performs fresh-session full resync. | `workbench-protocol`, `tauri-adapter`, `apps/desktop` | Fake transport/event sink with gaps, stale revisions and unknown outcomes | Retry reconnect or refresh projection |
| P1 | Device → owner → Job budgets enforce hard admission and soft backpressure. | `plugin-host`, `job-system` | Budget ledger, isolated runtime and disk-pressure fakes | Cancel, retry later, evict derived cache or discard Job |
| P1 | Suspend checkpoints/pause work; wake resumes local Jobs and waits for network policy. | `job-system`, `tauri-adapter` | Lifecycle, monotonic-clock and network-policy signal fakes | Resume, retry, cancel or wait for connectivity |
| P1 | Tier A mismatch prevents `ready` and reaches host-native/`RecoverySurface` update action; Tier B mismatch degrades only its feature. | `tauri-adapter`, `apps/desktop` | OS/architecture/WebView/GPU/file-IO/IME capability fixtures | Retry/update runtime or use fallback |
| P1 | Profile lock admits one writer and forwards validated second-instance activation. | `application-core`, `tauri-adapter` | Lock/instance adapter and activation harness | Focus existing instance, retry verified stale lock or export diagnostics |
| P1 | Path/symlink/deep-link/drag-drop validation rejects escapes; disconnected volumes preserve Asset identity. | `application-core`, `tauri-adapter` | Canonicalization, volume identity and `ActivationRequest` fixtures | Correct input, reconnect volume or retry source operation |
| P1 | Invalid release package or interrupted update preserves committed package/data generation. | `application-core`, `tauri-adapter` | Package verifier/updater fake, temp journal and migration fault injection | Retry/offline install, recover previous generation or export diagnostics |
| P1 | Uninstall removes binaries but retains user data until explicit deletion. | `application-core`, `tauri-adapter` | Retention fixture with external-source sentinel | Reinstall/recover or explicitly delete selected data |
| P1 | Crash evidence, when the platform provides it, is local, bounded and redacted; lack of native capture still leaves journal/exit diagnostics. | `application-core`, `tauri-adapter` | CrashCaptureAdapter fake with OOM/signal/exception fixtures | Export diagnostics, retry or continue without upload |
| P1 | Cold start, idle memory/disk/CPU and hidden WebView lifecycle stay within the versioned platform baseline; suspended surfaces do not run undeclared work. | `application-core`, `tauri-adapter`, `apps/desktop` | Monotonic-clock startup harness, resource probes and WebView lifecycle fake | Close/suspend a surface, retry, update runtime or continue degraded |
| P1 | Stable/preview extension packages verify digest, publisher identity, target, capabilities, freshness, revocation and anti-downgrade before activation; unsigned local packages require visible developer mode. | `plugin-api`, `plugin-host`, `application-core`, `tauri-adapter` | Signed-package verifier fake, key-rotation/revocation/offline fixtures and developer-mode policy test | Keep the current package, inspect trust issues, install a valid offline package or disable the extension |
| P1 | Staged extension upgrade keeps old package/data generations until the new runtime reaches a health checkpoint; failed activation or migration restores the previous pair without touching core data. | `plugin-host`, `persistence`, `application-core` | Upgrade transaction, crash/power-loss and schema-range fixtures | Resume, commit, rollback or discard the staged extension transaction |
| P1 (marketplace) | A paid extension receives only host-verified entitlement status with bounded offline grace; package signature and publisher trust remain separate, and no payment credential crosses the plugin boundary. | `plugin-host`, `workbench-protocol`, `tauri-adapter` | Fake entitlement issuer with expiry, revocation, clock-skew, sign-out and offline fixtures | Continue in grace, renew/sign in, disable paid feature or keep data and uninstall |

The first lifecycle implementation slice has these acceptance results:

| Slice | Result | Evidence |
| --- | --- | --- |
| Journal round-trip and unknown-schema rejection | Pass | `JsonFileRecoveryJournalStore` persistence test |
| Two unfinished pre-ready launches enter Safe Mode with recovery actions | Pass | `RecoverySupervisor` deterministic-clock test |
| Core layout migration failure enters Safe Mode and preserves the failed state | Pass | `ApplicationCore` migration fixture |
| Optional plugin activation failure degrades without entering core Safe Mode | Pass | `ApplicationCore` plugin activation fixture |
| Activated plugin runtime failure emits plugin-disabled and recovery-degraded evidence while leaving core data available | Pass | `tauri-adapter` runtime failure fixture |
| Rust recovery query, Tauri response/event and TypeScript types agree | Pass | `workbench-protocol`, `tauri-adapter`, desktop Tauri compile and TypeScript typecheck |
| Desktop `RecoveryClient` loads the projection and accepts recovery events without reading native storage | Pass | `apps/desktop/test/recovery-client.test.mjs` |
| Normal Tauri shutdown records intent and completion; an unfinished launch remains classifiable after forced termination | Pass | `RecoverySupervisor` shutdown receipt test and Tauri run-event wiring |
| Core deadline breach records `startup-timeout` and enters Safe Mode; optional activation deadline remains degraded and stops scheduling more components | Pass | Injected `StartupDeadlinePolicy`/`RecoveryClock` tests and activation-loop boundary |
| Host-native recovery UI, generation snapshots and user restore actions | Pending | Next P0 slices |

All P0 rows must pass before the native foundation is considered safe for alpha
users. P1 rows must pass for each supported platform and package/channel before
that combination is production-ready; Tier B failures remain diagnosable
degradation. The optional out-of-process `RecoverySupervisor` is measured
separately and is not a release prerequisite. Crash evidence is tested through
the separate `CrashCaptureAdapter` seam and is not used to justify a mandatory
helper process.

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
- A new transport performs a typed handshake, selects a compatible protocol major/minor, negotiates payload schemas/features and assigns a fresh `sessionId` before accepting mutations.
- A protocol minor addition with optional fields remains usable; a major mismatch, unknown required field or incompatible schema returns a typed compatibility error and leaves the Workbench in a diagnosable degraded/recovery state.
- A duplicate request with the same `requestId` returns the bounded original result for an idempotent command; an unknown mutation outcome queries authoritative state before retry and never blindly replays an external side effect.
- Duplicate or stale events are ignored, a sequence gap triggers bounded scope snapshot resync, and a stale revisioned write returns the authoritative snapshot with a typed error.
- After transport loss the Workbench retains the last valid projection read-only, enters bounded degraded reconnect, creates a fresh session, resynchronizes complete authoritative snapshots and discards old-session events/handles.
- Plugin runtime and UI Bridge reconnects recreate projections from Rust-owned state without replaying side-effecting commands; a failed optional feature remains isolated from the core shell.
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
- A Profile lock admits one writer; multiple windows share the same Core, and a second process forwards a validated file/deep-link/command-line activation request before exiting. A stale lock requires process start-time/instance-nonce verification and never triggers concurrent writes.
- Window restoration clamps missing-monitor or changed-work-area bounds to a visible display and records a repair diagnostic; high-DPI, IME, RTL, reduced-motion and accessibility checks pass through host adapters.
- A source on a removable or network volume remains registered when disconnected; Preview/Job operations return typed `source-unavailable`, and reconnection revalidates volume identity and path before resuming work.
- File-watcher events are hints; the Asset/source owner revalidates `ContentFingerprint`, invalidates derived results through `JobSystem`, and scan-on-use or explicit rescan works when watcher events are unavailable.
- Symlink loops/escapes, archive symlink entries, malformed file associations, drag/drop traversal and unsupported deep-link schemes are rejected by Rust with bounded diagnostics; the renderer never executes raw paths or commands.
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
