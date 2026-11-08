# VelvetCommandPalette

`VelvetCommandPalette` is the Velvet host projections component for host-owned command search overlay.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `open`, `commands`, `onSelect`, `onClose`, `restoreFocusRef`
- **States:** open, empty
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/command-palette.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/command-palette.stories.tsx`
- **Qualification:** both Node/Happy DOM and real browser
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `command-palette/open`
- `command-palette/empty`

The story runner covers keyboard navigation, selection and Escape dismissal.
The real-browser qualification additionally verifies query filtering with the
host command data.


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
The `command-palette/open` scenario covers query filtering, keyboard navigation,
selection and Escape dismissal. Home and End remain part of the keyboard
contract even when a host supplies a filtered command list. The visual contract
uses an overlay surface, compact rows and restored trigger focus; command
execution remains a host lifecycle rather than a renderer side effect.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
