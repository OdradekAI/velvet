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
  return (
    <section {...uiDataProps(common)} aria-label={title} className={`velvet-pane velvet-pane--${region} ${className}`.trim()} data-ui-kit-region={region}>
      <header className="velvet-pane__header">{title}</header>
      <div className="velvet-pane__body">{children}</div>
    </section>
  );
}
