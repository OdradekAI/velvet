import type { SelectHTMLAttributes } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetSelectProps
  extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'disabled' | 'aria-label' | 'aria-describedby'>,
    UiCommonProps {}

export function VelvetSelect({ className = '', disabled, busy, ...props }: VelvetSelectProps) {
  const { apiVersion, testId, ariaLabel, describedBy, ...selectProps } = props;
  return (
    <select
      {...selectProps}
      {...uiDataProps({ apiVersion, testId, ariaLabel, describedBy, disabled, busy })}
      className={`velvet-select ${className}`.trim()}
      data-ui-kit-component="select"
      disabled={disabled || busy}
    />
  );
}
