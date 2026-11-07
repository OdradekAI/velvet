import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetBadgeProps extends UiCommonProps {
  readonly children: ReactNode;
  readonly tone?: 'neutral' | 'info' | 'success' | 'warning' | 'danger';
  readonly className?: string;
}

export function VelvetBadge({ children, tone = 'neutral', className = '', ...common }: VelvetBadgeProps) {
  return (
    <span
      {...uiDataProps(common)}
      className={`velvet-badge velvet-badge--${tone} ${className}`.trim()}
      data-ui-kit-component="badge"
    >
      {children}
    </span>
  );
}
