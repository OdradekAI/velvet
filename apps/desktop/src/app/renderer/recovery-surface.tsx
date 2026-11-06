import { Component, type ErrorInfo, type ReactNode } from 'react';
import type {
  CoreRecoveryAction,
  CoreRecoveryProjection,
} from '../../platform/protocol.js';
import { VelvetButton, VelvetText } from '../ui/velvet-ui.js';

const ACTION_LABELS: Record<CoreRecoveryAction, string> = {
  retry: 'Retry startup',
  'disable-recent-component': 'Disable recent component',
  'restore-snapshot': 'Restore validated snapshot',
  'quarantine-state': 'Quarantine failed state',
  'reset-optional-state': 'Reset optional state',
  'export-diagnostics': 'Export diagnostics',
};

export interface RecoverySurfaceProps {
  readonly projection?: CoreRecoveryProjection;
  readonly error?: unknown;
  readonly onRetry: () => void;
  readonly onExportDiagnostics?: () => void | Promise<void>;
  readonly onRestoreSnapshot?: () => void | Promise<void>;
  readonly onQuarantineState?: () => void | Promise<void>;
}

/**
 * Host-owned recovery surface. It only renders a recovery projection and
 * delegates typed retry, restore, quarantine and diagnostic actions to the
 * composition root. Unsupported actions stay visible but disabled until their
 * owner-specific command exists.
 */
export function RecoverySurface({
  projection,
  error,
  onRetry,
  onExportDiagnostics,
  onRestoreSnapshot,
  onQuarantineState,
}: RecoverySurfaceProps) {
  const actions = projection?.actions.length ? projection.actions : ['retry'] as const;
  const failureMessage = projection?.lastFailure?.message;
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : undefined;

  return (
    <main
      aria-labelledby="velvet-recovery-title"
      className="velvet-recovery-surface"
      data-recovery-mode={projection?.mode ?? 'starting'}
    >
      <section className="velvet-recovery-card">
        <VelvetText as="p" className="velvet-recovery-eyebrow" tone="muted">Velvet recovery</VelvetText>
        <VelvetText as="h1" id="velvet-recovery-title">Velvet could not mount the workbench</VelvetText>
        <VelvetText ariaLive="polite" ariaLabel="Recovery status" as="p" className="velvet-recovery-status" role="status" tone="secondary">
          {projection?.mode === 'safe'
            ? 'Velvet is running in Safe Mode. Core data was left unchanged.'
            : 'The host recovery path is available while optional components remain isolated.'}
        </VelvetText>
        {failureMessage ? <VelvetText as="p" className="velvet-recovery-detail" tone="danger">{failureMessage}</VelvetText> : null}
        {errorMessage ? <VelvetText as="p" className="velvet-recovery-detail" tone="danger">{errorMessage}</VelvetText> : null}
        <dl className="velvet-recovery-meta">
          <div>
            <dt>Mode</dt>
            <dd>{projection?.mode ?? 'starting'}</dd>
          </div>
          <div>
            <dt>Phase</dt>
            <dd>{projection?.phase ?? 'preflight'}</dd>
          </div>
          <div>
            <dt>Attempt</dt>
            <dd>{projection?.attemptId ?? 'unknown'}</dd>
          </div>
        </dl>
        <div aria-label="Recovery actions" className="velvet-recovery-actions" role="group">
          {actions.map((action) => {
            const enabled = action === 'retry' || (
              action === 'export-diagnostics' && onExportDiagnostics !== undefined
            ) || (
              action === 'restore-snapshot' && onRestoreSnapshot !== undefined
            ) || (
              action === 'quarantine-state' && onQuarantineState !== undefined
            );
            let onClick: (() => void | Promise<void>) | undefined;
            switch (action) {
              case 'retry':
                onClick = onRetry;
                break;
              case 'export-diagnostics':
                onClick = onExportDiagnostics;
                break;
              case 'restore-snapshot':
                onClick = onRestoreSnapshot;
                break;
              case 'quarantine-state':
                onClick = onQuarantineState;
                break;
              default:
                onClick = undefined;
            }
            return (
              <VelvetButton
                aria-disabled={!enabled}
                data-recovery-action={action}
                disabled={!enabled}
                key={action}
                onClick={enabled && onClick ? () => { void onClick(); } : undefined}
                variant={enabled ? 'default' : 'quiet'}>
                {ACTION_LABELS[action]}
              </VelvetButton>
            );
          })}
        </div>
        <VelvetText as="p" className="velvet-recovery-note" tone="muted">
          Recovery actions do not run plugin code and do not reset core data automatically.
        </VelvetText>
      </section>
    </main>
  );
}

interface RecoveryBoundaryProps {
  readonly projection?: CoreRecoveryProjection;
  readonly onRetry: () => void;
  readonly onExportDiagnostics?: () => void | Promise<void>;
  readonly onRestoreSnapshot?: () => void | Promise<void>;
  readonly onQuarantineState?: () => void | Promise<void>;
  readonly children: ReactNode;
}

interface RecoveryBoundaryState {
  readonly error?: unknown;
}

/** Keeps a renderer mount failure inside the host recovery boundary. */
export class RecoveryBoundary extends Component<RecoveryBoundaryProps, RecoveryBoundaryState> {
  public state: RecoveryBoundaryState = {};

  public static getDerivedStateFromError(error: unknown): RecoveryBoundaryState {
    return { error };
  }

  public componentDidCatch(error: unknown, info: ErrorInfo): void {
    console.error('Velvet workbench mount failed', error, info.componentStack);
  }

  public render(): ReactNode {
    if (this.state.error) {
      return (
        <RecoverySurface
          error={this.state.error}
          onExportDiagnostics={this.props.onExportDiagnostics}
          onQuarantineState={this.props.onQuarantineState}
          onRetry={this.props.onRetry}
          onRestoreSnapshot={this.props.onRestoreSnapshot}
          projection={this.props.projection}
        />
      );
    }
    return this.props.children;
  }
}
