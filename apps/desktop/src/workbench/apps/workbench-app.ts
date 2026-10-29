import type {
  WorkspaceContext,
  WorkspaceContextCommand,
} from '../services/workspace-context.js';
import type { ViewContainerDescriptor, ViewDescriptor } from '../views/view-registry.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';

/** Stable identity for a selectable product surface inside the Workbench. */
export type AppSurfaceTrust = 'trusted' | 'isolated';

/**
 * Serialized surface metadata. The host resolves the entrypoint through a
 * trusted adapter or an isolated UI bridge; descriptors never contain React
 * components or other framework-owned objects.
 */
export interface AppSurfaceDescriptor {
  readonly trust: AppSurfaceTrust;
  readonly entrypoint: string;
  readonly bridgeVersion?: string;
}

export interface WorkbenchAppDescriptor {
  readonly id: WorkbenchAppId;
  readonly apiVersion?: string;
  readonly title: string;
  readonly activityBarIcon: string;
  readonly activityBarOrder?: number;
  readonly surface: AppSurfaceDescriptor;
  readonly viewContainers?: readonly ViewContainerDescriptor[];
  readonly views?: readonly ViewDescriptor[];
}

export interface AppSurfaceHost {
  readonly appId: WorkbenchAppId;
  readonly context: WorkspaceContext;
  /** App-scoped commands; activating another app remains host-owned. */
  dispatch(command: WorkspaceContextCommand): void;
}

/** Lifecycle returned by a host adapter after an app surface is created. */
export interface AppSurfaceLifecycle {
  mount(): void | Promise<void>;
  activate(): void | Promise<void>;
  deactivate(): void | Promise<void>;
  dispose(): void | Promise<void>;
}

export type AppSurfaceFactory = (
  host: AppSurfaceHost,
) => AppSurfaceLifecycle;

export type AppSurfaceRuntimeState =
  | 'registered'
  | 'mounted'
  | 'active'
  | 'suspended'
  | 'failed'
  | 'disposed';

interface AppSurfaceRuntime {
  readonly lifecycle: AppSurfaceLifecycle;
  state: AppSurfaceRuntimeState;
}

export interface ActivityBarViewContainerContribution {
  readonly id: string;
  readonly title: string;
  readonly icon: string;
  readonly containerId: string;
  readonly activityBarOrder?: number;
}

export type ActivityBarEntry =
  | {
      readonly kind: 'workbench-app';
      readonly appId: WorkbenchAppId;
      readonly title: string;
      readonly icon: string;
      readonly order: number;
    }
  | ({ readonly kind: 'view-container' } & ActivityBarViewContainerContribution & {
      readonly order: number;
    });

export class WorkbenchAppRegistry {
  private readonly descriptors = new Map<WorkbenchAppId, WorkbenchAppDescriptor>();
  private readonly factories = new Map<WorkbenchAppId, AppSurfaceFactory>();
  private readonly runtimes = new Map<WorkbenchAppId, AppSurfaceRuntime>();
  private readonly listeners = new Set<() => void>();

  public constructor(initial: readonly WorkbenchAppDescriptor[] = []) {
    for (const descriptor of initial) {
      this.register(descriptor);
    }
  }

  public register(descriptor: WorkbenchAppDescriptor): void {
    if (this.descriptors.has(descriptor.id)) {
      throw new Error(`Duplicate WorkbenchApp id: ${descriptor.id}`);
    }

    if (descriptor.surface.trust === 'isolated' && !descriptor.surface.bridgeVersion) {
      throw new Error(
        `Isolated WorkbenchApp ${descriptor.id} must declare a bridge version`,
      );
    }

    this.descriptors.set(descriptor.id, descriptor);
    this.notify();
  }

  public bindSurfaceFactory(appId: WorkbenchAppId, factory: AppSurfaceFactory): void {
    this.require(appId);
    if (this.runtimes.has(appId)) {
      throw new Error(`WorkbenchApp surface is already active: ${appId}`);
    }
    this.factories.set(appId, factory);
  }

  public createSurface(appId: WorkbenchAppId, host: AppSurfaceHost): AppSurfaceLifecycle {
    const factory = this.factories.get(appId);
    if (!factory) {
      throw new Error(`WorkbenchApp surface is not activated: ${appId}`);
    }

    return factory(host);
  }

  /**
   * Lazily mounts and activates the selected app surface. Apps without a
   * factory (the built-in placeholder surfaces in the current skeleton) are
   * valid registry entries and simply remain host-rendered.
   */
  public async activateSurface(
    appId: WorkbenchAppId,
    host: AppSurfaceHost,
  ): Promise<boolean> {
    this.require(appId);
    const factory = this.factories.get(appId);
    if (!factory) {
      return false;
    }

    let runtime = this.runtimes.get(appId);
    if (!runtime) {
      const lifecycle = factory(host);
      runtime = { lifecycle, state: 'registered' };
      this.runtimes.set(appId, runtime);
      this.notify();
      try {
        await lifecycle.mount();
        runtime.state = 'mounted';
        this.notify();
        await lifecycle.activate();
        runtime.state = 'active';
        this.notify();
      } catch (cause) {
        runtime.state = 'failed';
        this.notify();
        await this.disposeRuntime(appId, runtime, 'crash');
        throw cause;
      }
      return true;
    }

    if (runtime.state === 'active') {
      return true;
    }
    if (runtime.state === 'mounted' || runtime.state === 'suspended') {
      await runtime.lifecycle.activate();
      runtime.state = 'active';
      this.notify();
      return true;
    }

    throw new Error(`WorkbenchApp surface cannot activate from ${runtime.state}: ${appId}`);
  }

  /** Suspends a mounted app while another WorkbenchApp owns the WorkspaceArea. */
  public async deactivateSurface(appId: WorkbenchAppId): Promise<boolean> {
    const runtime = this.runtimes.get(appId);
    if (!runtime) {
      return false;
    }
    if (runtime.state !== 'active') {
      return true;
    }

    await runtime.lifecycle.deactivate();
    runtime.state = 'suspended';
    this.notify();
    return true;
  }

  /** Disposes a surface and removes its runtime so extension deactivation is complete. */
  public async disposeSurface(appId: WorkbenchAppId): Promise<boolean> {
    const runtime = this.runtimes.get(appId);
    if (!runtime) {
      return false;
    }
    await this.disposeRuntime(appId, runtime, 'unmount');
    return true;
  }

  public getSurfaceState(appId: WorkbenchAppId): AppSurfaceRuntimeState | undefined {
    return this.runtimes.get(appId)?.state;
  }

  /** Removes an app descriptor after its surface and factory have been cleaned. */
  public async unregister(appId: WorkbenchAppId): Promise<boolean> {
    const descriptor = this.descriptors.get(appId);
    if (!descriptor) {
      return false;
    }

    await this.disposeSurface(appId);
    this.factories.delete(appId);
    this.descriptors.delete(appId);
    this.notify();
    return true;
  }

  /** Removes a descriptor during synchronous activation rollback. No runtime may exist yet. */
  public remove(appId: WorkbenchAppId): boolean {
    if (this.runtimes.has(appId)) {
      throw new Error(`WorkbenchApp surface is still active: ${appId}`);
    }
    this.factories.delete(appId);
    const removed = this.descriptors.delete(appId);
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public get(appId: WorkbenchAppId): WorkbenchAppDescriptor | undefined {
    return this.descriptors.get(appId);
  }

  public list(): readonly WorkbenchAppDescriptor[] {
    return [...this.descriptors.values()].sort(
      (left, right) => (left.activityBarOrder ?? 0) - (right.activityBarOrder ?? 0),
    );
  }

  public listActivityBarEntries(): readonly ActivityBarEntry[] {
    return this.list().map((descriptor) => ({
      kind: 'workbench-app',
      appId: descriptor.id,
      title: descriptor.title,
      icon: descriptor.activityBarIcon,
      order: descriptor.activityBarOrder ?? 0,
    }));
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private require(appId: WorkbenchAppId): WorkbenchAppDescriptor {
    const descriptor = this.get(appId);
    if (!descriptor) {
      throw new Error(`Unknown WorkbenchApp: ${appId}`);
    }

    return descriptor;
  }

  private async disposeRuntime(
    appId: WorkbenchAppId,
    runtime: AppSurfaceRuntime,
    reason: 'unmount' | 'crash',
  ): Promise<void> {
    try {
      if (runtime.state === 'active') {
        await runtime.lifecycle.deactivate();
      }
    } finally {
      try {
        await runtime.lifecycle.dispose();
      } finally {
        runtime.state = reason === 'crash' ? 'failed' : 'disposed';
        if (this.runtimes.get(appId) === runtime) {
          this.runtimes.delete(appId);
        }
        this.notify();
      }
    }
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One renderer projection must not prevent other app projections from updating.
      }
    }
  }
}

/** Activity Bar entries for focused tools that do not own a central surface. */
export class ActivityBarContributionRegistry {
  private readonly entries = new Map<string, ActivityBarViewContainerContribution>();

  public register(entry: ActivityBarViewContainerContribution): void {
    if (this.entries.has(entry.id)) {
      throw new Error(`Duplicate Activity Bar contribution id: ${entry.id}`);
    }

    this.entries.set(entry.id, entry);
  }

  public list(): readonly ActivityBarEntry[] {
    return [...this.entries.values()]
      .map((entry) => ({
        ...entry,
        kind: 'view-container' as const,
        order: entry.activityBarOrder ?? 0,
      }))
      .sort((left, right) => left.order - right.order);
  }
}
