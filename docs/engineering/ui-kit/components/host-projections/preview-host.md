# VelvetPreviewHost

`VelvetPreviewHost` is the Velvet host projections component for preview lifecycle and fallback host.
It is implemented in `apps/desktop/src/app/ui/host-projections/`; its public
import remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `state`, `mode`, `message`, `toolbar`, `children`, `onRetry`, `onModeChange`, `onOpenExternal`
- **States:** loading, ready, empty, error, disabled
- **Async lifecycle:** success, cancelled, timeout, failure, retry
- **Accessibility:** named preview region; loading is busy and error/empty fallback states are announced
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/preview-host.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/preview-host.stories.tsx`
- **Host boundary:** layout, overlay, focus and projection state remain host-owned.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `preview-host/ready`
- `preview-host/error`
- `preview-host/retry`
- `preview-host/toolbar-mode`
- `preview-host/async-success`
- `preview-host/async-cancelled`
- `preview-host/async-timeout`
- `preview-host/async-failure`
- `preview-host/async-retry`

Async stories use the fake host adapter from the story runner. A request starts
as `pending`, then commits, cancels, times out or fails. Retry creates a new
attempt after a terminal failure and records both attempts in the host call log;
the component only renders the host-owned projection state and never owns the
underlying transport or job.

The browser fixture verifies the error live region, retry action and transition
to the ready preview state, plus the provider toolbar, mode marker, mode toggle
and explicit external-open callback.

`mode` is host-controlled. `onModeChange` and `onOpenExternal` are explicit host
actions; the component never stores zoom mode or opens a file directly.

The semantic visual contract keeps the preview on the canvas surface, reserves
a 48px control strip for provider actions and keeps zoom bounds in the host
projection (`5–800`). These are named catalog invariants; CSS classes remain
private.


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
