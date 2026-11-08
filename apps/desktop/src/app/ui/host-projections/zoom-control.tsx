import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetZoomControlProps extends UiCommonProps {
  readonly value: number;
  readonly min?: number;
  readonly max?: number;
  readonly step?: number;
  readonly onChange: (value: number) => void;
  readonly disabled?: boolean;
  readonly className?: string;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function VelvetZoomControl({
  value,
  min = 25,
  max = 500,
  step = 5,
  onChange,
  disabled = false,
  className = '',
  ...common
}: VelvetZoomControlProps) {
  const nextValue = (delta: number): void => {
    onChange(clamp(value + delta, min, max));
  };

  return (
    <div
      {...uiDataProps(common)}
      aria-label={common.ariaLabel ?? 'Zoom'}
      className={`velvet-zoom-control ${className}`.trim()}
      role="group"
    >
      <button aria-label="Zoom out" className="velvet-zoom-control__step" disabled={disabled || value <= min} onClick={() => nextValue(-step)} type="button">−</button>
      <input aria-label="Zoom level" aria-valuetext={`${value}%`} className="velvet-zoom-control__range" disabled={disabled} max={max} min={min} onChange={(event) => onChange(Number(event.target.value))} step={step} type="range" value={value} />
      <span aria-live="polite" className="velvet-zoom-control__value">{value}%</span>
      <button aria-label="Zoom in" className="velvet-zoom-control__step" disabled={disabled || value >= max} onClick={() => nextValue(step)} type="button">+</button>
    </div>
  );
}
