# VelvetSelect

`VelvetSelect` is the Velvet primitive for native select control with host styling. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetSelect } from './app/ui/index.js';
```

## Contract

- **Props:** `value`/`defaultValue`, `children`, `disabled`, `onChange`
- **State:** default, disabled
- **Owner:** see `apps/desktop/src/app/ui/contracts/primitives/select.contract.ts`.
- **Slots:** the contract file lists the supported child/data slots.
- **Actions:** Semantic actions are observed through the typed callback or native control event.
- **Keyboard and accessibility:** Native option and Arrow-key semantics.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/select.stories.tsx`:

- `select/default`
- `select/disabled`


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
