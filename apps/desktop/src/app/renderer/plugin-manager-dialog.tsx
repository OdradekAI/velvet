import { useCallback, useEffect, useState, type FormEvent } from 'react';
import type {
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
      setExtensions(await plugins.queryCoreExtensions());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
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
    const directory = sourceDir.trim();
    if (!directory) {
      setError('Enter the plugin package directory.');
      return;
    }

    setError(undefined);
    setMessage(undefined);
    if (!preview || previewSource !== directory) {
      await inspectDirectory(directory);
      return;
    }
    if (!preview.canInstall) {
      setError(preview.issues.join(' '));
      return;
    }

    setInstalling(true);
    try {
      const summary = await plugins.installInCore(directory);
      setSourceDir('');
      setPreview(undefined);
      setPreviewSource(undefined);
      setMessage(`${summary.id} installed and activated.`);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setInstalling(false);
    }
  };

  const inspectDirectory = async (directory: string) => {
    setError(undefined);
    setMessage(undefined);
    try {
      const nextPreview = await plugins.inspectInCore(directory);
      setPreview(nextPreview);
      setPreviewSource(directory);
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
        await inspectDirectory(directory);
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
      setError(reason instanceof Error ? reason.message : String(reason));
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
        <form className="velvet-plugin-manager__install" onSubmit={(event) => void install(event)}>
          <label htmlFor="velvet-plugin-source">Plugin package directory</label>
          <div className="velvet-plugin-manager__install-row">
            <input
              aria-label="Plugin package directory"
              className="velvet-input"
              id="velvet-plugin-source"
              onChange={(event) => {
                setSourceDir(event.target.value);
                setPreview(undefined);
                setPreviewSource(undefined);
              }}
              placeholder="/path/to/plugin-package"
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

