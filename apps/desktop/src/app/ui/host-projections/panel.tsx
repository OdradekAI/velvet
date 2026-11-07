import { useId } from 'react';
import type { ReactNode } from 'react';

export interface VelvetPanelProps {
  readonly title?: string;
  readonly children: ReactNode;
  readonly className?: string;
  readonly ariaLabel?: string;
  readonly id?: string;
  readonly surface?: 'panel' | 'raised' | 'overlay';
}

export function VelvetPanel({
  title,
  children,
  className = '',
  ariaLabel,
  id,
  surface = 'panel',
}: VelvetPanelProps) {
  const headingId = useId();
  return (
    <section
      aria-label={ariaLabel ?? (title ? undefined : 'Panel')}
      aria-labelledby={ariaLabel || !title ? undefined : headingId}
      className={`velvet-panel velvet-panel--surface-${surface} ${className}`.trim()}
      data-ui-kit-component="panel"
      id={id}
      role="region"
    >
      {title ? <header className="velvet-panel__header"><h2 id={headingId}>{title}</h2></header> : null}
      <div className="velvet-panel__body">{children}</div>
    </section>
  );
}
