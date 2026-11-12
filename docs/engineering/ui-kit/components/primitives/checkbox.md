# VelvetCheckbox

`VelvetCheckbox` is the Velvet primitive for controlled boolean setting control. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetCheckbox } from './app/ui/index.js';
```

## Contract

- **Props:** `checked`, `label`, `disabled`, `onCheckedChange`
- **States:** `checked`, `unchecked`, `disabled`, `focus`
- **Owner:** `host`; see `apps/desktop/src/app/ui/contracts/primitives/checkbox.contract.ts`.
- **Slots:** none.
- **Actions:** `toggle` calls `onCheckedChange` with `event.currentTarget.checked`; the callback carries a boolean, not an event.
- **Keyboard and accessibility:** Tab focuses the enabled native checkbox; Space or a click on the checkbox/label requests a toggle. Disabled checkboxes cannot be focused or toggled through normal user interaction.

`checked: boolean`, `label: string` and `onCheckedChange: (checked: boolean) => void` are required. After the callback, the host must pass the updated `checked` value. `ariaLabel`, `describedBy` and the remaining common metadata apply to the outer label, not directly to the input. The visible `label` names the input through a generated id. `disabled` disables the input; `busy` alone does not disable it.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/checkbox.stories.tsx`:

- `checkbox/checked`
- `checkbox/disabled`


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

See [VelvetCheckbox API and Playground](/ui-kit/components/checkbox) for interactive examples and prop controls.
