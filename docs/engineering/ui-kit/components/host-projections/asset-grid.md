# VelvetAssetGrid

`VelvetAssetGrid` is the Velvet host projections component for keyboard-navigable asset projection layouts.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `items`, `columns`, `layout`, `onSelect`, `onOpen`, `onDragStart`, `onDrop`
- **States:** default, selected, disabled, empty, layout, dragging, pinned
- **Actions:** select, open, navigate, drag, drop
- **Observables:** grid-role, selection, focus, open-callback, layout, drag-and-drop, pinned
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/asset-grid.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/asset-grid.stories.tsx`
- **Qualification:** both Node/Happy DOM and real browser
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Layout and drag/drop

`layout` accepts `grid` (default), `square`, `list` or `justified`. An item may
provide `aspectRatio` and `pinned`; those values are rendered as host-owned
projection state. Drag/drop is opt-in: when `onDragStart` and `onDrop` are
provided, the component exposes native draggable cells and reports typed source
and target ids. It does not persist ordering or read transport state.

The layout and drag/drop surface is derived from the bounded Eagle directive
analysis in [the versioned evidence matrix](../../../ui-kit-eagle-directive-evidence.md).
Eagle's lazy loading and InfiniteGrid paging remain host runtime concerns and
are intentionally outside this component's public API.

## Stories and evidence

- `asset-grid/default`
- `asset-grid/selected`
- `asset-grid/layout-modes`


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unknown Eagle behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
