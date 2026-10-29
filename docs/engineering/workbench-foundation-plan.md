# Workbench foundation plan

Status: In progress; topology and desktop renderer decisions accepted, first React shell slice implemented; business-independent Workbench foundation is the active first-stage scope
Date: 2026-10-28

This plan is intentionally a skeleton. It establishes seams and ownership without pretending that the unresolved domain decisions are already accepted. The broad architecture decisions are tracked under `.scratch/workbench-foundation/`; the active first-stage shell decisions are tracked under `.scratch/workbench-shell-foundation/`.

## Active first-stage scope

The immediate destination is the business-independent Workbench foundation, tracked in `.scratch/workbench-shell-foundation/`. It covers fixed Part docking and semantic Zones, VS Code-like visibility, collapse, resize and within-Zone ordering controls, Rust-owned layout persistence and migrations, host commands and overlays, selectable `WorkbenchApp` surfaces, shared `WorkspaceContext` projections, scoped plugin namespaces, reserved global companion/operations surfaces and the framework-neutral ViewContainer/View contribution seam. Built-in apps remain stable Activity Bar entries while accepted extensions may contribute app entries under the same host rules; product-domain behavior, preview providers, jobs, indexing and marketplace policy remain later decisions.

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
root inside Tauri and keeps the browser DemoApp as a local fallback. The next
implementation gate is broader accessibility, keyboard and resize coverage
around contributed panels. Plugin view contributions now map to the
framework-neutral `ViewRegistry` with stable extension-prefixed ids. A
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
The Activity Bar now projects contributed navigation ViewContainers for the
active WorkbenchApp. Selecting one reveals its persisted container and the
Primary Sidebar, moves focus to that Part and uses roving keyboard navigation;
selection itself remains transient so restart recovery stays owned by the Rust
layout snapshot.
Context-menu and dialog contributions now use the host `MenuRegistry` and
`OverlayRegistry`; dialog surfaces are mounted through the isolated UI Bridge
only while an overlay is open, command handlers bind through the plugin
runtime seam, and namespace/lifecycle cleanup is tested.
Plugin-owned `WorkbenchApp` manifests now register extension-prefixed app
descriptors in `WorkbenchAppRegistry`. Their Activity Bar entries are projected
through the same registry as built-in apps, while the selected AppSurface is
created as an isolated UI Bridge session, suspended on app switch and disposed
before deactivation removes the app namespace. Namespace ownership and full
app cleanup are covered by the plugin lifecycle tests.

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
        editors/
          editor-group.ts
          editor-input.ts
          editor-registry.ts
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

## Implementation sequence

1. **Documentation and vocabulary**: complete `docs/GLOSSARY.md`, accept the topology ADR, and add the decision link to the local Wayfinder map. **Done for the current skeleton.**
2. **Protocol types**: implement `workbench-protocol`, `layout-model`, `asset-core` and error envelopes without a UI. **Done for the current layout slice; native wire fixtures cover the Tauri mapping.**
3. **Layout service**: implement versioned layout state, migrations, fixed zones and persistence tests. **Done for the current file-backed Tauri store and restart recovery slice.**
4. **Workbench parts**: render ActivityBar, Resource Manager, Feed Workbench, PrimarySidebar, WorkspaceArea, AuxiliarySidebar, OperationsPanel and StatusBar with placeholder views. **Initial renderer slice done.**
5. **View registry**: add built-in views and one example external view per zone. Verify that cross-zone registration is rejected. **Host registry and rejection tests done; the React shell mounts registered ViewContainers/Views with a renderer adapter seam, PluginClient projects extension views plus isolated UI Bridge sessions into that seam, and the Activity Bar now selects contributed navigation containers.**
6. **Preview core**: implement generic image/file fallback, `PreviewResolver`, Quick/Inspect/Analyze intents and a fake provider used by contract tests.
7. **Plugin host**: load manifests, validate permissions, activate lazily, isolate failures and expose the UI SDK boundary.
8. **Jobs and facets**: implement one thumbnail job, one metadata facet and one cancellable analysis job.
9. **Tauri adapter**: connect typed commands, queries and events to the desktop client. No feature should bypass the protocol. **Done for the current layout slice, including native command/event serialization and global shortcut entry.**

## Acceptance scenarios

- The built-in `WorkbenchApp` entries are always discoverable and do not move.
- A trusted or isolated extension can contribute an ActivityBar `WorkbenchApp`, a central `AppSurface`, a ViewContainer-only tool, or the compatible combination without owning global layout.
- An add-on registered in one WorkbenchApp namespace can contribute an app-local dialog or context-menu action without appearing in another app.
- A host-approved CompanionView can follow shared context in the AuxiliarySidebar, while global jobs remain visible in the OperationsPanel.
- A navigation plugin, contextual plugin and operations plugin can only register in their declared zone.
- A plugin can be disabled while the workbench remains usable.
- A RAW-like fake provider supports all three preview intents through one resolver.
- A metadata-only plugin augments the inspector without replacing the primary preview.
- Layout changes survive restart and pass a schema migration test.
- A long-running job reports progress, supports cancellation and leaves a diagnosable result.
- Replacing the frontend adapter does not change the Rust protocol or plugin manifest.

## Structure review checkpoints

Before adding a new directory, ask:

1. Does it own a deep interface or is it only a pass-through?
2. Does the new responsibility belong to a `Part`, a workspace, a zone or a domain module?
3. Can a fake adapter test the behaviour through the interface?
4. What happens when the owning plugin is disabled or upgraded?
5. Does the module introduce a second persistence owner for state already owned elsewhere?
