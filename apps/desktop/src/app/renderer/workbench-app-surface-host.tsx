import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactNode, CSSProperties } from 'react';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import type { WorkbenchAppRegistry } from '../../workbench/apps/workbench-app.js';
import type { PluginClient } from '../../workbench/plugins/plugin-client.js';
import type {
  PluginSurfaceSession,
  PluginSurfaceState,
  UiSurfaceSize,
} from '../../workbench/plugins/ui-bridge.js';
import type {
  WorkspaceContext,
  WorkspaceContextCommand,
} from '../../workbench/services/workspace-context.js';

const INITIAL_SIZE: UiSurfaceSize = { width: 0, height: 0 };

export interface WorkbenchAppSurfaceHostProps {
  readonly appId: WorkbenchAppId;
  readonly apps: WorkbenchAppRegistry;
  readonly plugins: PluginClient;
  readonly context: WorkspaceContext;
  readonly dispatch: (command: WorkspaceContextCommand) => void;
  readonly fallback: ReactNode;
}

/**
 * React owns the WorkspaceArea host element. WorkbenchAppRegistry owns the
 * isolated surface lifecycle; this adapter only observes its session and
 * forwards host-owned context and geometry to the UI Bridge.
 */
export function WorkbenchAppSurfaceHost({
  appId,
  apps,
  plugins,
  context,
  dispatch,
  fallback,
}: WorkbenchAppSurfaceHostProps) {
  const descriptor = apps.get(appId);
  const hostRef = useRef<HTMLDivElement>(null);
  const [session, setSession] = useState<PluginSurfaceSession>();
  const [size, setSize] = useState<UiSurfaceSize>(INITIAL_SIZE);
  const [surfaceState, setSurfaceState] = useState<PluginSurfaceState | 'waiting'>('waiting');
  const [error, setError] = useState<string>();
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host || descriptor?.surface.trust !== 'isolated') {
      return undefined;
    }

    const updateSize = () => {
      setSize({
        width: host.clientWidth,
        height: host.clientHeight,
        devicePixelRatio: window.devicePixelRatio,
      });
    };
    updateSize();
    if (typeof ResizeObserver === 'undefined') {
      return undefined;
    }

    const observer = new ResizeObserver(updateSize);
    observer.observe(host);
    return () => observer.disconnect();
  }, [descriptor?.surface.trust]);

  useEffect(() => {
    if (descriptor?.surface.trust !== 'isolated') {
      return undefined;
    }

    let disposed = false;
    let activating = false;
    const activate = () => {
      const runtimeState = apps.getSurfaceState(appId);
      if (
        disposed ||
        activating ||
        runtimeState === 'active' ||
        runtimeState === 'mounted' ||
        runtimeState === 'registered'
      ) {
        return;
      }
      activating = true;
      void apps.activateSurface(appId, {
        appId,
        context,
        dispatch: (command) => dispatchRef.current(command),
      }).then(syncSession).catch((reason: unknown) => {
        if (!disposed) {
          setSurfaceState('failed');
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      }).finally(() => {
        activating = false;
      });
    };
    const syncSession = () => {
      const binding = plugins.listAppSurfaceBindings().find((candidate) => candidate.appId === appId);
      const next = binding
        ? plugins.ui.getSession(binding.extensionId, binding.surfaceId)
        : undefined;
      if (disposed) {
        return;
      }
      setSession(next);
      setSurfaceState(next?.state ?? 'waiting');
      if (next) {
        setError(undefined);
      }
      if (binding && (!next || apps.getSurfaceState(appId) === 'suspended')) {
        activate();
      }
    };

    syncSession();
    const unsubscribeApps = apps.subscribe(syncSession);
    const unsubscribeUi = plugins.ui.subscribe(syncSession);
    activate();

    return () => {
      disposed = true;
      unsubscribeApps();
      unsubscribeUi();
      void apps.deactivateSurface(appId).catch(() => undefined);
    };
  }, [appId, apps, context, descriptor?.surface.trust, plugins]);

  useEffect(() => {
    if (!session) {
      return;
    }
    if (
      session.state === 'mounted' ||
      session.state === 'active' ||
      session.state === 'suspended'
    ) {
      session.resize(size);
      setSurfaceState(session.state);
    }
  }, [session, size]);

  useEffect(() => {
    if (!session) {
      return undefined;
    }
    return context.subscribe((snapshot) => {
      if (
        session.state === 'mounted' ||
        session.state === 'active' ||
        session.state === 'suspended'
      ) {
        session.updateContext(snapshot);
      }
    });
  }, [context, session]);

  if (descriptor?.surface.trust !== 'isolated') {
    return <>{fallback}</>;
  }

  const style = {
    '--velvet-app-surface-state': surfaceState,
  } as CSSProperties;

  return (
    <div
      aria-label={`${descriptor.title} app surface`}
      className="velvet-app-surface-host"
      data-surface-state={surfaceState}
      ref={hostRef}
      style={style}
      tabIndex={-1}
    >
      {error ? (
        <div aria-live="polite" className="velvet-command-error" role="alert">
          <span>App surface failed: {error}</span>
          <button
            className="velvet-button velvet-button--quiet"
            onClick={() => setError(undefined)}
            type="button"
          >
            Dismiss
          </button>
        </div>
      ) : null}
      {!session && !error ? (
        <div className="velvet-empty-state">
          <div>Waiting for the {descriptor.title} UI Bridge…</div>
        </div>
      ) : null}
    </div>
  );
}
