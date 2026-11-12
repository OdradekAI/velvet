# VelvetText

`VelvetText` is the Velvet primitive for semantic text and tone wrapper. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetText } from './app/ui/index.js';
```

## Contract

- **Props:** `as`, `tone`, `children`, `className`, `ariaLabel`
- **States:** `default`, `tone`
- **Owner:** `component`; see `apps/desktop/src/app/ui/contracts/primitives/text.contract.ts`.
- **Slots:** `children`.
- **Actions:** No semantic action is defined.
- **Keyboard and accessibility:** No keyboard behavior is introduced; the chosen HTML element and child semantics are preserved.

`children: ReactNode` is required. `as` is `'p' | 'span' | 'strong' | 'small' | 'h1' | 'h2' | 'h3'` (default `'span'`); `tone` is `'primary' | 'secondary' | 'muted' | 'danger' | 'success' | 'warning'` (default `'primary'`). The wrapper also accepts `className?: string`, `id?: string` and `role?: string`. Use `as` to select appropriate native text/heading semantics.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/text.stories.tsx`:

- `text/default`
- `text/tones`


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

See [VelvetText API and Playground](/ui-kit/components/text) for interactive examples and prop controls.
