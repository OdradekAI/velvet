# VelvetCommandPalette

`VelvetCommandPalette` is the Velvet host projections component for host-owned command search overlay.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `open`, `commands`, `scope`, `scopeOptions`, `onScopeChange`, `query`, `onQueryChange`, `onSelect`, `onClose`, `restoreFocusRef`
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
- `command-palette/controlled-query`
- `command-palette/host-results`

The story runner covers keyboard navigation, selection, Escape dismissal, the
controlled query callback and host-provided result replacement. When `query` is
omitted, the component keeps a local draft for the input. When `query` is
supplied, the host owns that value and must commit changes received through
`onQueryChange`; the component does not maintain a second source of truth.

`commands` is already the result projection for the active scope. The component
does not parse query syntax, filter commands, rank results or access an index.
Those responsibilities belong to a Workbench search/command adapter. The
browser qualification verifies that the host can replace the result list while
the overlay keeps its keyboard, ARIA and focus behavior.


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
The `command-palette/open` scenario covers keyboard navigation, selection and
Escape dismissal. `scopeOptions` and `onScopeChange` let the host project
current/all result scope without moving query ownership into the component. The
input exposes an active descendant pointing at the selected option. Home and
End remain part of the keyboard contract over the host-supplied list. The
visual contract uses an overlay surface, compact rows and restored trigger
focus; command execution remains a host lifecycle rather than a renderer side
effect.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
