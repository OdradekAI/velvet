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
  return (
    <section
      aria-label={ariaLabel}
      className={`velvet-panel velvet-panel--surface-${surface} ${className}`.trim()}
      data-ui-kit-component="panel"
      id={id}
    >
      {title ? <header className="velvet-panel__header">{title}</header> : null}
      <div className="velvet-panel__body">{children}</div>
    </section>
  );
}
