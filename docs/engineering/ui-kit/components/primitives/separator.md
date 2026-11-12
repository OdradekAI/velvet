# VelvetSeparator

`VelvetSeparator` is the Velvet primitive for visual and semantic divider. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetSeparator } from './app/ui/index.js';
```

## Contract

- **Props:** `orientation`
- **States:** `horizontal`, `vertical`
- **Owner:** `component`; see `apps/desktop/src/app/ui/contracts/primitives/separator.contract.ts`.
- **Slots:** none.
- **Actions:** No semantic action is defined.
- **Keyboard and accessibility:** The non-focusable divider renders `role="separator"` and `aria-orientation`. It introduces no keyboard behavior.

The sole prop is `orientation?: 'horizontal' | 'vertical'`, defaulting to `'horizontal'`. This wrapper does not accept `UiCommonProps`, `children` or `className`.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/separator.stories.tsx`:

- `separator/horizontal`
- `separator/vertical`


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
```

See [VelvetSeparator API and Playground](/ui-kit/components/separator) for interactive examples and prop controls.
