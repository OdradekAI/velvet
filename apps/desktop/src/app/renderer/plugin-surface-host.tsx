import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import type {
  PluginSurfaceSession,
  PluginSurfaceState,
  PluginUiRegistry,
  UiSurfaceSize,
  UiThemeSnapshot,
} from '../../workbench/plugins/ui-bridge.js';
import type { WorkspaceContext } from '../../workbench/services/workspace-context.js';

const DEFAULT_THEME: UiThemeSnapshot = {
  scheme: 'dark',
  revision: 0,
  tokens: {},
};

const INITIAL_SIZE: UiSurfaceSize = { width: 0, height: 0 };

export interface PluginSurfaceHostProps {
  readonly ui: PluginUiRegistry;
  readonly extensionId: string;
  readonly surfaceId: string;
  readonly context: WorkspaceContext;
  readonly theme?: UiThemeSnapshot;
}

/**
 * React owns the host element and lifecycle only. The isolated adapter owns
 * the actual surface (iframe, native webview, or sidecar view) behind the UI
 * Bridge contract.
 */
export function PluginSurfaceHost({
  ui,
  extensionId,
  surfaceId,
  context,
  theme = DEFAULT_THEME,
}: PluginSurfaceHostProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<PluginSurfaceSession | undefined>(undefined);
  const waitingForAdapterRef = useRef(true);
  const [size, setSize] = useState<UiSurfaceSize>(INITIAL_SIZE);
  const [state, setState] = useState<PluginSurfaceState | 'waiting'>('waiting');
  const [error, setError] = useState<string>();

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
  }, []);

  useEffect(() => {
    let disposed = false;

    const mount = () => {
      if (disposed || !waitingForAdapterRef.current || sessionRef.current) {
        return;
      }

      let session: PluginSurfaceSession;
      try {
        session = ui.createSession(extensionId, surfaceId, {
          onFocusRequest: () => hostRef.current?.focus(),
        });
      } catch (reason) {
        setState('waiting');
        setError(reason instanceof Error ? reason.message : String(reason));
        return;
      }

      sessionRef.current = session;
      waitingForAdapterRef.current = false;
      setError(undefined);
      setState(session.state);
      void session
        .mount({
          context: context.snapshot(),
          theme,
          size,
        })
        .then(() => {
          if (disposed) {
            return;
          }
          session.activate();
          setState(session.state);
        })
        .catch((reason: unknown) => {
          if (!disposed) {
            setState('failed');
            setError(reason instanceof Error ? reason.message : String(reason));
          }
          void session.dispose('crash').finally(() => {
            if (sessionRef.current === session) {
              sessionRef.current = undefined;
            }
          });
        });
    };

    mount();
    const unsubscribe = ui.subscribe(mount);
    return () => {
      disposed = true;
      unsubscribe();
      const session = sessionRef.current;
      sessionRef.current = undefined;
      if (session) {
        void session.dispose('unmount');
      }
    };
  }, [context, extensionId, surfaceId, theme, ui]);

  useEffect(() => {
    const session = sessionRef.current;
    if (session && (session.state === 'mounted' || session.state === 'active' || session.state === 'suspended')) {
      session.resize(size);
    }
  }, [size]);

  useEffect(() => {
    const unsubscribe = context.subscribe((snapshot) => {
      const session = sessionRef.current;
      if (session && (session.state === 'mounted' || session.state === 'active' || session.state === 'suspended')) {
        session.updateContext(snapshot);
      }
    });
    return unsubscribe;
  }, [context]);

  useEffect(() => {
    const session = sessionRef.current;
    if (session && (session.state === 'mounted' || session.state === 'active' || session.state === 'suspended')) {
      session.updateTheme(theme);
    }
  }, [theme]);

  const style = {
    '--velvet-plugin-surface-state': state,
  } as CSSProperties;

  return (
    <div
      aria-busy={state === 'waiting' || state === 'registered' || state === 'mounted'}
      className="velvet-plugin-surface"
      data-plugin-surface={surfaceId}
      data-plugin-surface-state={state}
      ref={hostRef}
      style={style}
      tabIndex={-1}
    >
      {state === 'active' ? null : (
        <p className="velvet-view-placeholder">
          {error ?? `Waiting for isolated surface adapter (${surfaceId}).`}
        </p>
      )}
    </div>
  );
}
