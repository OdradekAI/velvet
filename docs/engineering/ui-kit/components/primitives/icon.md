# VelvetIcon

`VelvetIcon` is the Velvet primitive for accessible or decorative icon wrapper. It lives in the
`primitives` layer and is imported from `apps/desktop/src/app/ui/index.ts`.

```tsx
import { VelvetIcon } from './app/ui/index.js';
```

## Contract

- **Props:** `name`, `children`, `label`, `className`
- **States:** `default`, `decorative`, `labelled`
- **Owner:** `component`; see `apps/desktop/src/app/ui/contracts/primitives/icon.contract.ts`.
- **Slots:** `name`, `children`.
- **Actions:** No semantic action is defined.
- **Keyboard and accessibility:** No keyboard behavior is added. A non-empty `label` produces `role="img"` and `aria-label`; otherwise the wrapper has `aria-hidden="true"`.

`name?: VelvetIconName` selects a host-owned semantic glyph. `children?: ReactNode` is the compatibility slot for custom renderer content and takes precedence over `name`. The supported name literals are defined in `apps/desktop/src/app/ui/primitives/icon.tsx`. `label?: string` controls accessible icon semantics; an empty label is treated as decorative. The explicit `label` handling overrides the inherited `ariaLabel` prop.

The wrapper also accepts `UiCommonProps`: `apiVersion` (default `'1.0'`), `testId`, `ariaLabel`, `describedBy`, `disabled`, `busy`, `ariaLive?: 'off' | 'polite' | 'assertive'` and `data-*` metadata. See `apps/desktop/src/app/ui/internal/types.ts` and `apps/desktop/src/app/ui/internal/ui-data-props.ts`; these props do not imply uniform disabling behavior across display and interactive components. `className` is supported.

The contract is semantic. React implementation details, Tailwind classes, Radix
attributes and private DOM structure are not public API.

## Stories

The executable stories are in
`apps/desktop/src/app/ui/stories/primitives/icon.stories.tsx`:

- `icon/default`
- `icon/labelled`
- `icon/semantic-name`


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

See [VelvetIcon API and Playground](/ui-kit/components/icon) for interactive examples and prop controls.
