# VelvetAssetGrid

`VelvetAssetGrid` is the Velvet host projections component for keyboard-navigable asset projection layouts.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `items`, `columns`, `layout`, `onSelect`, `onOpen`, `onDragStart`, `onDrop`
- **States:** default, selected, disabled, empty, layout, dragging, pinned
- **Actions:** select, open, navigate, drag, drop
- **Observables:** grid-role, grid-metrics, selection, selection-modifier, focus, open-callback, layout, drag-and-drop, pinned, metadata
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/asset-grid.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/asset-grid.stories.tsx`
- **Qualification:** both Node/Happy DOM and real browser
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Layout and drag/drop

`layout` accepts `grid` (default), `square`, `list`, `justified` or `waterfall`.
An item may
provide `aspectRatio`, `metadata` and `pinned`; those values are rendered as
host-owned projection state. `metadata` is a React slot so the host can project
tags, resolution, rating or extension details without making those fields part
of the grid's selection state. Pinned state has an accessible marker but no
implicit persistence action. Drag/drop is opt-in: `onDragStart` enables native draggable cells and reports the source
id; `onDrop` independently enables drop handling and reports typed source and
target ids. It does not persist ordering or read transport state.

`onSelect(id, additive, range)` reports the pointer modifier that produced the
selection. The populated-library probe showed Shift-click selecting a
second item while keeping both cells selected. The host owns the selection
anchor and decides how to apply the `range` hint; the grid never stores a
second selection model.

The layout and drag/drop surface is derived from the bounded interaction evidence
recorded in the local research matrix.
Lazy loading and paging remain host runtime concerns and
are intentionally outside this component's public API.

## Stories and evidence

- `asset-grid/default`
- `asset-grid/selected`
- `asset-grid/grid-semantics`
- `asset-grid/layout-modes`
- `asset-grid/selection-modifiers`
- `asset-grid/zoom-layout`
- `asset-grid/waterfall-layout`

`asset-grid/zoom-layout` is an integration story: `VelvetZoomControl` stays
separate, and the host maps its value to `columns`. It records the observed
geometry checkpoints of eight columns at `180` and five at `230` without making
that mapping a public AssetGrid algorithm.

The grid exposes `aria-multiselectable="true"` because selection modifiers are
reported to the host. For `grid`, `square` and `list` layouts it also exposes
`aria-rowcount`, `aria-colcount`, `aria-rowindex` and `aria-colindex` using the
host-projected column count (with one column in `list` mode). `justified` and `waterfall` keep these metrics
unset because their responsive geometry is not a stable public row/column
contract. Waterfall preserves each item's aspect ratio and leaves column
measurement to the host layout surface.

The semantic visual contract uses a compact gallery profile, a `4/3` thumbnail
invariant for the default card and an active-surface selection state. The
contract deliberately names those invariants without exposing CSS selectors.


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
