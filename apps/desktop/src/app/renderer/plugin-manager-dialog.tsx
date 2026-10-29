import { useCallback, useEffect, useState, type FormEvent } from 'react';
import type { CorePluginSummary } from '../../platform/protocol.js';
import type { PluginClient } from '../../workbench/plugins/plugin-client.js';
import { VelvetDialog } from '../ui/velvet-ui.js';

export interface PluginManagerDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly plugins: PluginClient;
}

/** Provides the host-owned list and local package installation entry point. */
export function PluginManagerDialog({
  open,
  onOpenChange,
  plugins,
}: PluginManagerDialogProps) {
  const [extensions, setExtensions] = useState<readonly CorePluginSummary[]>([]);
  const [sourceDir, setSourceDir] = useState('');
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState(false);
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

    setInstalling(true);
    setError(undefined);
    setMessage(undefined);
    try {
      const summary = await plugins.installInCore(directory);
      setSourceDir('');
      setMessage(`${summary.id} installed and activated.`);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setInstalling(false);
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
              <div>
                <strong>{extension.id}</strong>
                <span>v{extension.version}</span>
              </div>
              <span
                className={`velvet-plugin-manager__status velvet-plugin-manager__status--${extension.state}`}
              >
                {extension.state}
              </span>
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
              onChange={(event) => setSourceDir(event.target.value)}
              placeholder="/path/to/plugin-package"
              value={sourceDir}
            />
            <button className="velvet-button" disabled={installing} type="submit">
              {installing ? 'Installing…' : 'Install and activate'}
            </button>
          </div>
        </form>
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

