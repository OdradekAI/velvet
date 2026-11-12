# VelvetStack

`VelvetStack` is the Velvet primitive for token-spaced vertical or horizontal layout. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetStack } from './app/ui/index.js';
```

## Contract

- **Props:** `children`, `direction`, `gap`, `className`
- **States:** `default`, `direction`, `gap`
- **Owner:** `component`; see `apps/desktop/src/app/ui/contracts/primitives/stack.contract.ts`.
- **Slots:** `children`.
- **Actions:** No semantic action is defined.
- **Keyboard and accessibility:** No keyboard behavior is introduced; child reading order, focus order and semantics are preserved.

`children: ReactNode` is required. `direction` is `'column' | 'row'` and defaults to `'column'`; `gap` is `'none' | 'xs' | 'sm' | 'md' | 'lg'` and defaults to `'md'`. The component owns presentation only and does not reorder children.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

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

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```

See [VelvetStack API and Playground](/ui-kit/components/stack) for interactive examples and prop controls.
