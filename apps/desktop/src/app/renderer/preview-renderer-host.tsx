import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import type { ResolvedPreview } from '../../workbench/preview/preview-host.js';
import { PreviewRendererRegistry } from '../../workbench/preview/preview-renderer-registry.js';
import type {
  PluginSurfaceSession,
  PluginSurfaceState,
  PluginSurfaceHandlers,
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

export interface PreviewRendererHostProps {
  readonly preview: ResolvedPreview;
  readonly registry: PreviewRendererRegistry;
  readonly context: WorkspaceContext;
  readonly theme?: UiThemeSnapshot;
  readonly handlers?: PluginSurfaceHandlers;
}

/**
 * Mounts one resolved renderer inside the host-owned preview frame. React
 * owns the container and session lifecycle; the renderer adapter owns the
 * iframe/native webview and communicates through the UI Bridge.
 */
export function PreviewRendererHost({
  preview,
  registry,
  context,
  theme = DEFAULT_THEME,
  handlers,
}: PreviewRendererHostProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const mountRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<PluginSurfaceSession | undefined>(undefined);
  const [size, setSize] = useState<UiSurfaceSize>(INITIAL_SIZE);
  const [state, setState] = useState<PluginSurfaceState | 'idle' | 'mounting'>('idle');
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
    const host = mountRef.current;
    if (!host) {
      return undefined;
    }

    let disposed = false;
    let session: PluginSurfaceSession | undefined;
    try {
      session = registry.createSession(preview, host, handlers);
    } catch (cause) {
      setState('failed');
      setError(cause instanceof Error ? cause.message : String(cause));
      return undefined;
    }

    if (!session) {
      setState('idle');
      setError(undefined);
      return undefined;
    }

    sessionRef.current = session;
    setState('mounting');
    setError(undefined);
    void session
      .mount({
        context: context.snapshot(),
        theme,
        size,
      }, host)
      .then(() => {
        if (disposed || sessionRef.current !== session) {
          return;
        }
        session?.activate();
        setState(session?.state ?? 'active');
      })
      .catch((cause: unknown) => {
        if (disposed || sessionRef.current !== session) {
          return;
        }
        setState('failed');
        setError(cause instanceof Error ? cause.message : String(cause));
        void session?.dispose('crash');
      });

    return () => {
      disposed = true;
      if (sessionRef.current === session) {
        sessionRef.current = undefined;
      }
      void session?.dispose('unmount');
    };
  }, [handlers, preview, registry]);

  useEffect(() => {
    const session = sessionRef.current;
    if (!session || (session.state !== 'mounted' && session.state !== 'active' && session.state !== 'suspended')) {
      return undefined;
    }
    session.resize(size);
    return undefined;
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
    '--velvet-preview-renderer-state': state,
  } as CSSProperties;

  return (
    <div
      aria-busy={state === 'mounting' || state === 'registered'}
      className="velvet-preview-renderer"
      data-preview-renderer-state={state}
      ref={hostRef}
      style={style}
    >
      <div className="velvet-preview-renderer-mount" ref={mountRef} />
      {state === 'mounting' ? <p data-preview-renderer-loading>Loading preview renderer…</p> : null}
      {error ? (
        <p aria-label="Preview renderer error" data-preview-renderer-error role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
