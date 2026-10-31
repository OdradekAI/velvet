import { Component, type ErrorInfo, type ReactNode } from 'react';
import type {
  CoreRecoveryAction,
  CoreRecoveryProjection,
} from '../../platform/protocol.js';

const ACTION_LABELS: Record<CoreRecoveryAction, string> = {
  retry: 'Retry startup',
  'disable-recent-component': 'Disable recent component',
  'restore-snapshot': 'Restore validated snapshot',
  'reset-optional-state': 'Reset optional state',
  'export-diagnostics': 'Export diagnostics',
};

export interface RecoverySurfaceProps {
  readonly projection?: CoreRecoveryProjection;
  readonly error?: unknown;
  readonly onRetry: () => void;
  readonly onExportDiagnostics?: () => void | Promise<void>;
}

/**
 * Host-owned recovery surface. It only renders a recovery projection and
 * delegates supported retry and diagnostic export actions to the composition
 * root. Unsupported destructive actions stay visible but disabled until their
 * owner-specific command exists.
 */
export function RecoverySurface({
  projection,
  error,
  onRetry,
  onExportDiagnostics,
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
        <p className="velvet-recovery-eyebrow">Velvet recovery</p>
        <h1 id="velvet-recovery-title">Velvet could not mount the workbench</h1>
        <p aria-live="polite" className="velvet-recovery-status" role="status">
          {projection?.mode === 'safe'
            ? 'Velvet is running in Safe Mode. Core data was left unchanged.'
            : 'The host recovery path is available while optional components remain isolated.'}
        </p>
        {failureMessage ? <p className="velvet-recovery-detail">{failureMessage}</p> : null}
        {errorMessage ? <p className="velvet-recovery-detail">{errorMessage}</p> : null}
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
            );
            const onClick = action === 'retry'
              ? onRetry
              : action === 'export-diagnostics'
                ? onExportDiagnostics
                : undefined;
            return (
              <button
                aria-disabled={!enabled}
                className={`velvet-button${enabled ? '' : ' velvet-button--quiet'}`}
                data-recovery-action={action}
                disabled={!enabled}
                key={action}
                onClick={enabled && onClick ? () => { void onClick(); } : undefined}
                type="button"
              >
                {ACTION_LABELS[action]}
              </button>
            );
          })}
        </div>
        <p className="velvet-recovery-note">
          Recovery actions do not run plugin code and do not reset core data automatically.
        </p>
      </section>
    </main>
  );
}

interface RecoveryBoundaryProps {
  readonly projection?: CoreRecoveryProjection;
  readonly onRetry: () => void;
  readonly onExportDiagnostics?: () => void | Promise<void>;
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
          onRetry={this.props.onRetry}
          projection={this.props.projection}
        />
      );
    }
    return this.props.children;
  }
}
