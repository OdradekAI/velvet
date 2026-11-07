# VelvetContextMenu

`VelvetContextMenu` is the Velvet overlays component for trigger-driven contextual command menu.
It is implemented in `apps/desktop/src/app/ui/overlays/`; its public import
remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `actions`, `children`
- **States:** default, checked
- **Contract:** `apps/desktop/src/app/ui/contracts/overlays/context-menu.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/overlays/context-menu.stories.tsx`
- **Qualification:** browser (Radix portal, pointer dismissal and keyboard selection)
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `context-menu/default`
- `context-menu/checked`

The real-browser fixture adds `context-menu/select`, `context-menu/cancel-escape`
and `context-menu/cancel-outside` coverage, including checked item semantics.


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-browser
```
