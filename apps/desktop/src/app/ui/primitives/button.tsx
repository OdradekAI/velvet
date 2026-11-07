import type { ButtonHTMLAttributes, ReactNode, Ref } from 'react';
import { uiDataProps } from '../internal/ui-data-props.js';
import type { UiButtonVariant, UiCommonProps } from '../internal/types.js';

export interface VelvetButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children' | 'disabled' | 'aria-label' | 'aria-describedby'>,
    UiCommonProps {
  readonly children?: ReactNode;
  readonly label?: string;
  readonly variant?: UiButtonVariant;
  readonly ref?: Ref<HTMLButtonElement>;
}

export function VelvetButton({
  children,
  label,
  variant = 'default',
  className = '',
  disabled,
  busy,
  apiVersion,
  testId,
  ariaLabel,
  describedBy,
  ref,
  ...props
}: VelvetButtonProps) {
  return (
    <button
      {...props}
      {...uiDataProps({ apiVersion, testId, ariaLabel, describedBy, disabled, busy })}
      className={`velvet-button velvet-button--${variant} ${className}`.trim()}
      data-ui-kit-component="button"
      disabled={disabled || busy}
      ref={ref}
      type={props.type ?? 'button'}
    >
      {busy ? <span aria-hidden="true" className="velvet-button__spinner" /> : null}
      {children ?? label}
    </button>
  );
}
