# VelvetContextMenu

`VelvetContextMenu` is the Velvet overlays component for trigger-driven contextual command menu.
It is implemented in `apps/desktop/src/app/ui/overlays/`; its public import
remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `actions`, `children`; each action may include a recursive `submenu`
- **States:** `closed`, `open`, `disabled`, `checked`, `submenu`
- **Contract:** `apps/desktop/src/app/ui/contracts/overlays/context-menu.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/overlays/context-menu.stories.tsx`
- **Qualification:** browser (Radix portal, pointer dismissal and keyboard selection)
- **Host boundary:** The host owns action data, callbacks and command lifecycle. The wrapper delegates transient open state, portal, roving focus and submenu transitions to Radix; it exposes no controlled `open` prop.

`children: ReactNode` and `actions: readonly VelvetContextMenuAction[]` are required. Each action requires `id: string` and `label: string`; optional fields are `disabled?: boolean`, `checked?: boolean`, `shortcutHint?: string`, `separatorBefore?: boolean`, `group?: string`, `submenu?: readonly VelvetContextMenuAction[]` and `onSelect?: () => void`. A non-empty `submenu` renders a submenu trigger instead of a leaf action. `group` is declared but currently not rendered; `shortcutHint` only displays a hint and does not register a shortcut.

For leaf actions, a defined `checked` value produces `role="menuitemcheckbox"` and `aria-checked`, including `false`. Selection invokes `onSelect` when supplied; the wrapper does not toggle `checked` itself. The host must update action data. Disabled actions cannot be selected. Keyboard navigation and submenu direction are delegated to Radix; Escape and outside-pointer dismissal are covered by the browser fixture. Slots are `trigger`, `actions` and `submenu.actions`; semantic actions are `open`, `select`, `dismiss` and `navigate`.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `context-menu/default`
- `context-menu/checked`
- `context-menu/submenu`

The real-browser fixture adds `context-menu/select`, `context-menu/cancel-escape`
and `context-menu/cancel-outside` coverage, including checked item semantics.

Nested actions render through the Velvet wrapper's submenu slot. Radix owns the
portal, roving focus and directional submenu transitions; the host still owns
the action callbacks and any command lifecycle.


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

See [VelvetContextMenu API and Playground](/ui-kit/components/context-menu) for interactive examples and prop controls.
