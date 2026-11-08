# VelvetZoomControl

`VelvetZoomControl` is a host-owned, controlled zoom projection. It exposes a
native range slider with explicit increment/decrement buttons and a live value.
The host owns the zoom value and decides how that value changes the asset grid;
the component does not persist settings or resize content by itself.

## Contract

- **Props:** `value`, `min`, `max`, `step`, `onChange`, `disabled`
- **States:** default, disabled, min, max, keyboard
- **Actions:** increment, decrement, change
- **Stories:** `zoom-control/default`, `zoom-control/keyboard`,
  `zoom-control/bounds`, `zoom-control/disabled`
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/zoom-control.contract.ts`
- **Implementation:** `apps/desktop/src/app/ui/host-projections/zoom-control.tsx`
- **Accessibility:** named group, native range semantics, polite live value, `aria-valuetext` and keyboard step behavior

## Evidence boundary

The qualification fixture exposes an `AXSlider` with increment/decrement
actions and an initial value of `180`. A controlled probe moved the value to
`505`, one decrement produced `500`, and 64 more decrements restored `180`,
establishing an observed step of `5` for that fixture state. This proves the
slider and step behavior only; external minimum, maximum, persistence and grid
reflow semantics remain unknown.

The Velvet wrapper therefore defaults to `min=25`, `max=500` and `step=5` as
host policy, while keeping all three values configurable. These defaults are a
Velvet design choice, not a claim about an external application full range.

## Usage

```tsx
<VelvetZoomControl
  ariaLabel="Asset zoom"
  value={zoom}
  onChange={setZoom}
  min={25}
  max={500}
  step={5}
/>
```

The component uses native range semantics, so browser qualification covers
keyboard focus, arrow-key changes, bounds, disabled state and visual geometry.
