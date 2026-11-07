import type { InputHTMLAttributes } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetInputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'disabled' | 'aria-label' | 'aria-describedby'>,
    UiCommonProps {}

export function VelvetInput({ className = '', disabled, busy, ...props }: VelvetInputProps) {
  const { apiVersion, testId, ariaLabel, describedBy, ...inputProps } = props;
  return (
    <input
      {...inputProps}
      {...uiDataProps({ apiVersion, testId, ariaLabel, describedBy, disabled, busy })}
      className={`velvet-input ${className}`.trim()}
      data-ui-kit-component="input"
      disabled={disabled || busy}
    />
  );
}
