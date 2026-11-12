# VelvetProgress

`VelvetProgress` is the Velvet primitive for job progress projection. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetProgress } from './app/ui/index.js';
```

## Contract

- **Props:** `value`, `label`, `tone`
- **States:** `determinate`, `indeterminate`, `success`, `danger`
- **Owner:** `host`; see `apps/desktop/src/app/ui/contracts/primitives/progress.contract.ts`.
- **Slots:** none.
- **Actions:** No semantic action is defined.
- **Keyboard and accessibility:** No keyboard behavior is added. The native progress element has `max={100}`; common ARIA metadata applies to its outer wrapper, while `label` names the progress element itself.

`value?: number` selects determinate progress when supplied; finite values are clamped to 0–100 and displayed as a percentage. Omitting `value` selects indeterminate progress. `label?: string` names the native progress element, falling back to `'Progress'`; `tone` is `'accent' | 'success' | 'danger'` and defaults to `'accent'`. The host supplies the progress projection; this component does not start, monitor or cancel Jobs.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/progress.stories.tsx`:

- `progress/determinate`
- `progress/indeterminate`


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

See [VelvetProgress API and Playground](/ui-kit/components/progress) for interactive examples and prop controls.
