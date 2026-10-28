# Workbench foundation plan

Status: In progress; topology accepted, skeleton implementation started
Date: 2026-10-28

This plan is intentionally a skeleton. It establishes seams and ownership without pretending that the unresolved domain decisions are already accepted. The open decisions are tracked in the local Wayfinder map under `.scratch/workbench-foundation/`.

## Target shape

```text
apps/
  desktop/
    src/
      app/
        desktop-app.ts
        index.ts
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
          context-sidebar/
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

`app/` is the composition root. `platform/` owns the transport-facing types and adapters without importing Vue or React. `workbench/` contains spatial infrastructure and host registries. Product workspaces live under the sibling `workspaces/` directory; they are stable Activity Bar entries and are not plugin views. A plugin view must never be added directly to `parts/`; it is registered through `ViewRegistry`.

The frontend boundary intentionally stays inside `apps/desktop` while there is one client. Extract a named root package only after a second frontend or independently versioned SDK consumes the same capability. This keeps the vben-inspired application/package distinction without creating placeholder packages.

The `packages/` and `extensions/` entries in the target shape are deferred extraction targets. They describe the public SDK and isolated extension packaging that will be introduced after the host contracts have a second consumer; they are not required directories in the current skeleton.

The first code skeleton keeps each Rust crate's public seam in `src/lib.rs` so the dependency graph stays small while the interfaces are being validated. Split a crate into the finer-grained files shown above only when a second implementation or a meaningful internal seam appears.

Avoid generic `common/`, `shared/`, `utils/` and `services/` dumping grounds. A module earns a directory when it owns a meaningful interface, state model or adapter.

## Interface skeletons

These are shape sketches for the first implementation, not final public APIs.

### Layout model

```rust
pub enum WorkbenchWorkspaceId {
    AssetManager,
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
    ContextSidebar,
    OperationsPanel,
    StatusBar,
    OverlayLayer,
}

pub struct LayoutState {
    pub schema_version: u32,
    pub active_workspace: WorkbenchWorkspaceId,
    pub parts: PartState,
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

1. **Documentation and vocabulary**: complete `docs/GLOSSARY.md`, accept the topology ADR, and add the decision link to the local Wayfinder map.
2. **Protocol types**: implement `workbench-protocol`, `layout-model`, `asset-core` and error envelopes without a UI.
3. **Layout service**: implement versioned layout state, migrations, fixed zones and persistence tests.
4. **Workbench parts**: render ActivityBar, AssetManager, FeedWorkbench, PrimarySidebar, WorkspaceArea, ContextSidebar, OperationsPanel and StatusBar with placeholder views.
5. **View registry**: add built-in views and one example external view per zone. Verify that cross-zone registration is rejected.
6. **Preview core**: implement generic image/file fallback, `PreviewResolver`, Quick/Inspect/Analyze intents and a fake provider used by contract tests.
7. **Plugin host**: load manifests, validate permissions, activate lazily, isolate failures and expose the UI SDK boundary.
8. **Jobs and facets**: implement one thumbnail job, one metadata facet and one cancellable analysis job.
9. **Tauri adapter**: connect typed commands, queries and events to the desktop client. No feature should bypass the protocol.

## Acceptance scenarios

- The two built-in ActivityBar workspaces are always discoverable and do not move.
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
