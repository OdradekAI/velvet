# VelvetButton

`VelvetButton` is the Velvet primitive for command or local action control. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetButton } from './app/ui/index.js';
```

## Contract

- **Props:** `children`, `label`, `variant`, `disabled`, `busy`, `onClick`
- **States:** `default`, `quiet`, `danger`, `disabled`, `busy`, `focus`
- **Owner:** `host`; see `apps/desktop/src/app/ui/contracts/primitives/button.contract.ts`.
- **Slots:** `children`.
- **Actions:** `activate` is exposed through `onClick`; it does not dispatch a host command by itself.
- **Keyboard and accessibility:** Enter and Space activate the native button; `disabled` or `busy` disables it and blocks activation. `busy` also sets `aria-busy` and displays a decorative spinner.

`children?: ReactNode` takes precedence over `label?: string`. `variant` is `'default' | 'quiet' | 'danger'` and defaults to `'default'`. The wrapper accepts typed native button attributes, `className` and `ref?: Ref<HTMLButtonElement>`; `type` defaults to `'button'`. `onClick` receives `React.MouseEvent<HTMLButtonElement>`. The host owns command visibility, capability checks, enablement and command execution; the component only renders and invokes the callback.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The type accepts `ariaLive`, but the current `VelvetButton` wrapper does not pass it to `uiDataProps`, so it does not produce `aria-live` from `ariaLive`. Use a separate live region for announcements.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/button.stories.tsx`:

- `button/default`
- `button/busy`
- `button/disabled`


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

See [VelvetButton API and Playground](/ui-kit/components/button) for interactive examples and prop controls.
See [UI Kit action coverage](/reference/ui-kit/action-coverage) for action evidence.
