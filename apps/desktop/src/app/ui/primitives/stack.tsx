import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetStackProps extends UiCommonProps {
  readonly children: ReactNode;
  readonly className?: string;
  readonly gap?: 'none' | 'xs' | 'sm' | 'md' | 'lg';
  readonly direction?: 'column' | 'row';
}

export function VelvetStack({
  children,
  className = '',
  gap = 'md',
  direction = 'column',
  ...common
}: VelvetStackProps) {
  return (
    <div
      {...uiDataProps(common)}
      className={`velvet-stack velvet-stack--${direction} velvet-stack--gap-${gap} ${className}`.trim()}
    >
      {children}
    </div>
  );
}
