import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import type { WorkbenchAppRegistry } from '../../workbench/apps/workbench-app.js';
import type {
  WorkspaceContext,
  WorkspaceContextCommand,
} from '../../workbench/services/workspace-context.js';
import type { UiSurfaceSize } from '../../workbench/plugins/ui-bridge.js';

const INITIAL_SIZE: UiSurfaceSize = { width: 0, height: 0 };

export interface TrustedAppSurfaceHostProps {
  readonly appId: WorkbenchAppId;
  readonly apps: WorkbenchAppRegistry;
  readonly context: WorkspaceContext;
  readonly dispatch: (command: WorkspaceContextCommand) => void;
  readonly fallback: ReactNode;
}

/**
 * Host adapter for built-in AppSurfaces. Built-ins render in the desktop
 * React tree, but their lifecycle still comes from WorkbenchAppRegistry so
 * switching, suspension, retry and failure use the same host seam as an
 * isolated UI Bridge surface.
 */
export function TrustedAppSurfaceHost({
  appId,
  apps,
  context,
  dispatch,
  fallback,
}: TrustedAppSurfaceHostProps) {
  const descriptor = apps.get(appId);
  const hostRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<UiSurfaceSize>(INITIAL_SIZE);
  const [surfaceState, setSurfaceState] = useState<'waiting' | 'active' | 'suspended' | 'failed'>('waiting');
  const [error, setError] = useState<string>();
  const [retryToken, setRetryToken] = useState(0);
  const [contextRevision, setContextRevision] = useState(() => context.snapshot().revision);
  const activationFailedRef = useRef(false);
  const activationInFlightRef = useRef(false);
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host) {
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
  }, [appId]);

  useEffect(() => {
    if (descriptor?.surface.trust !== 'trusted') {
      return undefined;
    }

    activationFailedRef.current = false;
    activationInFlightRef.current = false;
    let disposed = false;

    const sync = () => {
      if (disposed) {
        return;
      }
      const state = apps.getSurfaceState(appId);
      if (state === 'active' || state === 'suspended' || state === 'failed') {
        setSurfaceState(state);
      } else {
        setSurfaceState('waiting');
      }
    };

    const activate = () => {
      if (disposed || activationFailedRef.current || activationInFlightRef.current) {
        return;
      }
      if (apps.getSurfaceState(appId) === 'active') {
        sync();
        return;
      }

      activationInFlightRef.current = true;
      void apps.activateSurface(appId, {
        appId,
        context,
        dispatch: (command) => dispatchRef.current(command),
      })
        .then((activated) => {
          if (!disposed) {
            if (!activated) {
              setSurfaceState('waiting');
              return;
            }
            setError(undefined);
            sync();
          }
        })
        .catch((reason: unknown) => {
          if (!disposed) {
            activationFailedRef.current = true;
            setSurfaceState('failed');
            setError(reason instanceof Error ? reason.message : String(reason));
          }
        })
        .finally(() => {
          activationInFlightRef.current = false;
        });
    };

    sync();
    const unsubscribeApps = apps.subscribe(sync);
    const unsubscribeContext = context.subscribe((snapshot) => {
      setContextRevision(snapshot.revision);
    });
    activate();

    return () => {
      disposed = true;
      unsubscribeApps();
      unsubscribeContext();
      void apps.deactivateSurface(appId).catch(() => undefined);
    };
  }, [appId, apps, context, descriptor?.surface.trust, retryToken]);

  if (descriptor?.surface.trust !== 'trusted') {
    return <>{fallback}</>;
  }

  const style = {
    '--velvet-app-surface-state': surfaceState,
  } as CSSProperties;
  const snapshot = context.snapshot();

  if (surfaceState === 'waiting' && !error) {
    return <>{fallback}</>;
  }

  return (
    <div
      aria-label={`${descriptor.title} app surface`}
      className="velvet-app-surface-host velvet-app-surface-host--trusted"
      data-surface-state={surfaceState}
      data-trusted-app-surface={appId}
      ref={hostRef}
      style={style}
      tabIndex={-1}
    >
      {surfaceState === 'failed' ? (
        <div aria-live="polite" className="velvet-command-error" role="alert">
          <span>App surface failed: {error ?? 'unknown error'}</span>
          <button
            className="velvet-button velvet-button--quiet"
            data-app-surface-retry
            onClick={() => {
              activationFailedRef.current = false;
              setError(undefined);
              setSurfaceState('waiting');
              setRetryToken((token) => token + 1);
            }}
            type="button"
          >
            Retry
          </button>
        </div>
      ) : (
        <div className="velvet-trusted-app-surface" data-app-surface-content>
          <h2>{descriptor.title}</h2>
          <p>Trusted AppSurface · {descriptor.surface.entrypoint}</p>
          <p data-app-surface-context>
            Context revision {contextRevision} · active app {snapshot.activeAppId ?? 'none'} · size {size.width}×{size.height}
          </p>
        </div>
      )}
    </div>
  );
}
