import type {
  AppSurfaceLifecycle,
} from '../apps/workbench-app.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import type { WorkspaceContextSnapshot } from '../services/workspace-context.js';
import {
  normalizeUiThemeSnapshot,
  negotiateUiComponentVersion,
  isAllowedUiEntrypoint,
  SUPPORTED_UI_COMPONENT_CONTRACT_VERSION,
  SUPPORTED_UI_KIT_API_VERSION,
  type UiThemeSnapshot,
  type UiThemeSnapshotInput,
  type UiOverlayRequest,
  type UiProgressRequest,
} from './ui-kit.js';

export type {
  HostOnlyUiSurface,
  PluginUiComponent,
  UiContrast,
  UiDensity,
  UiSemanticToken,
  UiThemeScheme,
  UiThemeSnapshot,
  UiThemeSnapshotInput,
  UiOverlayRequest,
  UiProgressRequest,
} from './ui-kit.js';

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

export interface UiSurfaceSize {
  readonly width: number;
  readonly height: number;
  readonly devicePixelRatio?: number;
}

export interface UiLocaleProjection {
  readonly extensionId: string;
  readonly locale: string;
  readonly revision: number;
  readonly messages: Readonly<Record<string, string>>;
}

export type HostToSurfaceMessage =
  | {
      readonly type: 'host.ready';
      readonly bridgeVersion: string;
      readonly componentContractVersion: number;
      readonly surfaceId: string;
      readonly theme: UiThemeSnapshot;
      readonly size: UiSurfaceSize;
      readonly context: WorkspaceContextSnapshot;
      readonly locale?: UiLocaleProjection;
    }
  | { readonly type: 'host.context'; readonly context: WorkspaceContextSnapshot }
  | { readonly type: 'host.theme'; readonly theme: UiThemeSnapshot }
  | { readonly type: 'host.locale'; readonly locale: UiLocaleProjection }
  | { readonly type: 'host.resize'; readonly size: UiSurfaceSize }
  | { readonly type: 'host.focus'; readonly focused: boolean }
  | { readonly type: 'host.dispose'; readonly reason: 'unmount' | 'crash' | 'incompatible' }
  | { readonly type: 'host.error'; readonly code: string; readonly message: string };

export type SurfaceToHostMessage =
  | {
      readonly type: 'surface.ready';
      readonly bridgeVersion: string;
      readonly componentContractVersion?: number;
    }
  | { readonly type: 'surface.focus-request' }
  | {
      readonly type: 'surface.command';
      readonly requestId: string;
      readonly commandId: string;
      readonly args: unknown;
    }
  | { readonly type: 'surface.close' }
  | { readonly type: 'surface.overlay-request'; readonly request: UiOverlayRequest }
  | { readonly type: 'surface.notification'; readonly request: UiOverlayRequest }
  | { readonly type: 'surface.progress'; readonly request: UiProgressRequest }
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
  readonly uiKitApiVersion?: string;
  readonly componentContractVersion?: number;
  readonly zone?: 'navigation' | 'contextual' | 'operations';
}

export interface PluginSurfaceMountOptions {
  readonly context: WorkspaceContextSnapshot;
  readonly theme: UiThemeSnapshotInput;
  readonly size: UiSurfaceSize;
  readonly locale?: UiLocaleProjection;
}

export interface PluginSurfaceHandlers {
  readonly onCommand?: (
    commandId: string,
    args: unknown,
  ) => void | Promise<void>;
  readonly onFocusRequest?: () => void;
  readonly onOverlayRequest?: (request: UiOverlayRequest) => void | Promise<void>;
  readonly onNotification?: (request: UiOverlayRequest) => void | Promise<void>;
  readonly onProgress?: (request: UiProgressRequest) => void | Promise<void>;
}

export interface PluginSurfaceSession {
  readonly surfaceId: string;
  readonly state: PluginSurfaceState;
  mount(options: PluginSurfaceMountOptions, host?: HTMLElement): Promise<void>;
  attach(host: HTMLElement): void | Promise<void>;
  activate(): void;
  deactivate(): void;
  updateContext(context: WorkspaceContextSnapshot): void;
  updateTheme(theme: UiThemeSnapshotInput): void;
  updateLocale(locale: UiLocaleProjection): void;
  resize(size: UiSurfaceSize): void;
  dispose(reason?: 'unmount' | 'crash' | 'incompatible'): Promise<void>;
}

/**
 * Creates a short-lived UI Bridge session that is owned by another host
 * registry, such as PreviewRendererRegistry. The regular PluginUiRegistry
 * remains the owner for declared view/dialog/app surfaces; this factory keeps
 * transient renderer sessions out of that durable contribution registry.
 */
export function createTransientPluginSurfaceSession(
  descriptor: PluginSurfaceDescriptor,
  adapter: PluginSurfaceAdapter,
  handlers: PluginSurfaceHandlers = {},
  localeProjection?: UiLocaleProjection,
): PluginSurfaceSession {
  validateUiBridgeDescriptor(descriptor);
  return new PluginSurfaceSessionImpl(
    surfaceKey(descriptor.extensionId, descriptor.id),
    descriptor,
    adapter,
    handlers,
    localeProjection,
    () => undefined,
  );
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
  private readonly localeProjections = new Map<string, UiLocaleProjection>();
  private readonly listeners = new Set<() => void>();
  private transportSuspended = false;

  /** True while native transport recovery owns the surface lifecycle. */
  public get isTransportSuspended(): boolean {
    return this.transportSuspended;
  }

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
    if (this.transportSuspended) {
      throw new PluginUiBridgeError(
        'Plugin surface sessions are suspended while the core transport recovers',
        surfaceKey(extensionId, surfaceId),
      );
    }
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
      this.localeProjections.get(extensionId),
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

  /**
   * Tears down renderer-owned sessions while retaining descriptors and
   * adapters. A subsequent resume lets mounted hosts create fresh sessions
   * against the new native transport session.
   */
  public async suspendForTransportRecovery(): Promise<void> {
    if (this.transportSuspended) {
      return;
    }
    this.transportSuspended = true;
    this.notify();
    const sessions = [...this.sessions.values()];
    await Promise.allSettled(sessions.map((session) => session.dispose('crash')));
    this.notify();
  }

  /** Re-enables session creation after the authoritative snapshot is applied. */
  public resumeAfterTransportRecovery(): void {
    if (!this.transportSuspended) {
      return;
    }
    this.transportSuspended = false;
    this.notify();
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
    this.localeProjections.delete(extensionId);

    for (const [key, session] of sessions) {
      await session.dispose(reason);
      this.sessions.delete(key);
    }
    this.notify();
  }

  /** Projects the host locale to all current and future surfaces of an extension. */
  public projectLocale(
    extensionId: string,
    locale: UiLocaleProjection | undefined,
  ): void {
    if (locale) {
      this.localeProjections.set(extensionId, locale);
    } else {
      this.localeProjections.delete(extensionId);
    }
    for (const [key, session] of this.sessions) {
      if (key.startsWith(`${extensionId}:`) && locale) {
        try {
          session.updateLocale(locale);
        } catch {
          // A disposed surface is cleaned by its normal lifecycle path.
        }
      }
    }
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
    validateUiBridgeDescriptor(descriptor);
  }
}

function validateUiBridgeDescriptor(descriptor: PluginSurfaceDescriptor): void {
  if (descriptor.bridgeVersion !== SUPPORTED_UI_BRIDGE_VERSION) {
    throw new PluginUiBridgeError(
      `Unsupported UI bridge version: ${descriptor.bridgeVersion}`,
      descriptor.id,
    );
  }
  if (descriptor.uiKitApiVersion && descriptor.uiKitApiVersion !== SUPPORTED_UI_KIT_API_VERSION) {
    throw new PluginUiBridgeError(
      `Unsupported UI Kit API version: ${descriptor.uiKitApiVersion}`,
      descriptor.id,
    );
  }
  if (!isAllowedUiEntrypoint(descriptor.entrypoint)) {
    throw new PluginUiBridgeError(
      `UI surface entrypoint uses a disallowed scheme: ${descriptor.entrypoint}`,
      descriptor.id,
    );
  }
  if (
    descriptor.componentContractVersion !== undefined
    && negotiateUiComponentVersion([descriptor.componentContractVersion]) === undefined
  ) {
    throw new PluginUiBridgeError(
      `Unsupported UI component contract version: ${descriptor.componentContractVersion}`,
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
    descriptor.namespace !== 'capability' &&
    (!descriptor.appId || descriptor.namespace !== `workbench.app.${descriptor.appId}`)
  ) {
    throw new PluginUiBridgeError(
      `Plugin surface has an invalid namespace: ${descriptor.id}`,
      descriptor.id,
    );
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
    private localeProjection: UiLocaleProjection | undefined,
    private readonly onDisposed: () => void,
  ) {
    this.surfaceId = descriptor.id;
    this.unsubscribe = this.endpoint.subscribe((message) => {
      void this.handleMessage(message);
    });
  }

  public async mount(options: PluginSurfaceMountOptions, host?: HTMLElement): Promise<void> {
    this.ensureState('registered');
    let theme: UiThemeSnapshot;
    try {
      theme = normalizeUiThemeSnapshot(options.theme);
      this.options = { ...options, theme };
      await this.adapter.mount(this.endpoint, host);
      this.endpoint.send({
        type: 'host.ready',
        bridgeVersion: this.descriptor.bridgeVersion,
        componentContractVersion: SUPPORTED_UI_COMPONENT_CONTRACT_VERSION,
        surfaceId: this.surfaceId,
        theme,
        size: options.size,
        context: options.context,
        locale: options.locale ?? this.localeProjection,
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

  public updateTheme(theme: UiThemeSnapshotInput): void {
    this.ensureMounted();
    this.endpoint.send({ type: 'host.theme', theme: normalizeUiThemeSnapshot(theme) });
  }

  public updateLocale(locale: UiLocaleProjection): void {
    this.localeProjection = locale;
    if (this.state === 'mounted' || this.state === 'active' || this.state === 'suspended') {
      this.endpoint.send({ type: 'host.locale', locale });
    }
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
          await this.failSurface(
            'bridge-version-mismatch',
            `Expected bridge ${this.descriptor.bridgeVersion}, received ${message.bridgeVersion}`,
          );
          return;
        }
        if (
          message.componentContractVersion !== undefined
          && negotiateUiComponentVersion([message.componentContractVersion]) === undefined
        ) {
          await this.failSurface(
            'component-contract-version-mismatch',
            `Unsupported UI component contract version: ${message.componentContractVersion}`,
          );
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
      case 'surface.overlay-request':
        if (!this.handlers.onOverlayRequest) {
          this.endpoint.send({
            type: 'host.error',
            code: 'overlay-handler-unavailable',
            message: 'The host has no overlay handler for this surface',
          });
          return;
        }
        try {
          await this.handlers.onOverlayRequest(message.request);
        } catch (cause) {
          this.endpoint.send({
            type: 'host.error',
            code: 'overlay-request-failed',
            message: cause instanceof Error ? cause.message : String(cause),
          });
        }
        return;
      case 'surface.notification':
        if (!this.handlers.onNotification) {
          this.endpoint.send({
            type: 'host.error',
            code: 'notification-handler-unavailable',
            message: 'The host has no notification handler for this surface',
          });
          return;
        }
        try {
          await this.handlers.onNotification(message.request);
        } catch (cause) {
          this.endpoint.send({
            type: 'host.error',
            code: 'notification-request-failed',
            message: cause instanceof Error ? cause.message : String(cause),
          });
        }
        return;
      case 'surface.progress':
        try {
          await this.handlers.onProgress?.(message.request);
        } catch (cause) {
          this.endpoint.send({
            type: 'host.error',
            code: 'progress-request-failed',
            message: cause instanceof Error ? cause.message : String(cause),
          });
        }
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

  private async failSurface(code: string, message: string): Promise<void> {
    if (this.resourcesDisposed) {
      return;
    }
    this.state = 'failed';
    this.endpoint.send({ type: 'host.error', code, message });
    try {
      await this.adapter.unmount('incompatible');
    } catch {
      // The surface is already failed; cleanup remains best effort.
    }
    this.resourcesDisposed = true;
    this.unsubscribe();
    this.endpoint.close();
    this.options = undefined;
    this.onDisposed();
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
