import type { ReactNode } from 'react';
import { VelvetButton, VelvetState, type VelvetStateKind } from '../primitives/index.js';

export type VelvetPreviewMode = 'fit' | 'actual';

export interface VelvetPreviewProviderAction {
  readonly id: string;
  readonly label: string;
  readonly disabled?: boolean;
  readonly pressed?: boolean;
}

export interface VelvetPreviewHostProps {
  readonly state: 'loading' | 'ready' | 'empty' | 'error' | 'disabled';
  readonly mode?: VelvetPreviewMode;
  readonly toolbar?: ReactNode;
  readonly children?: ReactNode;
  readonly message?: string;
  readonly onRetry?: () => void;
  readonly onModeChange?: (mode: VelvetPreviewMode) => void;
  readonly providerActions?: readonly VelvetPreviewProviderAction[];
  readonly onProviderAction?: (actionId: string) => void;
  readonly onOpenExternal?: () => void;
}

export function VelvetPreviewHost({ state, mode = 'fit', toolbar, children, message, onRetry, onModeChange, providerActions, onProviderAction, onOpenExternal }: VelvetPreviewHostProps) {
  if (state === 'ready') {
    return (
      <section aria-label="Preview" className="velvet-preview-host" data-preview-mode={mode} data-preview-state={state} role="region">
        {toolbar || onModeChange || providerActions?.length || onOpenExternal ? (
          <div aria-label="Preview controls" className="velvet-preview-host__toolbar" role="toolbar">
            <span className="velvet-preview-host__mode" data-preview-mode-label="true">{mode === 'actual' ? 'Actual size' : 'Fit'}</span>
            {onModeChange ? <VelvetButton data-preview-action="toggle-mode" onClick={() => onModeChange(mode === 'actual' ? 'fit' : 'actual')} variant="quiet">{mode === 'actual' ? 'Fit' : 'Actual size'}</VelvetButton> : null}
            {providerActions?.map((action) => (
              <VelvetButton
                aria-pressed={action.pressed}
                data-preview-provider-action={action.id}
                disabled={action.disabled}
                key={action.id}
                onClick={() => onProviderAction?.(action.id)}
                variant="quiet"
              >
                {action.label}
              </VelvetButton>
            ))}
            {toolbar}
            {onOpenExternal ? <VelvetButton data-preview-action="open-external" onClick={onOpenExternal} variant="quiet">Open externally</VelvetButton> : null}
          </div>
        ) : null}
        {children}
      </section>
    );
  }
  const kind: VelvetStateKind = state === 'loading' ? 'loading' : state === 'error' ? 'error' : 'empty';
  return (
    <section aria-busy={state === 'loading' ? 'true' : undefined} aria-label="Preview" className="velvet-preview-host" data-preview-mode={mode} data-preview-state={state} role="region">
      <VelvetState
        description={message}
        kind={kind}
        onRetry={state === 'error' ? onRetry : undefined}
        title={state === 'disabled' ? 'Preview unavailable' : state === 'loading' ? 'Loading preview' : state === 'error' ? 'Preview failed' : 'No preview'}
      />
    </section>
  );
}
