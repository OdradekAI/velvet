import type { ReactNode } from 'react';
import { VelvetButton } from './button.js';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export type VelvetStateKind = 'loading' | 'empty' | 'error';

export interface VelvetStateProps extends UiCommonProps {
  readonly kind: VelvetStateKind;
  readonly title: string;
  readonly description?: string;
  readonly retryLabel?: string;
  readonly onRetry?: () => void;
  readonly action?: ReactNode;
  readonly className?: string;
}

export function VelvetState({
  kind,
  title,
  description,
  retryLabel = 'Retry',
  onRetry,
  action,
  className = '',
  ...common
}: VelvetStateProps) {
  return (
    <section
      {...uiDataProps(common)}
      aria-busy={kind === 'loading' ? 'true' : undefined}
      aria-live={kind === 'error' ? 'assertive' : 'polite'}
      className={`velvet-state velvet-state--${kind} ${className}`.trim()}
      data-ui-kit-component="state"
      role={kind === 'error' ? 'alert' : 'status'}
    >
      <strong>{title}</strong>
      {description ? <p>{description}</p> : null}
      {kind === 'error' && onRetry ? <VelvetButton onClick={onRetry}>{retryLabel}</VelvetButton> : null}
      {action}
    </section>
  );
}
