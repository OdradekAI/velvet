# VelvetInput

`VelvetInput` is the Velvet primitive for text entry control with host density and focus styling. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetInput } from './app/ui/index.js';
```

## Contract

- **Props:** `value`, `defaultValue`, `placeholder`, `disabled`, `busy`, `onChange`, `onKeyDown`
- **States:** `default`, `focus`, `disabled`, `busy`
- **Owner:** `component`; see `apps/desktop/src/app/ui/contracts/primitives/input.contract.ts`.
- **Slots:** none.
- **Actions:** `change` is exposed through `onChange`; `onKeyDown` remains a typed native keyboard callback.
- **Keyboard and accessibility:** Native text editing and focus semantics apply. `disabled` or `busy` disables editing and normal focus; `busy` also sets `aria-busy`. Supply an accessible name through a label or `ariaLabel`.

The wrapper accepts typed native input attributes, including `type`, `value`, `defaultValue`, `placeholder`, `readOnly`, `onChange` and `onKeyDown`, plus `className`. `onChange` receives `React.ChangeEvent<HTMLInputElement>`. With `value`, the caller controls the value and must update it after changes; with `defaultValue`, the native input owns subsequent edits. It adds no validation or persistence policy.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The type accepts `ariaLive`, but the current `VelvetInput` wrapper does not pass it to `uiDataProps`, so it does not produce `aria-live` from `ariaLive`. Use a separate live region for announcements.

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

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```

See [VelvetInput API and Playground](/ui-kit/components/input) for interactive examples and prop controls.
