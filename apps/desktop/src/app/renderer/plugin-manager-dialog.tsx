import { useCallback, useEffect, useState, type FormEvent } from 'react';
import type {
  CorePluginInstallRecovery,
  CorePluginInstallRecoveryAction,
  CorePluginInstallRecoveryLogEntry,
  CorePluginInstallPreview,
  CorePluginSummary,
} from '../../platform/protocol.js';
import type { PluginClient } from '../../workbench/plugins/plugin-client.js';
import { VelvetDialog } from '../ui/velvet-ui.js';

export interface PluginManagerDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly pickDirectory?: () => Promise<string | undefined>;
  readonly confirmUninstall?: (extensionId: string) => Promise<boolean>;
  readonly plugins: PluginClient;
}

/** Provides the host-owned list and local package installation entry point. */
export function PluginManagerDialog({
  open,
  onOpenChange,
  pickDirectory,
  confirmUninstall,
  plugins,
}: PluginManagerDialogProps) {
  const [extensions, setExtensions] = useState<readonly CorePluginSummary[]>([]);
  const [recoveries, setRecoveries] = useState<readonly CorePluginInstallRecovery[]>([]);
  const [recoveryLogs, setRecoveryLogs] = useState<readonly CorePluginInstallRecoveryLogEntry[]>([]);
  const [sourceDir, setSourceDir] = useState('');
  const [preview, setPreview] = useState<CorePluginInstallPreview>();
  const [previewSource, setPreviewSource] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [busyExtensionId, setBusyExtensionId] = useState<string>();
  const [message, setMessage] = useState<string>();
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(undefined);
    try {
      const nextExtensions = await plugins.queryCoreExtensions();
      const nextRecoveries = await plugins.queryCoreInstallRecoveries();
      const nextRecoveryLogs = await plugins.queryCoreInstallRecoveryLog();
      setExtensions(nextExtensions);
      setRecoveries(nextRecoveries);
      setRecoveryLogs(nextRecoveryLogs);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setError(message);
      try {
        setRecoveryLogs(await plugins.queryCoreInstallRecoveryLog());
      } catch {
        // The action error remains visible even if the log projection is unavailable.
      }
    } finally {
      setLoading(false);
    }
  }, [plugins]);

  useEffect(() => {
    if (!open) {
      return undefined;
    }

    void refresh();
    return plugins.subscribe(() => {
      setExtensions((current) => current.map((extension) =>
        plugins.getCoreSummary(extension.id) ?? extension,
      ));
    });
  }, [open, plugins, refresh]);

  const install = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const sourcePath = sourceDir.trim();
    if (!sourcePath) {
      setError('Enter the plugin package path.');
      return;
    }

    setError(undefined);
    setMessage(undefined);
    if (!preview || previewSource !== sourcePath) {
      await inspectPackage(sourcePath);
      return;
    }
    if (!preview.canInstall) {
      setError(preview.issues.join(' '));
      return;
    }

    setInstalling(true);
    try {
      const summary = isArchivePackage(sourcePath)
        ? await plugins.installArchiveInCore(sourcePath)
        : await plugins.installInCore(sourcePath);
      setSourceDir('');
      setPreview(undefined);
      setPreviewSource(undefined);
      setMessage(`${summary.id} installed and activated.`);
      await refresh();
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setError(message);
      try {
        setRecoveryLogs(await plugins.queryCoreInstallRecoveryLog());
      } catch {
        // The action error remains visible even if the log projection is unavailable.
      }
    } finally {
      setInstalling(false);
    }
  };

  const inspectPackage = async (sourcePath: string) => {
    setError(undefined);
    setMessage(undefined);
    try {
      const nextPreview = isArchivePackage(sourcePath)
        ? await plugins.inspectArchiveInCore(sourcePath)
        : await plugins.inspectInCore(sourcePath);
      setPreview(nextPreview);
      setPreviewSource(sourcePath);
    } catch (reason) {
      setPreview(undefined);
      setPreviewSource(undefined);
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const chooseDirectory = async () => {
    if (!pickDirectory) {
      return;
    }
    setError(undefined);
    try {
      const directory = await pickDirectory();
      if (directory) {
        setSourceDir(directory);
        await inspectPackage(directory);
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const enable = async (extension: CorePluginSummary) => {
    setBusyExtensionId(extension.id);
    setError(undefined);
    setMessage(undefined);
    try {
      const summary = extension.state === 'failed'
        ? await plugins.recoverInCore(extension.id)
        : await plugins.activateInCore(extension.id);
      setMessage(`${summary.id} ${summary.state}.`);
      await refresh();
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setError(message);
      try {
        setRecoveryLogs(await plugins.queryCoreInstallRecoveryLog());
      } catch {
        // The action error remains visible even if the log projection is unavailable.
      }
    } finally {
      setBusyExtensionId(undefined);
    }
  };

  const disable = async (extension: CorePluginSummary) => {
    setBusyExtensionId(extension.id);
    setError(undefined);
    setMessage(undefined);
    try {
      const summary = await plugins.disableInCore(extension.id);
      setMessage(`${summary.id} ${summary.state}.`);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusyExtensionId(undefined);
    }
  };

  const uninstall = async (extension: CorePluginSummary) => {
    if (confirmUninstall) {
      try {
        if (!(await confirmUninstall(extension.id))) {
          return;
        }
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
        return;
      }
    }
    setBusyExtensionId(extension.id);
    setError(undefined);
    setMessage(undefined);
    try {
      await plugins.uninstallInCore(extension.id);
      setMessage(`${extension.id} uninstalled.`);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusyExtensionId(undefined);
    }
  };

  const resolveRecovery = async (
    recovery: CorePluginInstallRecovery,
    action: CorePluginInstallRecoveryAction,
  ) => {
    setBusyExtensionId(recovery.extensionId);
    setError(undefined);
    setMessage(undefined);
    try {
      const summary = await plugins.resolveCoreInstallRecovery(recovery.extensionId, action);
      if (summary && summary.state !== 'active') {
        const activated = await plugins.activateInCore(recovery.extensionId);
        setMessage(`${activated.id} recovery resolved and activated.`);
      } else if (summary) {
        setMessage(`${summary.id} recovery resolved.`);
      } else {
        setMessage(`${recovery.extensionId} recovery discarded.`);
      }
      await refresh();
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setError(message);
      try {
        setRecoveryLogs(await plugins.queryCoreInstallRecoveryLog());
      } catch {
        // The action error remains visible even if the log projection is unavailable.
      }
    } finally {
      setBusyExtensionId(undefined);
    }
  };

  return (
    <VelvetDialog
      description="Install a local extension package and inspect its host-owned runtime state."
      onOpenChange={onOpenChange}
      open={open}
      title="Extensions"
    >
      <div className="velvet-plugin-manager" data-plugin-manager>
        <div className="velvet-plugin-manager__toolbar">
          <span className="velvet-plugin-manager__eyebrow">Installed extensions</span>
          <button
            className="velvet-button velvet-button--quiet"
            disabled={loading || installing}
            onClick={() => void refresh()}
            type="button"
          >
            {loading ? 'Refreshing…' : 'Refresh'}
          </button>
        </div>
        <div aria-live="polite" className="velvet-plugin-manager__list">
          {extensions.length === 0 && !loading ? (
            <p className="velvet-view-placeholder">No extensions are installed.</p>
          ) : null}
          {extensions.map((extension) => (
            <div className="velvet-plugin-manager__item" key={extension.id}>
              <div className="velvet-plugin-manager__identity">
                <strong>{extension.id}</strong>
                <span>v{extension.version}</span>
              </div>
              <div className="velvet-plugin-manager__controls">
                <span
                  className={`velvet-plugin-manager__status velvet-plugin-manager__status--${extension.state}`}
                >
                  {extension.state}
                </span>
                {extension.state === 'active' ? (
                  <button
                    aria-label={`Disable ${extension.id}`}
                    className="velvet-button velvet-button--quiet"
                    disabled={busyExtensionId !== undefined || installing}
                    onClick={() => void disable(extension)}
                    type="button"
                  >
                    {busyExtensionId === extension.id ? 'Working…' : 'Disable'}
                  </button>
                ) : extension.state !== 'starting' ? (
                  <button
                    aria-label={`${extension.state === 'failed' ? 'Recover' : 'Enable'} ${extension.id}`}
                    className="velvet-button velvet-button--quiet"
                    disabled={busyExtensionId !== undefined || installing}
                    onClick={() => void enable(extension)}
                    type="button"
                  >
                    {busyExtensionId === extension.id
                      ? 'Working…'
                      : extension.state === 'failed' ? 'Recover' : 'Enable'}
                  </button>
                ) : null}
                <button
                  aria-label={`Uninstall ${extension.id}`}
                  className="velvet-button velvet-button--quiet velvet-button--danger"
                  disabled={busyExtensionId !== undefined || installing}
                  onClick={() => void uninstall(extension)}
                  type="button"
                >
                  Uninstall
                </button>
              </div>
            </div>
          ))}
        </div>
        {recoveries.length > 0 ? (
          <div className="velvet-plugin-manager__recoveries" data-plugin-install-recoveries>
            <div className="velvet-plugin-manager__toolbar">
              <span className="velvet-plugin-manager__eyebrow">Interrupted installs</span>
              <span className="velvet-plugin-manager__recovery-hint">Review before activation</span>
            </div>
            {recoveries.map((recovery) => {
              const lastFailure = [...recoveryLogs]
                .reverse()
                .find((entry) => entry.extensionId === recovery.extensionId && entry.status === 'failed');
              const canRetry = lastFailure && (
                (lastFailure.action === 'resume' && recovery.canResume)
                || (lastFailure.action === 'commit' && recovery.canCommit)
                || (lastFailure.action === 'rollback' && recovery.canRollback)
                || (lastFailure.action === 'discard' && recovery.canDiscard)
              );
              return (
                <div className="velvet-plugin-manager__recovery" key={`${recovery.extensionId}-${recovery.kind}`}>
                <div className="velvet-plugin-manager__identity">
                  <strong>{recovery.extensionId}</strong>
                  <span>
                    {recovery.kind === 'rollback' ? 'upgrade interrupted' : 'staged install'}
                    {recovery.candidateVersion ? ` · v${recovery.candidateVersion}` : ''}
                  </span>
                </div>
                {recovery.issues.length > 0 ? (
                  <p className="velvet-command-error" role="alert">{recovery.issues.join(' ')}</p>
                ) : null}
                {lastFailure ? (
                  <p className="velvet-plugin-manager__recovery-log" role="status">
                    Attempt {lastFailure.attempt} failed: {lastFailure.message}
                  </p>
                ) : null}
                <div className="velvet-plugin-manager__controls">
                  {canRetry ? (
                    <button
                      className="velvet-button velvet-button--quiet"
                      disabled={busyExtensionId !== undefined || installing}
                      onClick={() => void resolveRecovery(recovery, lastFailure.action)}
                      type="button"
                    >
                      Retry {lastFailure.action}
                    </button>
                  ) : null}
                  {recovery.canResume ? (
                    <button
                      className="velvet-button velvet-button--quiet"
                      disabled={busyExtensionId !== undefined || installing}
                      onClick={() => void resolveRecovery(recovery, 'resume')}
                      type="button"
                    >
                      Resume and activate
                    </button>
                  ) : null}
                  {recovery.canCommit ? (
                    <button
                      className="velvet-button velvet-button--quiet"
                      disabled={busyExtensionId !== undefined || installing}
                      onClick={() => void resolveRecovery(recovery, 'commit')}
                      type="button"
                    >
                      Keep new
                    </button>
                  ) : null}
                  {recovery.canRollback ? (
                    <button
                      className="velvet-button velvet-button--quiet"
                      disabled={busyExtensionId !== undefined || installing}
                      onClick={() => void resolveRecovery(recovery, 'rollback')}
                      type="button"
                    >
                      Restore previous
                    </button>
                  ) : null}
                  {recovery.canDiscard ? (
                    <button
                      className="velvet-button velvet-button--quiet velvet-button--danger"
                      disabled={busyExtensionId !== undefined || installing}
                      onClick={() => void resolveRecovery(recovery, 'discard')}
                      type="button"
                    >
                      Discard
                    </button>
                  ) : null}
                </div>
                </div>
              );
            })}
          </div>
        ) : null}
        {recoveryLogs.length > 0 ? (
          <div className="velvet-plugin-manager__recovery-history" data-plugin-install-recovery-log>
            <div className="velvet-plugin-manager__toolbar">
              <span className="velvet-plugin-manager__eyebrow">Recovery history</span>
              <span className="velvet-plugin-manager__recovery-hint">Recent host attempts</span>
            </div>
            {[...recoveryLogs].slice(-8).reverse().map((entry) => (
              <div className="velvet-plugin-manager__recovery-log" key={`${entry.extensionId}-${entry.action}-${entry.attempt}-${entry.occurredAtMs}`}>
                <strong>{entry.extensionId}</strong>
                <span>
                  {entry.action} · attempt {entry.attempt} · {entry.status}
                </span>
                <small>{entry.message}</small>
                {entry.removedPaths.length > 0 ? (
                  <small>cleaned {entry.removedPaths.join(', ')}</small>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
        <form className="velvet-plugin-manager__install" onSubmit={(event) => void install(event)}>
          <label htmlFor="velvet-plugin-source">Plugin package path</label>
          <div className="velvet-plugin-manager__install-row">
            <input
              aria-label="Plugin package path"
              className="velvet-input"
              id="velvet-plugin-source"
              onChange={(event) => {
                setSourceDir(event.target.value);
                setPreview(undefined);
                setPreviewSource(undefined);
              }}
              placeholder="/path/to/plugin-package or .zip"
              value={sourceDir}
            />
            {pickDirectory ? (
              <button
                className="velvet-button velvet-button--quiet"
                disabled={installing || busyExtensionId !== undefined}
                onClick={() => void chooseDirectory()}
                type="button"
              >
                Browse…
              </button>
            ) : null}
            <button
              className="velvet-button"
              disabled={installing || busyExtensionId !== undefined}
              type="submit"
            >
              {installing
                ? 'Installing…'
                : preview?.operation === 'upgrade' ? 'Upgrade and activate' : 'Review manifest'}
            </button>
          </div>
        </form>
        {preview ? (
          <div className="velvet-plugin-manager__preview" data-plugin-install-preview>
            <div className="velvet-plugin-manager__preview-header">
              <strong>Manifest review</strong>
              <span>{preview.operation === 'upgrade' ? 'Upgrade' : 'New install'}</span>
            </div>
            <div className="velvet-plugin-manager__preview-grid">
              <span>Extension</span><strong>{preview.manifest.id}</strong>
              <span>Version</span><strong>v{preview.manifest.version}</strong>
              <span>Runtime</span><strong>{preview.manifest.runtime}</strong>
              <span>Host API</span>
              <strong>
                {preview.manifest.hostApi} · {preview.compatible ? 'compatible' : 'incompatible'}
              </strong>
              {preview.manifest.targetApp ? (
                <><span>Target app</span><strong>{preview.manifest.targetApp.appId}</strong></>
              ) : null}
            </div>
            <div className="velvet-plugin-manager__permissions">
              <span>Requested permissions</span>
              {preview.manifest.permissionSets.length === 0 ? (
                <em>None</em>
              ) : preview.manifest.permissionSets.map((permissionSet) => (
                <div key={permissionSet.zone}>
                  <strong>{permissionSet.zone}</strong>
                  <span>{permissionSet.permissions.map(formatPermission).join(', ')}</span>
                </div>
              ))}
            </div>
            {preview.issues.length > 0 ? (
              <p className="velvet-command-error" role="alert">{preview.issues.join(' ')}</p>
            ) : null}
          </div>
        ) : null}
        {message ? <p className="velvet-plugin-manager__success">{message}</p> : null}
        {error ? (
          <p aria-live="assertive" className="velvet-command-error" role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </VelvetDialog>
  );
}

function formatPermission(permission: string): string {
  return permission.replaceAll('-', ' ').replaceAll('.', ' ');
}

function isArchivePackage(sourcePath: string): boolean {
  return sourcePath.toLowerCase().endsWith('.zip');
}

