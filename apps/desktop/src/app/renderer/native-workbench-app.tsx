import { useEffect, useState } from 'react';
import { ask } from '@tauri-apps/plugin-dialog';
import { createNativeTauriDesktopApp, type DesktopApp } from '../desktop-app.js';
import type { LayoutCommand, LayoutSnapshot, WorkbenchAppId } from '../../platform/protocol.js';
import { WorkbenchShell } from './workbench-shell.js';
import { ViewRendererRegistry } from './view-renderer-registry.js';
import { createPluginViewRenderer } from './plugin-view-renderer.js';
import { PluginOverlayHost } from './plugin-overlay-host.js';
import { WorkbenchAppSurfaceHost } from './workbench-app-surface-host.js';
import { PluginManagerDialog } from './plugin-manager-dialog.js';
import { PluginRecoveryOperationsPanel } from './plugin-recovery-operations-panel.js';
import { JobOperationsPanel } from './job-operations-panel.js';
import { createNativeDirectoryPicker } from '../../platform/tauri-directory-picker.js';
import { createNativeFileExporter } from '../../platform/tauri-file-exporter.js';
import { PreviewOverlay } from './preview-overlay.js';

interface NativeWorkbenchState {
  readonly app: DesktopApp;
  readonly layout: LayoutSnapshot;
}

/** Mounts the renderer against the real Rust-owned Tauri workbench. */
export function NativeWorkbenchApp() {
  const [state, setState] = useState<NativeWorkbenchState>();
  const [startupError, setStartupError] = useState<unknown>();
  const [commandError, setCommandError] = useState<unknown>();
  const [pluginManagerOpen, setPluginManagerOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [appRevision, setAppRevision] = useState(0);
  const [viewRenderers] = useState(() => new ViewRendererRegistry());
  const [directoryPicker] = useState(() => createNativeDirectoryPicker());
  const [fileExporter] = useState(() => createNativeFileExporter());

  useEffect(() => {
    const app = createNativeTauriDesktopApp();
    const unsubscribe = app.workbench.layout.subscribe((layout) => {
      setState((current) => (current ? { ...current, layout } : current));
    });
    let disposed = false;

    void app.start()
      .then((layout) => {
        if (!disposed) {
          app.workbench.context.apply({ kind: 'activate-app', appId: layout.activeApp });
          app.workbench.focus.focus({ kind: 'part', part: 'workspace-area' });
          setState({ app, layout });
          void app.workbench.plugins
            .queryCoreExtensions()
            .then(async (extensions) => {
              const recoveries = await app.workbench.plugins.queryCoreInstallRecoveries();
              if (recoveries.length > 0) {
                setPluginManagerOpen(true);
              }
              const activeExtensions = extensions.filter((extension) => extension.state === 'active');
              const results = await Promise.allSettled(
                activeExtensions.map(async (extension) => {
                  const projection = await app.workbench.plugins.queryCoreProjection(extension.id);
                  await app.workbench.plugins.applyCoreProjection(projection);
                }),
              );
              for (const result of results) {
                if (result.status === 'rejected') {
                  console.error('Velvet could not project a native plugin contribution', result.reason);
                }
              }
            })
            .catch((reason: unknown) => {
              // Plugin inventory is recoverable and must not prevent the shell from starting.
              console.error('Velvet could not query native plugin inventory', reason);
            });
        }
      })
      .catch((reason: unknown) => {
        if (!disposed) {
          setStartupError(reason);
        }
      });

    return () => {
      disposed = true;
      unsubscribe();
      void app.dispose();
    };
  }, []);

  useEffect(() => {
    const workbench = state?.app.workbench;
    const pluginClient = workbench?.plugins;
    if (!workbench || !pluginClient) {
      return undefined;
    }

    const registeredPluginViews = new Set<string>();
    const syncPluginViews = () => {
      const bindings = pluginClient.listViewSurfaceBindings();
      const activeViewIds = new Set(bindings.map((binding) => binding.viewId));
      for (const viewId of registeredPluginViews) {
        if (!activeViewIds.has(viewId)) {
          viewRenderers.unregister(viewId);
          registeredPluginViews.delete(viewId);
        }
      }

      for (const binding of bindings) {
        if (viewRenderers.get(binding.viewId)) {
          continue;
        }
        viewRenderers.register(
          binding.viewId,
          createPluginViewRenderer(
            binding,
            pluginClient.ui,
            workbench.context,
          ),
        );
        registeredPluginViews.add(binding.viewId);
      }
    };

    syncPluginViews();
    const unsubscribe = pluginClient.subscribe(syncPluginViews);
    return () => {
      unsubscribe();
      for (const viewId of registeredPluginViews) {
        viewRenderers.unregister(viewId);
      }
    };
  }, [state?.app, viewRenderers]);

  useEffect(() => {
    const workbench = state?.app.workbench;
    if (!workbench) {
      return undefined;
    }
    const recoverMissingActiveApp = () => {
      const activeAppId = workbench.layout.snapshot()?.activeApp;
      if (activeAppId && !workbench.apps.get(activeAppId)) {
        const fallback = workbench.apps.list()[0];
        if (fallback) {
          void workbench.layout.activateApp(fallback.id)
            .then(() => workbench.context.apply({ kind: 'activate-app', appId: fallback.id }))
            .catch(() => undefined);
        }
      }
    };
    recoverMissingActiveApp();
    return workbench.apps.subscribe(() => {
      setAppRevision((revision) => revision + 1);
      recoverMissingActiveApp();
    });
  }, [state?.app]);

  if (startupError) {
    return (
      <main className="velvet-empty-state">
        <div>
          <p>Velvet could not start the native workbench.</p>
          <pre>{startupError instanceof Error ? startupError.message : String(startupError)}</pre>
        </div>
      </main>
    );
  }

  if (!state) {
    return (
      <main className="velvet-empty-state">
        <div>Starting Velvet…</div>
      </main>
    );
  }

  const currentState = state;

  function reportCommandError(reason: unknown): void {
    setCommandError(reason);
    void currentState.app.workbench.layout.load().catch((reloadReason: unknown) => {
      setCommandError(reloadReason);
    });
  }

  function activateApp(appId: WorkbenchAppId): void {
    setCommandError(undefined);
    void currentState.app.workbench.layout
      .activateApp(appId)
      .then(() => {
        currentState.app.workbench.context.apply({ kind: 'activate-app', appId });
      })
      .catch(reportCommandError);
  }

  function dispatchLayout(command: LayoutCommand): void {
    setCommandError(undefined);
    void currentState.app.workbench.layout.apply(command).catch(reportCommandError);
  }

  return (
    <WorkbenchShell
      apps={currentState.app.workbench.apps.list()}
      commandRegistry={currentState.app.workbench.commands}
      contextKeys={currentState.app.workbench.contextKeys}
      focusService={currentState.app.workbench.focus}
      layout={currentState.layout}
      menuRegistry={currentState.app.workbench.menus}
      onActivateApp={activateApp}
      onOpenPreview={() => setPreviewOpen(true)}
      onLayoutCommand={dispatchLayout}
      operationsContent={(
        <div className="velvet-operations-stack">
          <JobOperationsPanel jobs={currentState.app.workbench.jobs} />
          <PluginRecoveryOperationsPanel
            exportLogs={async () => {
              const destination = await fileExporter.pickExportPath();
              if (!destination) {
                return undefined;
              }
              await currentState.app.workbench.plugins.exportCoreInstallRecoveryLog(destination);
              return destination;
            }}
            plugins={currentState.app.workbench.plugins}
          />
        </div>
      )}
      titleActions={
        <button
          aria-label="Open Extensions"
          className="velvet-title__action"
          onClick={() => setPluginManagerOpen(true)}
          type="button"
        >
          Extensions
        </button>
      }
      viewRegistry={currentState.app.workbench.views}
      viewRenderers={viewRenderers}
    >
      <div className="velvet-native-workspace">
        <PluginManagerDialog
          confirmUninstall={async (extensionId) => {
            return ask(`Remove ${extensionId} and its installed package?`, {
              title: 'Uninstall extension',
              kind: 'warning',
            });
          }}
          onOpenChange={setPluginManagerOpen}
          open={pluginManagerOpen}
          pickDirectory={() => directoryPicker.pickDirectory()}
          plugins={currentState.app.workbench.plugins}
        />
        <PreviewOverlay
          context={{
            assetId: 'asset-1',
            contentKind: 'generic',
            intent: 'quick',
            surface: 'workspace',
          }}
          host={currentState.app.workbench.preview}
          commands={currentState.app.workbench.commands}
          contextKeys={currentState.app.workbench.contextKeys}
          jobs={currentState.app.workbench.jobs}
          onOpenChange={setPreviewOpen}
          open={previewOpen}
          registry={currentState.app.workbench.previewRegistry}
          rendererRegistry={currentState.app.workbench.previewRenderers}
          workspaceContext={currentState.app.workbench.context}
        />
        {commandError ? (
          <div aria-live="polite" className="velvet-command-error" role="alert">
            <span>
              Layout command failed:{' '}
              {commandError instanceof Error ? commandError.message : String(commandError)}
            </span>
            <button
              className="velvet-button velvet-button--quiet"
              onClick={() => setCommandError(undefined)}
              type="button"
            >
              Dismiss
            </button>
          </div>
        ) : null}
        <div data-app-revision={appRevision}>
          <WorkbenchAppSurfaceHost
            appId={currentState.layout.activeApp}
            apps={currentState.app.workbench.apps}
            context={currentState.app.workbench.context}
            dispatch={(command) => currentState.app.workbench.context.apply(command)}
            fallback={
              <div className="velvet-empty-state">
                <div>
                  <p>{currentState.layout.activeApp} workspace</p>
                  <p>Rust-owned layout and native keybindings are active.</p>
                </div>
              </div>
            }
            plugins={currentState.app.workbench.plugins}
          />
        </div>
        <PluginOverlayHost
          context={currentState.app.workbench.context}
          overlays={currentState.app.workbench.overlays}
          plugins={currentState.app.workbench.plugins}
        />
      </div>
    </WorkbenchShell>
  );
}
