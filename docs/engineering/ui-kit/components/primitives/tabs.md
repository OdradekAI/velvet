# VelvetTabs

`VelvetTabs` is the Velvet primitive for roving-focus tab selection control. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetTabs } from './app/ui/index.js';
```

## Contract

- **Props:** `tabs`, `selectedId`, `onSelect`
- **States:** `selected`, `disabled`, `focus`
- **Owner:** `host`; see `apps/desktop/src/app/ui/contracts/primitives/tabs.contract.ts`.
- **Slots:** `tab labels`.
- **Actions:** `select` and `navigate` call `onSelect` with a tab id. Clicking requests selection; keyboard navigation also moves focus.
- **Keyboard and accessibility:** The tablist is horizontal and is named by `ariaLabel` or the fallback `Tabs`. Only the selected tab has `tabIndex={0}`. ArrowRight/ArrowDown select and focus the next enabled tab; ArrowLeft/ArrowUp select and focus the previous one, wrapping at either end. Home/End select and focus the first/last enabled tab. Disabled tabs are skipped.
- **Qualification:** Node/Happy DOM story plus real-browser tablist orientation, selection and focus fixture.

`tabs: readonly VelvetTab[]`, `selectedId: string` and `onSelect: (id: string) => void` are required. Each tab has `id: string`, `label: string`, optional `disabled?: boolean` and `badge?: string | number`. The host must update `selectedId` after `onSelect` and keep it on an enabled tab to retain a tab stop. The component renders no `tabpanel` or `aria-controls`; the host owns content and its accessible association. Common `disabled` or `busy` props do not disable the tab buttons; use each tab's `disabled` field.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

`describedBy` and `ariaLive` apply to the outer wrapper, not the inner tablist.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/tabs.stories.tsx`:

- `tabs/default`
- `tabs/disabled`

The default scenario covers ArrowRight navigation, direct tab selection and
focus following the selected tab.


## Evidence

- `host-guarantee`: `docs/engineering/ui-kit-component-guide.md`
- `observed`: `apps/desktop/test/ui-kit-components.test.mjs` and the story runner
- `unknown`: any state not listed in the contract remains unclaimed until a
  fixture or browser qualification adds evidence

## Validation

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-primitives
```

See [VelvetTabs API and Playground](/ui-kit/components/tabs) for interactive examples and prop controls.
