import { useEffect, useState } from 'react';
import { createNativeTauriDesktopApp, type DesktopApp } from '../desktop-app.js';
import type { LayoutCommand, LayoutSnapshot, WorkbenchAppId } from '../../platform/protocol.js';
import { WorkbenchShell } from './workbench-shell.js';
import { ViewRendererRegistry } from './view-renderer-registry.js';
import { createPluginViewRenderer } from './plugin-view-renderer.js';

interface NativeWorkbenchState {
  readonly app: DesktopApp;
  readonly layout: LayoutSnapshot;
}

/** Mounts the renderer against the real Rust-owned Tauri workbench. */
export function NativeWorkbenchApp() {
  const [state, setState] = useState<NativeWorkbenchState>();
  const [startupError, setStartupError] = useState<unknown>();
  const [commandError, setCommandError] = useState<unknown>();
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
    void currentState.app.workbench.layout.activateApp(appId).catch(reportCommandError);
  }

  function dispatchLayout(command: LayoutCommand): void {
    setCommandError(undefined);
    void currentState.app.workbench.layout.apply(command).catch(reportCommandError);
  }

  return (
    <WorkbenchShell
      apps={currentState.app.workbench.apps.list()}
      focusService={currentState.app.workbench.focus}
      layout={currentState.layout}
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
        <div className="velvet-empty-state">
          <div>
            <p>{currentState.layout.activeApp} workspace</p>
            <p>Rust-owned layout and native keybindings are active.</p>
          </div>
        </div>
      </div>
    </WorkbenchShell>
  );
}
