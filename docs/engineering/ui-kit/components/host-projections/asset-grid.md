# VelvetAssetGrid

`VelvetAssetGrid` is the Velvet host projections component for keyboard-navigable asset projection grid.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `items`, `columns`, `onSelect`, `onOpen`
- **States:** default, selected
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/asset-grid.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/asset-grid.stories.tsx`
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `asset-grid/default`
- `asset-grid/selected`


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unknown Eagle behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
