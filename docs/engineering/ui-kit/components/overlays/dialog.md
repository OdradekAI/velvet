# VelvetDialog

`VelvetDialog` is the Velvet overlays component for host-owned modal overlay.
It is implemented in `apps/desktop/src/app/ui/overlays/`; its public import
remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `open`, `onOpenChange`, `title`, `description`, `children`, `actions`, `severity`, `closeLabel`, `restoreFocusRef`
- **States:** `closed`, `open`, `warning`, `danger`, `busy`
- **Contract:** `apps/desktop/src/app/ui/contracts/overlays/dialog.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/overlays/dialog.stories.tsx`
- **Qualification:** browser (Radix portal, focus trap and restoration)
- **Host boundary:** The host owns the controlled `open` value and action lifecycle. The wrapper delegates portal and modal focus handling to Radix and uses `restoreFocusRef` when the host changes `open` from true to false.

Required props are `open: boolean`, `onOpenChange: (open: boolean) => void`, `title: string` and `children: ReactNode`. Optional props are `description?: string`, `actions?: readonly VelvetDialogAction[]` (default `[]`), `severity?: 'normal' | 'warning' | 'danger'` (default `'normal'`), `closeLabel?: string` (default `'Close'`) and `restoreFocusRef?: RefObject<HTMLElement | null>`.

Each action requires `id: string`, `label: string` and `onSelect: () => void`; it may include `variant?: 'default' | 'quiet' | 'danger'`, `primary?: boolean`, `disabled?: boolean` and `busy?: boolean`. An explicit `variant` takes precedence; otherwise primary actions use `'default'` and other actions use `'quiet'`. `disabled` or `busy` blocks only that action button. There is no dialog-wide `busy` prop, and a busy action does not prevent dismissal.

Slots are `children` and `actions`; semantic actions are `open`, `close` and `submit`. Calling an action's `onSelect` does not automatically close the dialog. The host must update `open` in response to `onOpenChange` or an action. The close button and Escape request closure; Radix supplies modal focus behavior and title/description semantics. To restore focus to a host trigger after closure, supply its `restoreFocusRef`; restoration only focuses the referenced element if it still exists.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `dialog/closed`
- `dialog/open`

The real-browser fixture adds `dialog/submit`, `dialog/cancel-button` and
`dialog/cancel-escape` coverage, including focus restoration after the portal closes.


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-browser
```

See [VelvetDialog API and Playground](/ui-kit/components/dialog) for interactive examples and prop controls.
