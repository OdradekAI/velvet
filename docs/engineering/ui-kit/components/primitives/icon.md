# VelvetIcon

`VelvetIcon` is the Velvet primitive for accessible or decorative icon wrapper. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetIcon } from './app/ui/index.js';
```

## Contract

- **Props:** `children`, `label`, `className`
- **State:** default, labelled
- **Owner:** see `apps/desktop/src/app/ui/contracts/primitives/icon.contract.ts`.
- **Slots:** the contract file lists the supported child/data slots.
- **Actions:** No semantic action; this component preserves child semantics.
- **Keyboard and accessibility:** No keyboard behavior is introduced by this layout or display component.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/icon.stories.tsx`:

- `icon/default`
- `icon/labelled`


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
