# VelvetButton

`VelvetButton` is the Velvet primitive for command or local action control. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetButton } from './app/ui/index.js';
```

## Contract

- **Props:** `children`, `label`, `variant`, `disabled`, `busy`, `onClick`
- **State:** default, busy, disabled
- **Owner:** see `apps/desktop/src/app/ui/contracts/primitives/button.contract.ts`.
- **Slots:** the contract file lists the supported child/data slots.
- **Actions:** Semantic actions are observed through the typed callback or native control event.
- **Keyboard and accessibility:** Enter and Space activate the native button; disabled/busy states block activation.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/button.stories.tsx`:

- `button/default`
- `button/busy`
- `button/disabled`


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
