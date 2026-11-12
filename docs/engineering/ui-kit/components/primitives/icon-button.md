# VelvetIconButton

`VelvetIconButton` is the Velvet primitive for compact icon-first action with an accessible label. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetIconButton } from './app/ui/index.js';
```

## Contract

- **Props:** `icon`, `label`, `disabled`, `busy`, `onClick`
- **States:** `default`, `disabled`, `focus`
- **Owner:** `host`; see `apps/desktop/src/app/ui/contracts/primitives/icon-button.contract.ts`.
- **Slots:** `icon`.
- **Actions:** `activate` is exposed through the inherited `onClick` callback.
- **Keyboard and accessibility:** Enter and Space activate the native button. `disabled` or `busy` blocks activation; the accessible `label` is required.

`icon: ReactNode` and `label: string` are required. The wrapper inherits `VelvetButtonProps` except `children`, `label` and `ariaLabel`; this includes `variant`, `onClick`, `className`, `describedBy`, native button attributes and `ref`. `variant` is `'default' | 'quiet' | 'danger'` with default `'default'`. `label` supplies the button's `aria-label`; icon content is hidden from assistive technology. The host owns command eligibility and execution.

Inherited common props include `apiVersion` (default `'1.0'`), `testId`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. `ariaLabel` is excluded because `label` supplies it.

The type accepts `ariaLive`, but the current `VelvetButton` wrapper does not pass it to `uiDataProps`, so it does not produce `aria-live` from `ariaLive`. Use a separate live region for announcements.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/icon-button.stories.tsx`:

- `icon-button/default`
- `icon-button/disabled`


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

See [VelvetIconButton API and Playground](/ui-kit/components/icon-button) for interactive examples and prop controls.
