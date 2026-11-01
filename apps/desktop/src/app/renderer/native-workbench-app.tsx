import { useEffect, useState } from 'react';
import { ask } from '@tauri-apps/plugin-dialog';
import { createNativeTauriDesktopApp, type DesktopApp } from '../desktop-app.js';
import { createNativeSourceRootPicker } from '../../platform/tauri-source-root-picker.js';
import type {
  CoreTransportStatus,
  LayoutCommand,
  LayoutSnapshot,
  WorkbenchAppId,
} from '../../platform/protocol.js';
import { WorkbenchShell } from './workbench-shell.js';
import { ViewRendererRegistry } from './view-renderer-registry.js';
import { createPluginViewRenderer } from './plugin-view-renderer.js';
import { PluginOverlayHost } from './plugin-overlay-host.js';
import { WorkbenchAppSurfaceHost } from './workbench-app-surface-host.js';
import { PluginManagerDialog } from './plugin-manager-dialog.js';
import { PluginRecoveryOperationsPanel } from './plugin-recovery-operations-panel.js';
import { JobOperationsPanel } from './job-operations-panel.js';
import { createNativePluginPackagePicker } from '../../platform/tauri-plugin-package-picker.js';
import { createNativeFileExporter } from '../../platform/tauri-file-exporter.js';
import { PreviewOverlay } from './preview-overlay.js';
import { RecoveryBoundary, RecoverySurface } from './recovery-surface.js';
import { SourceRootDialog } from './source-root-dialog.js';
import type { CoreRecoveryProjection } from '../../platform/protocol.js';
import type { WorkspaceRoutingIntent } from '../../workbench/activation-routing.js';

interface NativeWorkbenchState {
  readonly app: DesktopApp;
  readonly layout?: LayoutSnapshot;
  readonly transportStatus: CoreTransportStatus;
}

/** Mounts the renderer against the real Rust-owned Tauri workbench. */
export function NativeWorkbenchApp() {
  const [state, setState] = useState<NativeWorkbenchState>();
  const [recoveryProjection, setRecoveryProjection] = useState<CoreRecoveryProjection>();
  const [startupError, setStartupError] = useState<unknown>();
  const [startupAttempt, setStartupAttempt] = useState(0);
  const [commandError, setCommandError] = useState<unknown>();
  const [pluginManagerOpen, setPluginManagerOpen] = useState(false);
  const [sourceRootDialogOpen, setSourceRootDialogOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [appRevision, setAppRevision] = useState(0);
  const [viewRenderers] = useState(() => new ViewRendererRegistry());
  const [packagePicker] = useState(() => createNativePluginPackagePicker());
  const [fileExporter] = useState(() => createNativeFileExporter());
  const [sourceRootPicker] = useState(() => createNativeSourceRootPicker());

  useEffect(() => {
    let app: DesktopApp;
    try {
      app = createNativeTauriDesktopApp();
    } catch (error) {
      setStartupError(error);
      return undefined;
    }
    setState({ app, transportStatus: app.workbench.transportStatus });
    const unsubscribeTransport = app.workbench.subscribeTransportStatus((transportStatus) => {
      setState((current) => (current?.app === app ? { ...current, transportStatus } : current));
    });
    setRecoveryProjection(app.workbench.recovery.snapshot());
    const unsubscribeRecovery = app.workbench.recovery.subscribe(setRecoveryProjection);
    const unsubscribe = app.workbench.layout.subscribe((layout) => {
      setState((current) => (current?.app === app ? { ...current, layout } : current));
    });
    let disposed = false;

    void app.start()
      .then((layout) => {
        if (!disposed) {
          app.workbench.context.apply({ kind: 'activate-app', appId: layout.activeApp });
          app.workbench.focus.focus({ kind: 'part', part: 'workspace-area' });
          setState((current) => ({
            app,
            layout,
            transportStatus: current?.app === app
              ? current.transportStatus
              : app.workbench.transportStatus,
          }));
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
      unsubscribeRecovery();
      unsubscribeTransport();
      void app.dispose();
    };
  }, [startupAttempt]);

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

    const consumeAssetIntent = (intent: WorkspaceRoutingIntent) => {
      if (intent.kind !== 'open-asset' && intent.kind !== 'open-file') {
        return;
      }
      void workbench.consumeWorkspaceRoutingIntent(intent, (consumed) => {
        if (consumed.kind === 'open-asset') {
          workbench.context.apply({
            kind: 'set-selection',
            selection: [{ kind: 'asset', id: consumed.assetId }],
          });
          return;
        }
        return workbench.registerFileActivation(consumed).then((assetId) => {
          workbench.context.apply({
            kind: 'set-selection',
            selection: [{ kind: 'asset', id: assetId }],
          });
        });
      }).catch(() => undefined);
    };

    const unsubscribeWorkspaceRouting = workbench.subscribeWorkspaceRouting(consumeAssetIntent);
    for (const intent of workbench.workspaceRoutingIntents()) {
      consumeAssetIntent(intent);
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
    const unsubscribeApps = workbench.apps.subscribe(() => {
      setAppRevision((revision) => revision + 1);
      recoverMissingActiveApp();
    });
    return () => {
      unsubscribeWorkspaceRouting();
      unsubscribeApps();
    };
  }, [state?.app, state?.layout]);

  function retryStartup(): void {
    setStartupError(undefined);
    setRecoveryProjection(undefined);
    setState(undefined);
    setStartupAttempt((attempt) => attempt + 1);
  }

  function exportRecoveryDiagnostics(): void {
    const app = state?.app;
    if (!app) {
      return;
    }
    void (async () => {
      const destination = await fileExporter.pickExportPath('velvet-recovery-diagnostics.json');
      if (!destination) {
        return;
      }
      await app.workbench.recovery.exportDiagnostics(destination);
    })().catch((error: unknown) => {
      setStartupError(error);
    });
  }

  async function restoreRecoverySnapshot(): Promise<void> {
    const app = state?.app;
    if (!app) {
      return;
    }
    try {
      await app.workbench.recovery.restoreRecoverySet();
      retryStartup();
    } catch (error: unknown) {
      setStartupError(error);
    }
  }

  async function quarantineRecoveryFailure(): Promise<void> {
    const app = state?.app;
    if (!app) {
      return;
    }
    try {
      await app.workbench.recovery.quarantineFailure();
      retryStartup();
    } catch (error: unknown) {
      setStartupError(error);
    }
  }

  if (startupError) {
    return (
      <RecoverySurface
        error={startupError}
        onExportDiagnostics={state ? exportRecoveryDiagnostics : undefined}
        onQuarantineState={state ? quarantineRecoveryFailure : undefined}
        onRetry={retryStartup}
        onRestoreSnapshot={state ? restoreRecoverySnapshot : undefined}
        projection={recoveryProjection}
      />
    );
  }

  if (!state || !state.layout) {
    return (
      <main className="velvet-empty-state">
        <div>Starting Velvet…</div>
      </main>
    );
  }

  const currentState = state;
  const currentLayout = state.layout;
  const readOnly = currentState.transportStatus.state !== 'connected';

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
    <RecoveryBoundary
      key={startupAttempt}
      onExportDiagnostics={state ? exportRecoveryDiagnostics : undefined}
      onQuarantineState={state ? quarantineRecoveryFailure : undefined}
      onRetry={retryStartup}
      onRestoreSnapshot={state ? restoreRecoverySnapshot : undefined}
      projection={recoveryProjection}
    >
      <WorkbenchShell
        apps={currentState.app.workbench.apps.list()}
        commandRegistry={currentState.app.workbench.commands}
        contextKeys={currentState.app.workbench.contextKeys}
        focusService={currentState.app.workbench.focus}
        layout={currentLayout}
        readOnly={readOnly}
        menuRegistry={currentState.app.workbench.menus}
        onActivateApp={readOnly ? () => undefined : activateApp}
        onOpenPreview={readOnly ? undefined : () => setPreviewOpen(true)}
        onLayoutCommand={readOnly ? undefined : dispatchLayout}
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
          <>
            <button
              aria-label="Open Source Folders"
              className="velvet-title__action"
              onClick={() => setSourceRootDialogOpen(true)}
              type="button"
            >
              Source folders
            </button>
            <button
              aria-label="Open Extensions"
              className="velvet-title__action"
              onClick={() => setPluginManagerOpen(true)}
              type="button"
            >
              Extensions
            </button>
          </>
        }
        viewRegistry={currentState.app.workbench.views}
        viewRenderers={viewRenderers}
      >
      <div className="velvet-native-workspace">
        {readOnly ? (
          <div
            aria-live="polite"
            className="velvet-command-error"
            data-transport-recovery-state={currentState.transportStatus.state}
            role="status"
          >
            Native core {currentState.transportStatus.state}; the workbench is read-only until it reconnects.
          </div>
        ) : null}
        <PluginManagerDialog
          confirmUninstall={async (extensionId) => {
            return ask(`Remove ${extensionId} and its installed package?`, {
              title: 'Uninstall extension',
              kind: 'warning',
            });
          }}
          onOpenChange={setPluginManagerOpen}
          open={pluginManagerOpen}
          pickPackage={(selection) => packagePicker.pick(selection)}
          plugins={currentState.app.workbench.plugins}
        />
        <SourceRootDialog
          confirmRevoke={(root) => ask(`Revoke ${root}? Existing Asset identity will be kept, but reads will pause.`, {
            title: 'Revoke source folder',
            kind: 'warning',
          })}
          onOpenChange={setSourceRootDialogOpen}
          open={sourceRootDialogOpen}
          pickDirectory={() => sourceRootPicker.pick()}
          readOnly={readOnly}
          sourceRoots={currentState.app.workbench.sourceRoots}
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
            appId={currentLayout.activeApp}
            apps={currentState.app.workbench.apps}
            context={currentState.app.workbench.context}
            dispatch={(command) => currentState.app.workbench.context.apply(command)}
            fallback={
              <div className="velvet-empty-state">
                <div>
                  <p>{currentLayout.activeApp} workspace</p>
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
    </RecoveryBoundary>
  );
}
