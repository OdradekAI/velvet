# VelvetTabs

`VelvetTabs` is the Velvet primitive for roving-focus tab selection control. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetTabs } from './app/ui/index.js';
```

## Contract

- **Props:** `tabs`, `selectedId`, `onSelect`
- **State:** default, disabled
- **Owner:** see `apps/desktop/src/app/ui/contracts/primitives/tabs.contract.ts`.
- **Slots:** the contract file lists the supported child/data slots.
- **Actions:** Semantic actions are observed through the typed callback or native control event.
- **Keyboard and accessibility:** Arrow keys move between enabled tabs; Home and End select the first/last enabled tab.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/tabs.stories.tsx`:

- `tabs/default`
- `tabs/disabled`

The default scenario covers ArrowRight navigation and direct tab selection.


## Evidence

- `host-guarantee`: `docs/engineering/ui-kit-component-guide.md`
- `observed`: `apps/desktop/test/ui-kit-components.test.mjs` and the story runner
- `unknown`: any state not listed in the contract remains unclaimed until a
  fixture or browser qualification adds evidence

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```
