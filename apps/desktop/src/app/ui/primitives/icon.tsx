import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetIconProps extends UiCommonProps {
  readonly children: ReactNode;
  readonly label?: string;
  readonly className?: string;
}

export function VelvetIcon({ children, label, className = '', ...common }: VelvetIconProps) {
  return (
    <span
      {...uiDataProps(common)}
      aria-hidden={label ? undefined : true}
      aria-label={label}
      className={`velvet-icon ${className}`.trim()}
      data-ui-kit-component="icon"
      role={label ? 'img' : undefined}
    >
      {children}
    </span>
  );
}
