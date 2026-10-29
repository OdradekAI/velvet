# ADR-0003: Workbench parts, fixed zones and layout state

Status: Accepted
Date: 2026-10-29

## Context

The workbench must feel like an IDE without forcing users to learn arbitrary plugin locations. The user has explicitly chosen three stable extension zones with different jobs:

1. navigation and source context on the left;
2. contextual inspection and third-party tools on the right;
3. jobs, logs, terminal and operational tools at the bottom.

The first built-in `WorkbenchApp` entries are `asset-manager` (Resource
Manager) and `feed-workbench` (Feed Workbench). They are selectable product
surfaces, not editor tabs. A product extension may contribute another
`WorkbenchApp` entry after the app contribution contract is accepted.

## Decision

Model the workbench using stable `Part` objects and fixed semantic zones:

```text
Workbench
├── TitleBar
├── ActivityBar
│   └── WorkbenchApp entries or ViewContainer entries
├── PrimarySidebar       navigation zone
├── WorkspaceArea        active WorkbenchApp AppSurface
├── AuxiliarySidebar     contextual/auxiliary zone
├── OperationsPanel      operations zone
├── StatusBar
└── OverlayLayer
```

The implementation may map these concepts to VS Code terminology (`Sidebar`, `Secondary Side Bar`/`Auxiliary Bar`, `Panel`), but product-facing names explain the job of each region. The `AuxiliarySidebar` is reserved for host-managed contextual tools and global `CompanionView` contributions; the `OperationsPanel` is reserved for global jobs and operation state. App-specific tools use app-scoped containers unless the host explicitly grants a global contribution.

`WorkbenchApp` entries and plugins contribute `ViewContainer` and `ViewDescriptor` objects with a declared scope and zone. A descriptor carries a stable id, title/title key, icon, preferred Part, order, default visibility, enablement expression and required capabilities. A ViewContainer may expose an Activity Bar entry only when it is in the navigation zone and targets the PrimarySidebar:

```text
navigation  → ActivityBar / PrimarySidebar
contextual  → AuxiliarySidebar
operations  → OperationsPanel
```

The mapping above applies to global Workbench contributions. An app-scoped
navigation container targets the PrimarySidebar; app-scoped contextual and
operations containers render inside the active AppSurface in WorkspaceArea.
AuxiliarySidebar and OperationsPanel remain global CompanionView and
OperationView surfaces. A View must stay in the same scope and semantic Zone as
its container.

The renderer projects Activity Bar entries from the active app's registered
navigation containers. Selecting one makes that container the visible primary
sidebar selection, reveals the container and sidebar when either is hidden or
collapsed, and moves focus to the sidebar. Selection is transient; container
visibility and order remain the durable layout state. Activity Bar buttons use
roving keyboard navigation with Arrow, Home and End keys, while the host keeps
the built-in WorkbenchApp entries ahead of contributed container entries.

In version one, Part docking positions are fixed. A plugin cannot move a contribution across semantic zones. Users may hide, show, collapse and focus ViewContainers, reorder them within a compatible Part, and reorder or move Views between compatible containers in the same scope and Zone. Hidden contributions retain their order, and the host repairs order when a contribution is disabled or removed. Built-in ActivityBar entries remain stable, while accepted product extensions may contribute `WorkbenchApp` entries without gaining ownership of the global shell.

Ordering is represented as host-owned state and changed through typed commands. Commands should identify the contribution and a stable `before` or `after` anchor; the renderer's drag-and-drop interaction is only an adapter over those commands.

Part geometry follows a small, explicit constraint table so a saved layout
cannot collapse the workbench into an unusable state:

| Part | Visibility | Collapse | Resize | Size rule |
| --- | --- | --- | --- | --- |
| TitleBar | user-toggleable | fixed | fixed | 36 px |
| ActivityBar | user-toggleable | fixed | fixed | 48 px |
| PrimarySidebar | user-toggleable | user-toggleable | 180–520 px | default 260 px |
| WorkspaceArea | always visible | fixed | fills remaining space | no persisted size |
| AuxiliarySidebar | user-toggleable | user-toggleable | 220–520 px | default 300 px |
| OperationsPanel | user-toggleable | user-toggleable | 120–520 px | default 180 px |
| StatusBar | user-toggleable | fixed | fixed | 24 px |
| OverlayLayer | host-owned | fixed | fixed | no persisted size |

The Rust `LayoutService` validates these constraints before changing state.
The desktop renderer consumes the resulting snapshot and sends typed commands;
it does not enforce a second persistence rule. A stale or invalid command
leaves the previous snapshot usable and returns a typed layout error.

The first command surface names these operations explicitly:
`workbench.toggle-part`, `workbench.focus-part`, `workbench.resize-part`,
`workbench.collapse-part`, `workbench.maximize-part`, `workbench.move-view`,
`workbench.upsert-view-container`, `workbench.upsert-view`,
`workbench.set-view-container-visibility`,
`workbench.reorder-view-container`, `workbench.remove-view-container`,
`workbench.reset-layout`,
`workbench.reset-app-layout` and `workbench.reset-view-layout`.

Persist layout state through one Rust-owned `LayoutService` in three scopes:

- Profile stores Workbench Part visibility, size, collapse state and order,
  global CompanionView/OperationView state, and title-bar tool-group state.
- Workspace stores the active `WorkbenchApp`, app-scoped ViewContainer/View
  state, and durable AppSurface layout such as tabs, filters and saved views.
- Session stores focus, selection and other window restoration hints. Overlay,
  drag and in-progress resize state remains transient.

The state is versioned and migrated by the same `LayoutService`; individual
parts and apps do not persist their own geometry. The renderer may optimistically
project interaction state, but Rust returns the authoritative snapshot with a
monotonic revision event. Plugin defaults are merged only when no user override
exists, and reset commands are available for the Workbench, active
WorkbenchApp and all View contributions.

The native Tauri composition injects a JSON `LayoutStateStore` rooted at the
platform application data directory. Its current file name is
`workbench-layout.json`; the persisted document uses the layout schema version
and is written through a temporary file before replacement. The storage
adapter owns filesystem access, while `LayoutService` remains responsible for
validation, migration and recovery of an invalid or missing document.

## Alternatives considered

1. Allow every plugin to choose any left, right or bottom location. Rejected because the meaning of each region would drift and plugin discovery would become unpredictable.
2. Use a single `shell` directory and let components own layout state. Rejected because it hides spatial roles and scatters persistence.
3. Treat every ActivityBar entry as an editor container. Rejected because Velvet's central area hosts business-oriented `WorkbenchApp` surfaces rather than a single editor-tab abstraction.

## Consequences

Positive:

- Users can understand a zone by its function without learning plugin-specific placement rules.
- Permissions and APIs can be constrained by zone.
- Layout persistence and migrations have one owner.

Costs:

- A plugin cannot place arbitrary UI anywhere in version one.
- Some advanced workflows may need a deliberate new zone or a reviewed workspace contribution.

## Initial layout state shape

```json
{
  "schemaVersion": 1,
  "revision": 0,
  "activeApp": "asset-manager",
  "titleBar": { "visible": true, "size": 36, "collapsed": false },
  "activityBar": { "visible": true, "size": 48, "collapsed": false },
  "primarySidebar": { "visible": true, "size": 260, "collapsed": false },
  "workspaceArea": { "groups": [] },
  "auxiliarySidebar": { "visible": true, "size": 300, "collapsed": false },
  "operationsPanel": { "visible": false, "size": 180, "collapsed": false },
  "statusBar": { "visible": true, "size": 24, "collapsed": false },
  "containers": {
    "library": { "visible": true, "order": 0 }
  },
  "views": {
    "asset-library": {
      "containerId": "library",
      "zone": "navigation",
      "visible": true,
      "order": 0
    }
  }
}
```

## References

- [VS Code layout parts](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/layout/browser/layoutService.ts)
- [VS Code view containers and view descriptors](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/common/views.ts)
- [VS Code view contribution points](https://code.visualstudio.com/api/references/contribution-points#contributes.viewsContainers)
- [JetBrains tool windows](https://plugins.jetbrains.com/docs/intellij/tool-windows.html)
