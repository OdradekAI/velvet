# VelvetStack

`VelvetStack` is the Velvet primitive for token-spaced vertical or horizontal layout. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetStack } from './app/ui/index.js';
```

## Contract

- **Props:** `children`, `direction`, `gap`, `className`
- **State:** default, directions
- **Owner:** see `apps/desktop/src/app/ui/contracts/primitives/stack.contract.ts`.
- **Slots:** the contract file lists the supported child/data slots.
- **Actions:** No semantic action; this component preserves child semantics.
- **Keyboard and accessibility:** No keyboard behavior is introduced by this layout or display component.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/stack.stories.tsx`:

- `stack/default`
- `stack/directions`


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
