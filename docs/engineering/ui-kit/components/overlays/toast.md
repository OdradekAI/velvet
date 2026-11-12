# VelvetToastHost

`VelvetToastHost` is the Velvet overlays component for live notification projection.
It is implemented in `apps/desktop/src/app/ui/overlays/`; its public import
remains available from `apps/desktop/src/app/ui/index.ts`.

## Contract

- **Props:** `items`, `onDismiss`, `className`
- **States:** `info`, `success`, `warning`, `error`
- **Contract:** `apps/desktop/src/app/ui/contracts/overlays/toast.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/overlays/toast.stories.tsx`
- **Qualification:** Node/Happy DOM and real browser live-region/dismissal fixture
- **Host boundary:** The host supplies the notification items and handles dismissal. `VelvetToastHost` renders that projection; it does not remove items, persist notifications or schedule automatic expiry.

`items: readonly VelvetToastItem[]` and `onDismiss: (id: string) => void` are required; `className?: string` is optional. Each item requires `id: string` and `message: string`; optional fields are `title?: string`, `severity?: 'info' | 'success' | 'warning' | 'error'` (default `'info'`) and `action?: ReactNode`. `action` is a content slot, not a command callback.

`VelvetToastHost` renders a region named `'Notifications'`. Error items use `role="alert"` and `aria-live="assertive"`; other severities use `role="status"` and `aria-live="polite"`. The dismiss icon button is named `'Dismiss notification'` and supports native button keyboard activation. The `dismiss` action only calls `onDismiss(item.id)`; the owner must remove the item from the next `items` projection.

For local host composition, `VelvetToastProvider` owns an in-memory list and renders `VelvetToastHost`. `useVelvetToast()` exposes `push(item: Omit<VelvetToastItem, 'id'>): string` and `dismiss(id: string): void`. The provider assigns increasing `toast-N` ids for that mounted provider and removes items on dismissal; it has no persistence or expiry policy. Calling the hook outside the provider throws `useVelvetToast must be used inside VelvetToastProvider`.

The component contract is semantic. Radix types, private DOM structure, Tailwind
classes and Workbench implementation details are not public API.

## Stories and evidence

- `toast/default`
- `toast/error`


The current story runner provides deterministic fixture coverage. Required for overlays and host projections: run the real-browser qualification fixture when changing focus, portal, geometry or host projection behavior.
Unverified external behavior remains `unknown` until a runtime capture proves it.

## Validation

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-primitives
```

See [VelvetToastHost API and Playground](/ui-kit/components/toast) for interactive examples and prop controls.
