import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetProgressProps extends UiCommonProps {
  readonly value?: number;
  readonly label?: string;
  readonly tone?: 'accent' | 'success' | 'danger';
  readonly className?: string;
}

export function VelvetProgress({
  value,
  label,
  tone = 'accent',
  className = '',
  ...common
}: VelvetProgressProps) {
  const determinate = value !== undefined;
  const bounded = determinate ? Math.max(0, Math.min(100, value)) : undefined;
  return (
    <div {...uiDataProps(common)} className={`velvet-progress velvet-progress--${tone} ${className}`.trim()} data-ui-kit-component="progress">
      {label ? <span className="velvet-progress__label">{label}</span> : null}
      <progress aria-label={label ?? 'Progress'} max={100} value={bounded} />
      {determinate ? <span className="velvet-progress__value">{bounded}%</span> : null}
    </div>
  );
}
