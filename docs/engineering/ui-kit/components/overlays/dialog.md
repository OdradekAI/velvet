# VelvetDialog

`VelvetDialog` is the Velvet overlays component for host-owned modal overlay.
It is implemented in `apps/desktop/src/app/ui/overlays/`; its public import
remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `open`, `title`, `description`, `actions`, `onOpenChange`, `restoreFocusRef`
- **States:** closed, open
- **Contract:** `apps/desktop/src/app/ui/contracts/overlays/dialog.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/overlays/dialog.stories.tsx`
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `dialog/closed`
- `dialog/open`


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unknown Eagle behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```
