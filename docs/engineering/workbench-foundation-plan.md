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
falls back to the generic preview with a diagnostic. A persistent
source-location resolver remains a follow-up to the Asset model decision;
`FilePreviewAssetSource` already proves the bounded file-read adapter against
that resolver seam without adding filesystem access to the frontend or plugin
runtime.

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
6. **Preview core**: implement generic image/file fallback, `PreviewResolver`, Quick/Inspect/Analyze intents and a fake provider used by contract tests. **Host slice done:** surface-aware deterministic resolution, provider-failure fallback, typed Tauri/TypeScript projection, `PreviewHost` state machine, Quick overlay and unregisterable provider/augmentation slots. The first bounded source adapter and trusted text/image providers are implemented; persistent source-location adapters, format-specific plugin execution, augmentation renderer adapters and product-specific cache consumers remain follow-up work.
7. **Plugin host**: load manifests, validate permissions and localization bundles, activate lazily, isolate failures and expose the UI SDK boundary. **Local discovery, install preflight, permission enforcement, host API compatibility checks, numeric version upgrades, transactional commit/rollback, crash-recovery scanning, deferred activation, retryable recovery, orphan cleanup and bounded recovery logs are implemented; marketplace policy and signing remain later work. The host localization service, fallback formatter, diagnostics, loader/plugin lifecycle seam and isolated UI Bridge locale projection are implemented; archive filesystem loading and durable Profile/Session locale ownership remain adapter work.**
8. **Jobs and facets**: the shared Rust JobSystem lifecycle seam is wired to OperationsPanel with persisted progress, cancellation, retry, failure and restart recovery. It now exposes a Rust JobRunner registry, attempt-safe cooperative cancellation, panic isolation, persisted JobResult payloads, JobProvenance projection, exact provenance reuse that refuses cache hits without an input fingerprint, and an isolated plugin `invoke-job`/`job.run` adapter with typed input, result and bounded progress events. Owner cleanup revokes runners and cancels queued/running Jobs before runtime shutdown; late results are ignored, and extension-owned retries fail with a diagnostic when the owner runner is unavailable instead of using the core fallback. Product-specific thumbnail, metadata and analysis runners remain follow-up consumers.
9. **Tauri adapter**: connect typed commands, queries and events to the desktop client. No feature should bypass the protocol. **Done for the current layout slice, including native command/event serialization and global shortcut entry.**

## Acceptance scenarios

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

## Structure review checkpoints

Before adding a new directory, ask:

1. Does it own a deep interface or is it only a pass-through?
2. Does the new responsibility belong to a `Part`, a workspace, a zone or a domain module?
3. Can a fake adapter test the behaviour through the interface?
4. What happens when the owning plugin is disabled or upgraded?
5. Does the module introduce a second persistence owner for state already owned elsewhere?
