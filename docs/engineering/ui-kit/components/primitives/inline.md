# VelvetInline

`VelvetInline` is the Velvet primitive for token-spaced horizontal layout. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetInline } from './app/ui/index.js';
```

## Contract

- **Props:** `children`, `gap`, `className`
- **States:** `row`
- **Owner:** `component`; see `apps/desktop/src/app/ui/contracts/primitives/inline.contract.ts`.
- **Slots:** `children`.
- **Actions:** No semantic action is defined.
- **Keyboard and accessibility:** No keyboard behavior is introduced; child reading order, focus order and semantics are preserved.

`VelvetInlineProps` inherits `VelvetStackProps` except `direction`, which is fixed to `'row'`. `children: ReactNode` is required; `gap` is `'none' | 'xs' | 'sm' | 'md' | 'lg'` and defaults to `'md'`. It arranges badges, text and compact actions without reordering the children.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/inline.stories.tsx`:

- `inline/default`


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

See [VelvetInline API and Playground](/ui-kit/components/inline) for interactive examples and prop controls.
