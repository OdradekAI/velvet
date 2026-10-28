import { useEffect, useState } from 'react';
import { createNativeTauriDesktopApp, type DesktopApp } from '../desktop-app.js';
import type { LayoutCommand, LayoutSnapshot, WorkbenchAppId } from '../../platform/protocol.js';
import { BUILTIN_WORKBENCH_APPS } from '../../workspaces/index.js';
import { WorkbenchShell } from './workbench-shell.js';

interface NativeWorkbenchState {
  readonly app: DesktopApp;
  readonly layout: LayoutSnapshot;
}

/** Mounts the renderer against the real Rust-owned Tauri workbench. */
export function NativeWorkbenchApp() {
  const [state, setState] = useState<NativeWorkbenchState>();
  const [error, setError] = useState<unknown>();

  useEffect(() => {
    const app = createNativeTauriDesktopApp();
    const unsubscribe = app.workbench.layout.subscribe((layout) => {
      setState((current) => (current ? { ...current, layout } : current));
    });
    let disposed = false;

    void app.start()
      .then((layout) => {
        if (!disposed) {
          setState({ app, layout });
        }
      })
      .catch((reason: unknown) => {
        if (!disposed) {
          setError(reason);
        }
      });

    return () => {
      disposed = true;
      unsubscribe();
      void app.dispose();
    };
  }, []);

  if (error) {
    return (
      <main className="velvet-empty-state">
        <div>
          <p>Velvet could not start the native workbench.</p>
          <pre>{error instanceof Error ? error.message : String(error)}</pre>
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

  function activateApp(appId: WorkbenchAppId): void {
    void currentState.app.workbench.layout.activateApp(appId).catch(setError);
  }

  function dispatchLayout(command: LayoutCommand): void {
    void currentState.app.workbench.layout.apply(command).catch(setError);
  }

  return (
    <WorkbenchShell
      apps={BUILTIN_WORKBENCH_APPS}
      focusService={currentState.app.workbench.focus}
      layout={currentState.layout}
      onActivateApp={activateApp}
      onLayoutCommand={dispatchLayout}
    >
      <div className="velvet-empty-state">
        <div>
          <p>{currentState.layout.activeApp} workspace</p>
          <p>Rust-owned layout and native keybindings are active.</p>
        </div>
      </div>
    </WorkbenchShell>
  );
}
