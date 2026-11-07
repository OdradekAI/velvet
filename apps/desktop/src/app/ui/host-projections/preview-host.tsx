import type { ReactNode } from 'react';
import { VelvetState, type VelvetStateKind } from '../primitives/index.js';

export interface VelvetPreviewHostProps {
  readonly state: 'loading' | 'ready' | 'empty' | 'error' | 'disabled';
  readonly children?: ReactNode;
  readonly message?: string;
  readonly onRetry?: () => void;
}

export function VelvetPreviewHost({ state, children, message, onRetry }: VelvetPreviewHostProps) {
  if (state === 'ready') {
    return <section aria-label="Preview" className="velvet-preview-host" data-preview-state={state}>{children}</section>;
  }
  const kind: VelvetStateKind = state === 'loading' ? 'loading' : state === 'error' ? 'error' : 'empty';
  return (
    <section aria-label="Preview" className="velvet-preview-host" data-preview-state={state}>
      <VelvetState
        description={message}
        kind={kind}
        onRetry={state === 'error' ? onRetry : undefined}
        title={state === 'disabled' ? 'Preview unavailable' : state === 'loading' ? 'Loading preview' : state === 'error' ? 'Preview failed' : 'No preview'}
      />
    </section>
  );
}
