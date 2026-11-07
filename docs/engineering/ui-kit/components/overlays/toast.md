# VelvetToastHost

`VelvetToastHost` is the Velvet overlays component for live notification projection.
It is implemented in `apps/desktop/src/app/ui/overlays/`; its public import
remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `items`, `onDismiss`
- **States:** default, error
- **Contract:** `apps/desktop/src/app/ui/contracts/overlays/toast.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/overlays/toast.stories.tsx`
- **Qualification:** Node/Happy DOM and real browser live-region/dismissal fixture
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `toast/default`
- `toast/error`


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-primitives
```
