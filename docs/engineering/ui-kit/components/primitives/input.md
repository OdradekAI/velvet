# VelvetInput

`VelvetInput` is the Velvet primitive for text entry control with host density and focus styling. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetInput } from './app/ui/index.js';
```

## Contract

- **Props:** `value`/`defaultValue`, `placeholder`, `disabled`, `busy`, `onChange`
- **State:** default, busy
- **Owner:** see `apps/desktop/src/app/ui/contracts/primitives/input.contract.ts`.
- **Slots:** the contract file lists the supported child/data slots.
- **Actions:** Semantic actions are observed through the typed callback or native control event.
- **Keyboard and accessibility:** Native text editing and focus semantics; busy disables editing.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/input.stories.tsx`:

- `input/default`
- `input/change`
- `input/busy`


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
