import type {
  PluginSurfaceAdapter,
  PluginSurfaceDescriptor,
  SurfaceToHostMessage,
  UiBridgeEndpoint,
} from './ui-bridge.js';
import {
  UI_BRIDGE_HOST_EVENT,
  UI_BRIDGE_SURFACE_EVENT,
} from './ui-bridge.js';

export interface TauriSurfaceWebview {
  emit(event: string, payload: unknown): Promise<unknown>;
  listen<T>(event: string, listener: (event: { payload: T }) => void): Promise<() => void>;
  once<T>(event: string, listener: (event: { payload: T }) => void): unknown;
  show(): Promise<void>;
  hide(): Promise<void>;
  setFocus(): Promise<void>;
  setPosition(position: unknown): Promise<void>;
  setSize(size: unknown): Promise<void>;
  close(): Promise<void>;
}

export interface TauriSurfaceWebviewOptions {
  readonly url: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly focus: boolean;
  readonly dragDropEnabled: boolean;
}

export interface TauriSurfaceRuntime {
  getCurrentWebview(): { readonly window: unknown };
  createWebview(
    window: unknown,
    label: string,
    options: TauriSurfaceWebviewOptions,
  ): TauriSurfaceWebview;
  LogicalPosition: new (x: number, y: number) => unknown;
  LogicalSize: new (width: number, height: number) => unknown;
}

export interface PluginSurfaceAdapterOptions {
  readonly tauriRuntime?: TauriSurfaceRuntime;
}

/** Selects a native Tauri child WebView for remote/native entrypoints and an iframe elsewhere. */
export function createPluginSurfaceAdapter(
  descriptor: PluginSurfaceDescriptor,
  host: HTMLElement,
  options: PluginSurfaceAdapterOptions = {},
): PluginSurfaceAdapter {
  if (isTauriRuntime() && isTauriWebviewEntrypoint(descriptor.entrypoint)) {
    return new TauriWebviewSurfaceAdapter(descriptor, host, options.tauriRuntime);
  }

  return new IframeSurfaceAdapter(descriptor, host);
}

class IframeSurfaceAdapter implements PluginSurfaceAdapter {
  private iframe: HTMLIFrameElement | undefined;
  private removeHostSubscription: (() => void) | undefined;
  private readonly onMessage = (event: MessageEvent<unknown>) => {
    if (!this.iframe || event.source !== this.iframe.contentWindow) {
      return;
    }
    const message = unwrapSurfaceMessage(event.data);
    if (message) {
      this.endpoint?.receive(message);
    }
  };
  private endpoint: UiBridgeEndpoint | undefined;

  public constructor(
    private readonly descriptor: PluginSurfaceDescriptor,
    private host: HTMLElement,
  ) {}

  public mount(endpoint: UiBridgeEndpoint): void {
    if (this.iframe) {
      throw new Error(`Surface is already mounted: ${this.descriptor.id}`);
    }

    this.endpoint = endpoint;
    const iframe = document.createElement('iframe');
    iframe.className = 'velvet-plugin-surface-frame';
    iframe.title = this.descriptor.id;
    iframe.src = resolveIframeUrl(this.descriptor.entrypoint);
    iframe.setAttribute('sandbox', 'allow-forms allow-modals allow-popups allow-scripts');
    iframe.setAttribute('aria-label', `${this.descriptor.id} plugin surface`);
    iframe.style.border = '0';
    iframe.style.display = 'block';
    iframe.style.height = '100%';
    iframe.style.width = '100%';

    this.iframe = iframe;
    this.removeHostSubscription = endpoint.subscribeHost((message) => {
      iframe.contentWindow?.postMessage(
        { type: 'velvet.ui.host', message },
        '*',
      );
    });
    window.addEventListener('message', this.onMessage);
    this.host.replaceChildren(iframe);
  }

  public attach(host: HTMLElement): void {
    this.host = host;
    if (this.iframe && this.iframe.parentElement !== host) {
      host.replaceChildren(this.iframe);
    }
  }

  public unmount(_reason: 'unmount' | 'crash' | 'incompatible'): void {
    this.removeHostSubscription?.();
    this.removeHostSubscription = undefined;
    window.removeEventListener('message', this.onMessage);
    this.iframe?.remove();
    this.iframe = undefined;
    this.endpoint = undefined;
  }
}

class TauriWebviewSurfaceAdapter implements PluginSurfaceAdapter {
  private webview: TauriSurfaceWebview | undefined;
  private removeHostSubscription: (() => void) | undefined;
  private removeSurfaceListener: (() => void) | undefined;
  private runtime: TauriSurfaceRuntime | undefined;
  private focusRequest = 0;

  public constructor(
    private readonly descriptor: PluginSurfaceDescriptor,
    private host: HTMLElement,
    runtime?: TauriSurfaceRuntime,
  ) {
    this.runtime = runtime;
  }

  public async mount(endpoint: UiBridgeEndpoint): Promise<void> {
    if (this.webview) {
      throw new Error(`Surface is already mounted: ${this.descriptor.id}`);
    }

    const runtime = this.runtime ?? await loadTauriSurfaceRuntime();
    this.runtime = runtime;
    const current = runtime.getCurrentWebview();
    const label = surfaceLabel(this.descriptor);
    const bounds = this.bounds();
    const webview = runtime.createWebview(current.window, label, {
      url: resolveTauriUrl(this.descriptor.entrypoint),
      ...bounds,
      focus: false,
      dragDropEnabled: false,
    });
    this.webview = webview;

    this.removeHostSubscription = endpoint.subscribeHost((message) => {
      if (message.type === 'host.resize') {
        void this.setBounds();
      }
      if (message.type === 'host.focus') {
        const request = ++this.focusRequest;
        if (message.focused) {
          void webview.show()
            .then(() => request === this.focusRequest ? webview.setFocus() : undefined)
            .catch(() => undefined);
        } else {
          void webview.hide().catch(() => undefined);
        }
      }
      void webview.emit(UI_BRIDGE_HOST_EVENT, message).catch(() => undefined);
    });
    this.removeSurfaceListener = await webview.listen<unknown>(UI_BRIDGE_SURFACE_EVENT, (event) => {
      const message = unwrapSurfaceMessage(event.payload);
      if (message) {
        endpoint.receive(message);
      }
    });

    await waitForWebviewCreated(webview);
    await this.setBounds();
  }

  public attach(host: HTMLElement): void {
    this.host = host;
    void this.setBounds().catch(() => undefined);
  }

  public async unmount(_reason: 'unmount' | 'crash' | 'incompatible'): Promise<void> {
    this.focusRequest += 1;
    this.removeHostSubscription?.();
    this.removeHostSubscription = undefined;
    this.removeSurfaceListener?.();
    this.removeSurfaceListener = undefined;
    const webview = this.webview;
    this.webview = undefined;
    if (webview) {
      await webview.close();
    }
  }

  private async setBounds(): Promise<void> {
    const webview = this.webview;
    if (!webview) {
      return;
    }
    const bounds = this.bounds();
    const runtime = this.runtime ?? await loadTauriSurfaceRuntime();
    this.runtime = runtime;
    await webview.setPosition(new runtime.LogicalPosition(bounds.x, bounds.y));
    await webview.setSize(new runtime.LogicalSize(bounds.width, bounds.height));
  }

  private bounds(): { x: number; y: number; width: number; height: number } {
    const rect = this.host.getBoundingClientRect();
    return {
      x: Math.max(0, Math.round(rect.left)),
      y: Math.max(0, Math.round(rect.top)),
      width: Math.max(1, Math.round(rect.width)),
      height: Math.max(1, Math.round(rect.height)),
    };
  }
}

async function loadTauriSurfaceRuntime(): Promise<TauriSurfaceRuntime> {
  const [{ getCurrentWebview, Webview }, { LogicalPosition, LogicalSize }] = await Promise.all([
    import('@tauri-apps/api/webview'),
    import('@tauri-apps/api/dpi'),
  ]);

  return {
    getCurrentWebview: () => getCurrentWebview() as unknown as { readonly window: unknown },
    createWebview: (window, label, options) => new Webview(
      window as ConstructorParameters<typeof Webview>[0],
      label,
      options as ConstructorParameters<typeof Webview>[2],
    ) as unknown as TauriSurfaceWebview,
    LogicalPosition: LogicalPosition as unknown as new (x: number, y: number) => unknown,
    LogicalSize: LogicalSize as unknown as new (width: number, height: number) => unknown,
  };
}

function isTauriWebviewEntrypoint(entrypoint: string): boolean {
  return entrypoint.startsWith('tauri-webview://') || /^https?:\/\//.test(entrypoint);
}

function isTauriRuntime(): boolean {
  return Boolean((globalThis as { isTauri?: unknown }).isTauri);
}

function resolveTauriUrl(entrypoint: string): string {
  return entrypoint.startsWith('tauri-webview://')
    ? entrypoint.replace(/^tauri-webview:\/\//, 'https://')
    : entrypoint;
}

function resolveIframeUrl(entrypoint: string): string {
  if (/^(https?|asset|tauri|data|blob):/.test(entrypoint)) {
    return entrypoint;
  }
  return `about:blank#velvet-${encodeURIComponent(entrypoint)}`;
}

function surfaceLabel(descriptor: PluginSurfaceDescriptor): string {
  return `velvet-surface-${descriptor.extensionId}-${descriptor.id}`
    .replace(/[^A-Za-z0-9/_:-]/g, '_')
    .slice(0, 120);
}

function unwrapSurfaceMessage(value: unknown): SurfaceToHostMessage | undefined {
  const candidate = isRecord(value) && value.type === 'velvet.ui.surface'
    ? value.message
    : value;
  if (!isRecord(candidate) || typeof candidate.type !== 'string') {
    return undefined;
  }
  return candidate as unknown as SurfaceToHostMessage;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

async function waitForWebviewCreated(webview: TauriSurfaceWebview): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    void webview.once('tauri://created', () => resolve());
    void webview.once<string>('tauri://error', (event) => {
      reject(new Error(event.payload || 'Tauri WebView creation failed'));
    });
  });
}
