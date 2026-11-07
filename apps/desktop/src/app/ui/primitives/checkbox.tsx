import { useId } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetCheckboxProps extends UiCommonProps {
  readonly checked: boolean;
  readonly label: string;
  readonly onCheckedChange: (checked: boolean) => void;
  readonly className?: string;
}

export function VelvetCheckbox({
  checked,
  label,
  onCheckedChange,
  className = '',
  disabled,
  ...common
}: VelvetCheckboxProps) {
  const id = useId();
  return (
    <label {...uiDataProps(common)} className={`velvet-checkbox ${className}`.trim()} htmlFor={id}>
      <input
        checked={checked}
        disabled={disabled}
        id={id}
        onChange={(event) => onCheckedChange(event.currentTarget.checked)}
        type="checkbox"
      />
      <span>{label}</span>
    </label>
  );
}
