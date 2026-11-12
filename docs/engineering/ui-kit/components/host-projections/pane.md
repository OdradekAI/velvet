# VelvetPane

`VelvetPane` is the Velvet host projections component for semantic workbench zone container.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `title`, `region`, `children`
- **States:** navigation, context, operations, preview
- **Accessibility:** named `region` using `ariaLabel` when supplied, otherwise a heading linked by `aria-labelledby`
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/pane.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/pane.stories.tsx`
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `pane/navigation`
- `pane/context`

The navigation story and browser fixture verify the region role, heading link,
zone marker and body content.


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
