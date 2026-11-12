# VelvetSelect

`VelvetSelect` is the Velvet primitive for native select control with host styling. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetSelect } from './app/ui/index.js';
```

## Contract

- **Props:** `value`, `defaultValue`, `disabled`, `busy`, `onChange`, `children`
- **States:** `default`, `focus`, `disabled`
- **Owner:** `component`; see `apps/desktop/src/app/ui/contracts/primitives/select.contract.ts`.
- **Slots:** `options`.
- **Actions:** `change` is exposed through the native `onChange` callback.
- **Keyboard and accessibility:** Native option selection and Arrow-key behavior apply. `disabled` or `busy` disables the control; `busy` also sets `aria-busy`. Provide a label or `ariaLabel`.

The wrapper accepts typed native select attributes, including `value`, `defaultValue`, `multiple`, `name`, `onChange` and `children`, plus `className`. Pass native option content through `children` (the contract's `options` slot). `onChange` receives `React.ChangeEvent<HTMLSelectElement>`. `value` is caller-controlled; `defaultValue` initializes native state. Native option, multiple-selection and form-submission semantics are retained.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The type accepts `ariaLive`, but the current `VelvetSelect` wrapper does not pass it to `uiDataProps`, so it does not produce `aria-live` from `ariaLive`. Use a separate live region for announcements.

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

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```

See [VelvetSelect API and Playground](/ui-kit/components/select) for interactive examples and prop controls.
