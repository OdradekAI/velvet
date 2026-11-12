# VelvetDataTable

`VelvetDataTable` is the Velvet host projections component for sortable tabular projection.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `rows`, `columns`, `rowId`, `selectedId`, `sort`, `onSelect`, `onSort`, `emptyMessage`
- **States:** default, selected, sortable, empty
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/data-table.contract.ts`
- **Stories:** `default`, `selected`, `sort-direction`, `empty`
- **Qualification:** both Node/Happy DOM and real browser
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Sort state belongs to the host: `onSort`
receives the column id and the next `ascending`/`descending` direction, while
the controlled `sort` prop projects the committed state through `aria-sort`.
Empty rows render a labelled status cell and do not create a second selection
owner. Radix types, private DOM structure, Tailwind classes and Workbench
implementation details are not public API.

## Stories and evidence

- `data-table/default`
- `data-table/selected`
- `data-table/sort-direction`
- `data-table/empty`


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
