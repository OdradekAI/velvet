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
  }

  public bindSurfaceFactory(appId: WorkbenchAppId, factory: AppSurfaceFactory): void {
    this.require(appId);
    this.factories.set(appId, factory);
  }

  public createSurface(appId: WorkbenchAppId, host: AppSurfaceHost): AppSurfaceLifecycle {
    const factory = this.factories.get(appId);
    if (!factory) {
      throw new Error(`WorkbenchApp surface is not activated: ${appId}`);
    }

    return factory(host);
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

  private require(appId: WorkbenchAppId): WorkbenchAppDescriptor {
    const descriptor = this.get(appId);
    if (!descriptor) {
      throw new Error(`Unknown WorkbenchApp: ${appId}`);
    }

    return descriptor;
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
