# ADR-0003: Workbench parts, fixed zones and layout state

Status: Proposed
Date: 2026-10-28

## Context

The workbench must feel like an IDE without forcing users to learn arbitrary plugin locations. The user has explicitly chosen three stable extension zones with different jobs:

1. navigation and source context on the left;
2. contextual inspection and third-party tools on the right;
3. jobs, logs, terminal and operational tools at the bottom.

The first ActivityBar workspaces are `AssetManager` and `FeedWorkbench`. They are product-level workspaces, not arbitrary plugin tabs.

## Proposed decision

Model the workbench using stable `Part` objects and fixed semantic zones:

```text
Workbench
├── TitleBar
├── ActivityBar
├── PrimarySidebar       navigation zone
├── WorkspaceArea        central content host
├── ContextSidebar       contextual zone
├── OperationsPanel      operations zone
├── StatusBar
└── OverlayLayer
```

The implementation may map these concepts to VS Code terminology (`Sidebar`, `AuxiliaryBar`, `Panel`), but product-facing names explain the job of each region.

Plugins contribute `ViewContainer` and `ViewDescriptor` objects with a declared zone:

```text
navigation  → ActivityBar / PrimarySidebar
contextual  → ContextSidebar
operations  → OperationsPanel
```

In version one, a plugin cannot move a contribution across semantic zones. Users may hide, reorder, resize and collapse views inside their allowed zone. The ActivityBar workspaces remain fixed and plugins do not create top-level workspaces by default.

Persist `LayoutState` separately at global-profile and workspace scope. The state is versioned and migrated by one `LayoutService`; individual parts do not persist their own geometry.

## Alternatives considered

1. Allow every plugin to choose any left, right or bottom location. Rejected because the meaning of each region would drift and plugin discovery would become unpredictable.
2. Use a single `shell` directory and let components own layout state. Rejected because it hides spatial roles and scatters persistence.
3. Make all product workspaces plugin-contributed. Rejected for the first version because AssetManager and FeedWorkbench are the product's stable navigation model.

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
  "activeWorkspace": "asset-manager",
  "activityBar": { "visible": true },
  "primarySidebar": { "visible": true, "width": 280 },
  "workspaceArea": { "groups": [] },
  "contextSidebar": { "visible": true, "width": 340 },
  "operationsPanel": { "visible": false, "height": 240 },
  "statusBar": { "visible": true },
  "views": {}
}
```

## References

- [VS Code layout parts](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/layout/browser/layoutService.ts)
- [VS Code view containers and view descriptors](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/common/views.ts)
- [VS Code view contribution points](https://code.visualstudio.com/api/references/contribution-points#contributes.viewsContainers)
- [JetBrains tool windows](https://plugins.jetbrains.com/docs/intellij/tool-windows.html)
