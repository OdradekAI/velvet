import type {
  AppSurfaceLifecycle,
} from '../apps/workbench-app.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import type { WorkspaceContextSnapshot } from '../services/workspace-context.js';

export const SUPPORTED_UI_BRIDGE_VERSION = '1';
export const UI_BRIDGE_HOST_EVENT = 'velvet-ui-bridge-host';
export const UI_BRIDGE_SURFACE_EVENT = 'velvet-ui-bridge-surface';

export type PluginSurfaceKind = 'app-surface' | 'view' | 'dialog' | 'popover' | 'inspector';

export type PluginSurfaceState =
  | 'registered'
  | 'mounted'
  | 'active'
  | 'suspended'
  | 'failed'
  | 'disposed';

export interface UiThemeSnapshot {
  readonly scheme: 'light' | 'dark';
  readonly revision: number;
  readonly tokens: Readonly<Record<string, string>>;
}

export interface UiSurfaceSize {
  readonly width: number;
  readonly height: number;
  readonly devicePixelRatio?: number;
}

export type HostToSurfaceMessage =
  | {
      readonly type: 'host.ready';
      readonly bridgeVersion: string;
      readonly surfaceId: string;
      readonly theme: UiThemeSnapshot;
      readonly size: UiSurfaceSize;
      readonly context: WorkspaceContextSnapshot;
    }
  | { readonly type: 'host.context'; readonly context: WorkspaceContextSnapshot }
  | { readonly type: 'host.theme'; readonly theme: UiThemeSnapshot }
  | { readonly type: 'host.resize'; readonly size: UiSurfaceSize }
  | { readonly type: 'host.focus'; readonly focused: boolean }
  | { readonly type: 'host.dispose'; readonly reason: 'unmount' | 'crash' | 'incompatible' }
  | { readonly type: 'host.error'; readonly code: string; readonly message: string };

export type SurfaceToHostMessage =
  | { readonly type: 'surface.ready'; readonly bridgeVersion: string }
  | { readonly type: 'surface.focus-request' }
  | {
      readonly type: 'surface.command';
      readonly requestId: string;
      readonly commandId: string;
      readonly args: unknown;
    }
  | { readonly type: 'surface.close' }
  | { readonly type: 'surface.error'; readonly message: string };

export interface UiBridgeEndpoint {
  /** Sends a host lifecycle message to the mounted iframe or child WebView. */
  send(message: HostToSurfaceMessage): void;
  /** Lets a transport adapter forward host messages across its native boundary. */
  subscribeHost(listener: (message: HostToSurfaceMessage) => void): () => void;
  subscribe(listener: (message: SurfaceToHostMessage) => void): () => void;
  receive(message: SurfaceToHostMessage): void;
  close(): void;
}

export interface PluginSurfaceAdapter {
  mount(endpoint: UiBridgeEndpoint, host?: HTMLElement): void | Promise<void>;
  attach?(host: HTMLElement): void | Promise<void>;
  unmount(reason: 'unmount' | 'crash' | 'incompatible'): void | Promise<void>;
}

export interface PluginSurfaceDescriptor {
  readonly id: string;
  readonly extensionId: string;
  readonly namespace: string;
  readonly kind: PluginSurfaceKind;
  readonly entrypoint: string;
  readonly bridgeVersion: string;
  readonly appId?: WorkbenchAppId;
}

export interface PluginSurfaceMountOptions {
  readonly context: WorkspaceContextSnapshot;
  readonly theme: UiThemeSnapshot;
  readonly size: UiSurfaceSize;
}

export interface PluginSurfaceHandlers {
  readonly onCommand?: (
    commandId: string,
    args: unknown,
  ) => void | Promise<void>;
  readonly onFocusRequest?: () => void;
}

export interface PluginSurfaceSession {
  readonly surfaceId: string;
  readonly state: PluginSurfaceState;
  mount(options: PluginSurfaceMountOptions, host?: HTMLElement): Promise<void>;
  attach(host: HTMLElement): void | Promise<void>;
  activate(): void;
  deactivate(): void;
  updateContext(context: WorkspaceContextSnapshot): void;
  updateTheme(theme: UiThemeSnapshot): void;
  resize(size: UiSurfaceSize): void;
  dispose(reason?: 'unmount' | 'crash' | 'incompatible'): Promise<void>;
}

/** Adapts an isolated session to the WorkbenchApp AppSurface lifecycle seam. */
export function asAppSurfaceLifecycle(
  session: PluginSurfaceSession,
  options: PluginSurfaceMountOptions,
): AppSurfaceLifecycle {
  return {
    mount: () => session.mount(options),
    activate: () => session.activate(),
    deactivate: () => session.deactivate(),
    dispose: () => session.dispose(),
  };
}

export class PluginUiBridgeError extends Error {
  public constructor(message: string, public readonly surfaceId: string) {
    super(message);
    this.name = 'PluginUiBridgeError';
  }
}

export class PluginUiRegistry {
  private readonly descriptors = new Map<string, PluginSurfaceDescriptor>();
  private readonly adapters = new Map<string, PluginSurfaceAdapter>();
  private readonly sessions = new Map<string, PluginSurfaceSessionImpl>();
  private readonly listeners = new Set<() => void>();

  public register(descriptor: PluginSurfaceDescriptor): void {
    this.validateDescriptor(descriptor);
    const key = surfaceKey(descriptor.extensionId, descriptor.id);
    if (this.descriptors.has(key)) {
      throw new PluginUiBridgeError(`Duplicate plugin surface: ${key}`, key);
    }

    this.descriptors.set(key, descriptor);
    this.notify();
  }

  public bindAdapter(
    extensionId: string,
    surfaceId: string,
    adapter: PluginSurfaceAdapter,
  ): void {
    const key = surfaceKey(extensionId, surfaceId);
    if (!this.descriptors.has(key)) {
      throw new PluginUiBridgeError(`Unknown plugin surface: ${key}`, key);
    }

    this.adapters.set(key, adapter);
    this.notify();
  }

  public getAdapter(
    extensionId: string,
    surfaceId: string,
  ): PluginSurfaceAdapter | undefined {
    return this.adapters.get(surfaceKey(extensionId, surfaceId));
  }

  public getDescriptor(
    extensionId: string,
    surfaceId: string,
  ): PluginSurfaceDescriptor | undefined {
    return this.descriptors.get(surfaceKey(extensionId, surfaceId));
  }

  public getSession(
    extensionId: string,
    surfaceId: string,
  ): PluginSurfaceSession | undefined {
    return this.sessions.get(surfaceKey(extensionId, surfaceId));
  }

  /** Removes an unmounted surface descriptor during activation rollback. */
  public unregister(extensionId: string, surfaceId: string): boolean {
    const key = surfaceKey(extensionId, surfaceId);
    if (this.sessions.has(key)) {
      throw new PluginUiBridgeError(`Plugin surface session is still active: ${key}`, key);
    }

    const removed = this.descriptors.delete(key);
    this.adapters.delete(key);
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public createSession(
    extensionId: string,
    surfaceId: string,
    handlers: PluginSurfaceHandlers = {},
  ): PluginSurfaceSession {
    const key = surfaceKey(extensionId, surfaceId);
    const descriptor = this.descriptors.get(key);
    if (!descriptor) {
      throw new PluginUiBridgeError(`Unknown plugin surface: ${key}`, key);
    }
    if (this.sessions.has(key)) {
      throw new PluginUiBridgeError(`Plugin surface session already exists: ${key}`, key);
    }

    const adapter = this.adapters.get(key);
    if (!adapter) {
      throw new PluginUiBridgeError(`Plugin surface adapter is not bound: ${key}`, key);
    }

    const session = new PluginSurfaceSessionImpl(
      key,
      descriptor,
      adapter,
      handlers,
      () => {
        this.sessions.delete(key);
        this.notify();
      },
    );
    this.sessions.set(key, session);
    return session;
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public async disposeExtension(
    extensionId: string,
    reason: 'unmount' | 'crash' | 'incompatible' = 'unmount',
  ): Promise<void> {
    const sessions = [...this.sessions.entries()].filter(([key]) =>
      key.startsWith(`${extensionId}:`),
    );

    // Remove descriptors before disposing sessions. The renderer may be
    // subscribed to registry changes; seeing an ended session while its
    // descriptor is still present would immediately remount it during plugin
    // failure cleanup.
    for (const key of [...this.descriptors.keys()]) {
      if (key.startsWith(`${extensionId}:`)) {
        this.descriptors.delete(key);
        this.adapters.delete(key);
      }
    }

    for (const [key, session] of sessions) {
      await session.dispose(reason);
      this.sessions.delete(key);
    }
    this.notify();
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One renderer projection must not prevent other surfaces from updating.
      }
    }
  }

  private validateDescriptor(descriptor: PluginSurfaceDescriptor): void {
    if (descriptor.bridgeVersion !== SUPPORTED_UI_BRIDGE_VERSION) {
      throw new PluginUiBridgeError(
        `Unsupported UI bridge version: ${descriptor.bridgeVersion}`,
        descriptor.id,
      );
    }
    if (descriptor.namespace === 'workbench' && descriptor.appId) {
      throw new PluginUiBridgeError(
        `Global surface cannot declare an app id: ${descriptor.id}`,
        descriptor.id,
      );
    }
    if (
      descriptor.namespace !== 'workbench' &&
      (!descriptor.appId || descriptor.namespace !== `workbench.app.${descriptor.appId}`)
    ) {
      throw new PluginUiBridgeError(
        `Plugin surface has an invalid namespace: ${descriptor.id}`,
        descriptor.id,
      );
    }
  }
}

class PluginSurfaceSessionImpl implements PluginSurfaceSession {
  public state: PluginSurfaceState = 'registered';
  public readonly surfaceId: string;
  private readonly endpoint = new InMemoryUiBridgeEndpoint();
  private readonly unsubscribe: () => void;
  private options: PluginSurfaceMountOptions | undefined;
  private resourcesDisposed = false;

  public constructor(
    _key: string,
    private readonly descriptor: PluginSurfaceDescriptor,
    private readonly adapter: PluginSurfaceAdapter,
    private readonly handlers: PluginSurfaceHandlers,
    private readonly onDisposed: () => void,
  ) {
    this.surfaceId = descriptor.id;
    this.unsubscribe = this.endpoint.subscribe((message) => {
      void this.handleMessage(message);
    });
  }

  public async mount(options: PluginSurfaceMountOptions, host?: HTMLElement): Promise<void> {
    this.ensureState('registered');
    this.options = options;
    try {
      await this.adapter.mount(this.endpoint, host);
      this.endpoint.send({
        type: 'host.ready',
        bridgeVersion: this.descriptor.bridgeVersion,
        surfaceId: this.surfaceId,
        theme: options.theme,
        size: options.size,
        context: options.context,
      });
      this.state = 'mounted';
    } catch (cause) {
      this.state = 'failed';
      this.endpoint.send({
        type: 'host.error',
        code: 'surface-mount-failed',
        message: cause instanceof Error ? cause.message : String(cause),
      });
      const detail = cause instanceof Error ? cause.message : String(cause);
      throw new PluginUiBridgeError(
        `Plugin surface mount failed: ${this.surfaceId}: ${detail}`,
        this.surfaceId,
      );
    }
  }

  public async attach(host: HTMLElement): Promise<void> {
    this.ensureMounted();
    await this.adapter.attach?.(host);
  }

  public activate(): void {
    if (this.state !== 'mounted' && this.state !== 'suspended') {
      throw new PluginUiBridgeError(`Plugin surface cannot activate from ${this.state}`, this.surfaceId);
    }
    this.state = 'active';
    this.endpoint.send({ type: 'host.focus', focused: true });
  }

  public deactivate(): void {
    if (this.state !== 'active') {
      return;
    }
    this.state = 'suspended';
    this.endpoint.send({ type: 'host.focus', focused: false });
  }

  public updateContext(context: WorkspaceContextSnapshot): void {
    this.ensureMounted();
    this.endpoint.send({ type: 'host.context', context });
  }

  public updateTheme(theme: UiThemeSnapshot): void {
    this.ensureMounted();
    this.endpoint.send({ type: 'host.theme', theme });
  }

  public resize(size: UiSurfaceSize): void {
    this.ensureMounted();
    this.endpoint.send({ type: 'host.resize', size });
  }

  public async dispose(reason: 'unmount' | 'crash' | 'incompatible' = 'unmount'): Promise<void> {
    if (this.resourcesDisposed) {
      return;
    }

    this.endpoint.send({ type: 'host.dispose', reason });
    try {
      await this.adapter.unmount(reason);
    } catch {
      this.state = 'failed';
      this.resourcesDisposed = true;
      this.unsubscribe();
      this.endpoint.close();
      this.onDisposed();
      return;
    }

    this.state = reason === 'crash' ? 'failed' : 'disposed';
    this.resourcesDisposed = true;
    this.unsubscribe();
    this.endpoint.close();
    this.options = undefined;
    this.onDisposed();
  }

  private async handleMessage(message: SurfaceToHostMessage): Promise<void> {
    switch (message.type) {
      case 'surface.ready':
        if (message.bridgeVersion !== this.descriptor.bridgeVersion) {
          this.state = 'failed';
          this.endpoint.send({
            type: 'host.error',
            code: 'bridge-version-mismatch',
            message: `Expected bridge ${this.descriptor.bridgeVersion}, received ${message.bridgeVersion}`,
          });
        }
        return;
      case 'surface.focus-request':
        this.handlers.onFocusRequest?.();
        this.endpoint.send({ type: 'host.focus', focused: true });
        return;
      case 'surface.command':
        if (!this.handlers.onCommand) {
          this.endpoint.send({
            type: 'host.error',
            code: 'command-handler-unavailable',
            message: `No command handler is bound for ${message.commandId}`,
          });
          return;
        }
        try {
          await this.handlers.onCommand(message.commandId, message.args);
        } catch (cause) {
          this.state = 'failed';
          this.endpoint.send({
            type: 'host.error',
            code: 'command-failed',
            message: cause instanceof Error ? cause.message : String(cause),
          });
        }
        return;
      case 'surface.close':
        await this.dispose('unmount');
        return;
      case 'surface.error':
        this.endpoint.send({
          type: 'host.error',
          code: 'surface-error',
          message: message.message,
        });
        await this.dispose('crash');
        return;
    }
  }

  private ensureMounted(): void {
    if (
      this.state !== 'mounted' &&
      this.state !== 'active' &&
      this.state !== 'suspended'
    ) {
      throw new PluginUiBridgeError(`Plugin surface is not mounted: ${this.surfaceId}`, this.surfaceId);
    }
  }

  private ensureState(expected: PluginSurfaceState): void {
    if (this.state !== expected) {
      throw new PluginUiBridgeError(
        `Plugin surface expected ${expected}, received ${this.state}`,
        this.surfaceId,
      );
    }
  }
}

class InMemoryUiBridgeEndpoint implements UiBridgeEndpoint {
  private readonly hostListeners = new Set<(message: HostToSurfaceMessage) => void>();
  private readonly listeners = new Set<(message: SurfaceToHostMessage) => void>();
  private closed = false;

  public send(message: HostToSurfaceMessage): void {
    if (this.closed) {
      throw new Error('UI bridge endpoint is closed');
    }
    for (const listener of this.hostListeners) {
      listener(message);
    }
  }

  public subscribeHost(listener: (message: HostToSurfaceMessage) => void): () => void {
    this.hostListeners.add(listener);
    return () => this.hostListeners.delete(listener);
  }

  public subscribe(listener: (message: SurfaceToHostMessage) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public receive(message: SurfaceToHostMessage): void {
    if (this.closed) {
      throw new Error('UI bridge endpoint is closed');
    }
    for (const listener of this.listeners) {
      listener(message);
    }
  }

  public close(): void {
    this.closed = true;
    this.hostListeners.clear();
    this.listeners.clear();
  }
}

function surfaceKey(extensionId: string, surfaceId: string): string {
  return `${extensionId}:${surfaceId}`;
}
