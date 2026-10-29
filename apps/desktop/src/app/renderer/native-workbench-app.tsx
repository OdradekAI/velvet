import { useEffect, useState } from 'react';
import { createNativeTauriDesktopApp, type DesktopApp } from '../desktop-app.js';
import type { LayoutCommand, LayoutSnapshot, WorkbenchAppId } from '../../platform/protocol.js';
import { WorkbenchShell } from './workbench-shell.js';
import { ViewRendererRegistry } from './view-renderer-registry.js';
import { createPluginViewRenderer } from './plugin-view-renderer.js';
import { PluginOverlayHost } from './plugin-overlay-host.js';

interface NativeWorkbenchState {
  readonly app: DesktopApp;
  readonly layout: LayoutSnapshot;
}

/** Mounts the renderer against the real Rust-owned Tauri workbench. */
export function NativeWorkbenchApp() {
  const [state, setState] = useState<NativeWorkbenchState>();
  const [startupError, setStartupError] = useState<unknown>();
  const [commandError, setCommandError] = useState<unknown>();
  const [surfaceError, setSurfaceError] = useState<unknown>();
  const [appRevision, setAppRevision] = useState(0);
  const [viewRenderers] = useState(() => new ViewRendererRegistry());

  useEffect(() => {
    const app = createNativeTauriDesktopApp();
    const unsubscribe = app.workbench.layout.subscribe((layout) => {
      setState((current) => (current ? { ...current, layout } : current));
    });
    let disposed = false;

    void app.start()
      .then((layout) => {
        if (!disposed) {
          app.workbench.focus.focus({ kind: 'part', part: 'workspace-area' });
          setState({ app, layout });
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
    return workbench.apps.subscribe(() => {
      setAppRevision((revision) => revision + 1);
      const activeAppId = workbench.layout.snapshot()?.activeApp;
      if (activeAppId && !workbench.apps.get(activeAppId)) {
        const fallback = workbench.apps.list()[0];
        if (fallback) {
          void workbench.layout.activateApp(fallback.id)
            .then(() => workbench.context.apply({ kind: 'activate-app', appId: fallback.id }))
            .catch(() => undefined);
        }
      }
    });
  }, [state?.app]);

  useEffect(() => {
    const workbench = state?.app.workbench;
    const appId = state?.layout.activeApp;
    if (!workbench || !appId) {
      return undefined;
    }

    const descriptor = workbench.apps.get(appId);
    if (!descriptor || descriptor.surface.trust !== 'isolated') {
      setSurfaceError(undefined);
      return undefined;
    }

    let disposed = false;
    setSurfaceError(undefined);
    void workbench.apps.activateSurface(appId, {
      appId,
      context: workbench.context,
      dispatch: (command) => {
        workbench.context.apply(command);
      },
    }).catch((reason: unknown) => {
      if (!disposed) {
        setSurfaceError(reason);
      }
    });

    return () => {
      disposed = true;
      void workbench.apps.deactivateSurface(appId).catch(() => undefined);
    };
  }, [state?.app, state?.layout.activeApp]);

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
      onLayoutCommand={dispatchLayout}
      viewRegistry={currentState.app.workbench.views}
      viewRenderers={viewRenderers}
    >
      <div className="velvet-native-workspace">
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
        {surfaceError ? (
          <div aria-live="polite" className="velvet-command-error" role="alert">
            <span>
              App surface failed:{' '}
              {surfaceError instanceof Error ? surfaceError.message : String(surfaceError)}
            </span>
            <button
              className="velvet-button velvet-button--quiet"
              onClick={() => setSurfaceError(undefined)}
              type="button"
            >
              Dismiss
            </button>
          </div>
        ) : null}
        <div className="velvet-empty-state" data-app-revision={appRevision}>
          <div>
            <p>{currentState.layout.activeApp} workspace</p>
            <p>Rust-owned layout and native keybindings are active.</p>
          </div>
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
