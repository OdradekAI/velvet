import { useId } from 'react';
import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetPaneProps extends UiCommonProps {
  readonly title: string;
  readonly region: 'navigation' | 'context' | 'operations' | 'preview';
  readonly children: ReactNode;
  readonly className?: string;
}

export function VelvetPane({ title, region, children, className = '', ...common }: VelvetPaneProps) {
  const headingId = useId();
  return (
    <section {...uiDataProps(common)} aria-label={common.ariaLabel} aria-labelledby={common.ariaLabel ? undefined : headingId} className={`velvet-pane velvet-pane--${region} ${className}`.trim()} data-ui-kit-region={region} role="region">
      <header className="velvet-pane__header"><h2 id={headingId}>{title}</h2></header>
      <div className="velvet-pane__body">{children}</div>
    </section>
  );
}
