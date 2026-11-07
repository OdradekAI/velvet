# VelvetActivityBar

`VelvetActivityBar` is the Velvet host projections component for workbench navigation rail.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `items`, `onSelect`
- **States:** default, badges
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/activity-bar.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/activity-bar.stories.tsx`
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `activity-bar/default`
- `activity-bar/badges`

The default scenario covers ArrowRight navigation and click selection.


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unknown Eagle behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```
