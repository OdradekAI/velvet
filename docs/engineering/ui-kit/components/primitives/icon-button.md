# VelvetIconButton

`VelvetIconButton` is the Velvet primitive for compact icon-first action with an accessible label. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetIconButton } from './app/ui/index.js';
```

## Contract

- **Props:** `icon`, `label`, `disabled`, `busy`, `onClick`
- **State:** default, disabled
- **Owner:** see `apps/desktop/src/app/ui/contracts/primitives/icon-button.contract.ts`.
- **Slots:** the contract file lists the supported child/data slots.
- **Actions:** Semantic actions are observed through the typed callback or native control event.
- **Keyboard and accessibility:** Enter and Space activate the native button; the label is required.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/icon-button.stories.tsx`:

- `icon-button/default`
- `icon-button/disabled`


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
