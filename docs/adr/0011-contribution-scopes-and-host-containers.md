# ADR-0011: Contribution scopes and host containers

Status: Accepted
Date: 2026-10-29

## Context

Velvet needs more than panel contributions. A plugin may add a full
WorkbenchApp, a one-shot dialog, a right-click action, a background monitor, a
format/preview provider or an inspector. Eagle demonstrates the usefulness of
separating window plugins, background services, format extensions and
inspector extensions, and its API exposes host-owned context menus and plugin
lifecycle callbacks. Velvet needs the same breadth without exposing raw DOM,
unrestricted filesystem access or framework internals.

The Feed Workbench also introduces a cross-application flow. A monitor or
subscription collects source material in the Feed app; a user may explicitly
materialize selected content into the Resource Manager, where the same or
different extensions can preview and process it.

## Decision

Every contribution declares an explicit scope:

```text
workbench       global shell, overlay and cross-app capabilities
workbench-app   one WorkbenchApp surface and its local containers
capability      reusable data, preview, source, processing or job capability
```

When a package extends an existing `WorkbenchApp`, its contribution is
resolved in that app's namespace and is not itself an additional app. The
namespace and target app rules are defined in ADR-0012.

The host owns these containers:

```text
ActivityBar, TitleBar, WorkspaceArea,
PrimarySidebar, AuxiliarySidebar, OperationsPanel,
StatusBar, OverlayLayer, ContextMenu and CommandPalette
```

Contribution kinds are grouped by scope:

```text
Workbench:
  ActivityBarEntry, TitleBarAction, Command, Keybinding,
  ContextMenuContribution, DialogContribution, Notification,
  SettingsContribution

WorkbenchApp:
  AppSurface, ViewContainer, View, AppToolbar,
  AppContextMenuContribution, InspectorSection

Capability:
  SourceConnector, FeedMonitor, ContentKindProvider,
  PreviewProvider, PreviewAugmentation, Processor,
  Job, AssetFacet and ExtensionModel
```

A package may declare contributions in more than one scope, but every
contribution is validated independently for its target app, host container,
permissions and lifecycle. A global contribution cannot infer broad data or
network access from being visible in the shell.

The desktop host represents these declarations as framework-neutral descriptors
with a namespace, scope, optional app target, host container, semantic zone and
context anchor. `ContributionRegistry` rejects mismatched scope/kind/container
combinations before activation. WorkbenchApp commands and dialogs remain
app-scoped; `CompanionView` and `OperationView` are reserved for the global
AuxiliarySidebar and OperationsPanel respectively.

View contributions carry display metadata and, for isolated surfaces, an
entrypoint plus UI bridge version. `PluginClient` maps an app-scoped
`ViewContainer` and its `View` children into the host `ViewRegistry`; global
CompanionView and OperationView contributions get one synthetic container each
in their reserved Part. The derived descriptor ids are prefixed with the
extension id, while `viewContainerId` remains a local manifest reference. This
keeps persisted layout keys stable without allowing one extension to target
another extension's container.

`context-menu` and `app-context-menu` contributions require a command from the
same extension and are registered in `MenuRegistry` under the derived
namespace. The isolated runtime binds the extension's command handler after
activation; until then the menu remains registered but invocation fails with a
typed lifecycle error. `dialog` contributions are registered in `OverlayRegistry` with
host-owned modal, escape and focus-restoration behavior. When a dialog declares
an isolated UI surface, the corresponding `PluginSurfaceSession` is mounted
only for an open overlay. Deactivation closes the overlay and disposes its
session before removing the menu, command and contribution descriptors.

Context menus are host-owned composition points. A contribution declares its
target context (`asset`, `feed-item`, `collection`, `selection`, `view` or
`background`), label, ordering group, enablement predicate and command. The
host supplies the current typed context and executes the command through the
capability seam. Plugins do not replace the native menu or mutate the DOM.

Dialogs and popovers use the OverlayLayer. A plugin receives a typed invocation
and a host-managed lifecycle/focus handle; trusted Velvet UI or an isolated UI
surface renders the body. The host owns placement, accessibility, escape,
failure, timeout and cleanup behavior.

Feed monitors and subscriptions are capability contributions. They write
FeedItem/Capture records through the core and expose progress, retry and
provenance. The explicit `materialize` operation creates or links an Asset for
the Resource Manager and preserves the originating FeedItem, source and
capture provenance. It does not copy the same domain record into two app-owned
databases.

## Alternatives considered

1. Treat every plugin as a WorkbenchApp. Rejected because format providers,
   context-menu actions and background monitors do not need a central surface.
2. Give plugins direct access to browser menus and dialogs. Rejected because
   focus, accessibility, theme, permissions and failure handling would diverge.
3. Convert every FeedItem into an Asset immediately. Rejected because reading
   material and user-owned resources have different lifecycle and provenance.

## Consequences

Positive:

- Eagle-like window, context-menu, inspector and background plugin patterns fit
  one explicit contribution model.
- WorkbenchApp surfaces, global shell tools and reusable data capabilities can
  evolve independently.
- Feed collection and Resource Manager processing can share identity and
  provenance without duplicate persistence owners.

Costs:

- The manifest and host registries need contribution scope and target schemas.
- Context predicates and typed context snapshots need compatibility rules.
- Materialization requires a later Asset/Feed data-model decision.

## Follow-up decisions

- The first host contract includes declarative registration for Workbench,
  WorkbenchApp and capability kinds listed above. Availability to third-party
  extensions remains an activation-policy decision in the plugin host.
- Context anchors are stable names (`asset`, `feed-item`, `collection`,
  `selection`, `view` and `background`); the isolated UI bridge receives only
  the fields granted by the host.
- Dialog bodies use the trusted Velvet UI adapter or the isolated UI bridge;
  the host owns focus, placement, escape, timeout and cleanup.
- The Asset/Feed data model and provenance representation remain a later
  domain decision.
