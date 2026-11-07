import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetTextProps extends UiCommonProps {
  readonly as?: 'p' | 'span' | 'strong' | 'small' | 'h1' | 'h2' | 'h3';
  readonly tone?: 'primary' | 'secondary' | 'muted' | 'danger' | 'success' | 'warning';
  readonly children: ReactNode;
  readonly className?: string;
  readonly id?: string;
  readonly role?: string;
}

export function VelvetText({
  as = 'span',
  tone = 'primary',
  children,
  className = '',
  id,
  role,
  ...common
}: VelvetTextProps) {
  const Tag = as;
  return (
    <Tag
      {...uiDataProps(common)}
      className={`velvet-text velvet-text--${tone} ${className}`.trim()}
      id={id}
      role={role}
    >
      {children}
    </Tag>
  );
}
