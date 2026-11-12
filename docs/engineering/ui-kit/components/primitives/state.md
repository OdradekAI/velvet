# VelvetState

`VelvetState` is the Velvet primitive for loading, empty and error state composition. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetState } from './app/ui/index.js';
```

## Contract

- **Props:** `kind`, `title`, `description`, `retryLabel`, `onRetry`, `action`
- **States:** `loading`, `empty`, `error`
- **Owner:** `host`; see `apps/desktop/src/app/ui/contracts/primitives/state.contract.ts`.
- **Slots:** `action`.
- **Actions:** `retry` invokes `onRetry`. `action` is a content slot available in every kind and does not imply a host command. Common `disabled` or `busy` props do not disable the retry button.
- **Keyboard and accessibility:** Loading sets `aria-busy="true"`; loading and empty use `role="status"` with `aria-live="polite"`. Error uses `role="alert"` with `aria-live="assertive"`. Only `kind="error"` with `onRetry` renders the native retry button, which supports Enter and Space.

`kind: 'loading' | 'empty' | 'error'` and `title: string` are required. Optional props include `description?: string`, `retryLabel?: string` (default `'Retry'`), `onRetry?: () => void`, `action?: ReactNode` and `className`. The host owns loading, failure and retry work; the component does not change `kind` or run a retry policy automatically. `kind` determines `aria-busy` and `aria-live`, overriding the inherited `busy` and `ariaLive` values.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/state.stories.tsx`:

- `state/loading`
- `state/empty`
- `state/error-retry`


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

See [VelvetState API and Playground](/ui-kit/components/state) for interactive examples and prop controls.
