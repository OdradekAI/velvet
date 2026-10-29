import type { Part, Zone } from '../protocol.js';

export type ViewContainerScope =
  | { readonly kind: 'workbench' }
  | { readonly kind: 'workbench-app'; readonly appId: string };

export interface ViewContainerActivityBarEntry {
  readonly title: string;
  readonly icon: string;
  readonly order?: number;
}

export interface ViewContainerDescriptor {
  readonly id: string;
  readonly title: string;
  readonly titleKey?: string;
  readonly icon: string;
  readonly zone: Zone;
  readonly preferredPart: Part;
  readonly scope: ViewContainerScope;
  readonly order?: number;
  readonly defaultVisible?: boolean;
  readonly enablement?: string;
  readonly requiredCapabilities?: readonly string[];
  readonly activityBar?: ViewContainerActivityBarEntry;
}

export interface ViewDescriptor {
  readonly id: string;
  readonly containerId: string;
  readonly title: string;
  readonly titleKey?: string;
  readonly icon: string;
  readonly zone: Zone;
  readonly scope: ViewContainerScope;
  readonly order?: number;
  readonly defaultVisible?: boolean;
  readonly enablement?: string;
  readonly activation?: 'eager' | 'on-demand';
  readonly requiredCapabilities?: readonly string[];
}

export type ViewAction = 'hide' | 'show' | 'reorder' | 'move' | 'collapse' | 'focus';

export const VIEW_ACTIONS: readonly ViewAction[] = [
  'hide',
  'show',
  'reorder',
  'move',
  'collapse',
  'focus',
];

const GLOBAL_ZONE_PARTS: Record<Zone, Part> = {
  navigation: 'primary-sidebar',
  contextual: 'auxiliary-sidebar',
  operations: 'operations-panel',
};

const APP_ZONE_PARTS: Record<Zone, readonly Part[]> = {
  navigation: ['primary-sidebar'],
  contextual: ['workspace-area'],
  operations: ['workspace-area'],
};

export class ViewRegistry {
  private readonly containers = new Map<string, ViewContainerDescriptor>();
  private readonly views = new Map<string, ViewDescriptor>();
  private readonly listeners = new Set<() => void>();

  public registerContainer(container: ViewContainerDescriptor): void {
    if (this.containers.has(container.id)) {
      throw new Error(`Duplicate view container id: ${container.id}`);
    }

    validateViewContainer(container);
    this.containers.set(container.id, container);
    this.notify();
  }

  public getContainer(id: string): ViewContainerDescriptor | undefined {
    return this.containers.get(id);
  }

  /** Removes a container after its registered views have been removed. */
  public unregisterContainer(id: string): boolean {
    if ([...this.views.values()].some((view) => view.containerId === id)) {
      throw new Error(`View container still has registered views: ${id}`);
    }

    const removed = this.containers.delete(id);
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public listContainers(appId?: string): ViewContainerDescriptor[] {
    return [...this.containers.values()]
      .filter((container) => {
        if (container.scope.kind === 'workbench') {
          return true;
        }

        return appId === undefined || container.scope.appId === appId;
      })
      .sort((left, right) => (left.order ?? 0) - (right.order ?? 0));
  }

  public register(view: ViewDescriptor): void {
    if (this.views.has(view.id)) {
      throw new Error(`Duplicate view id: ${view.id}`);
    }

    const container = this.getContainer(view.containerId);
    if (!container) {
      throw new Error(`Unknown view container: ${view.containerId}`);
    }

    validateView(view, container);
    this.views.set(view.id, view);
    this.notify();
  }

  public get(id: string): ViewDescriptor | undefined {
    return this.views.get(id);
  }

  public unregister(id: string): boolean {
    const removed = this.views.delete(id);
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public list(zone?: Zone, appId?: string): ViewDescriptor[] {
    return [...this.views.values()]
      .filter((view) => {
        if (zone && view.zone !== zone) {
          return false;
        }

        return isScopeVisible(view.scope, appId);
      })
      .sort((left, right) => (left.order ?? 0) - (right.order ?? 0));
  }

  public canMoveView(viewId: string, targetContainerId: string): boolean {
    const view = this.get(viewId);
    const container = this.getContainer(targetContainerId);
    if (!view || !container) {
      return false;
    }

    return isCompatibleContainer(view, container);
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One renderer projection must not prevent other view projections from updating.
      }
    }
  }
}

export function validateViewContainer(container: ViewContainerDescriptor): void {
  const allowedParts = allowedPartsFor(container.scope, container.zone);
  if (!allowedParts.includes(container.preferredPart)) {
    throw new Error(
      `ViewContainer ${container.id} cannot use ${container.preferredPart} in ${container.zone} zone`,
    );
  }

  if (container.activityBar && container.zone !== 'navigation') {
    throw new Error(
      `ViewContainer ${container.id} may expose an Activity Bar entry only in the navigation zone`,
    );
  }

  if (container.activityBar && container.preferredPart !== 'primary-sidebar') {
    throw new Error(
      `Activity Bar ViewContainer ${container.id} must target the Primary Sidebar`,
    );
  }
}

export function validateView(
  view: ViewDescriptor,
  container: ViewContainerDescriptor,
): void {
  if (view.zone !== container.zone) {
    throw new Error(
      `View ${view.id} must remain in its container's ${container.zone} zone`,
    );
  }

  if (!sameScope(view.scope, container.scope)) {
    throw new Error(`View ${view.id} cannot register in another scope's container`);
  }
}

export function isCompatibleContainer(
  view: ViewDescriptor,
  container: ViewContainerDescriptor,
): boolean {
  return view.zone === container.zone && sameScope(view.scope, container.scope);
}

function allowedPartsFor(
  scope: ViewContainerScope,
  zone: Zone,
): readonly Part[] {
  if (scope.kind === 'workbench') {
    return [GLOBAL_ZONE_PARTS[zone]];
  }

  return APP_ZONE_PARTS[zone];
}

function sameScope(left: ViewContainerScope, right: ViewContainerScope): boolean {
  if (left.kind === 'workbench' || right.kind === 'workbench') {
    return left.kind === right.kind;
  }

  return left.appId === right.appId;
}

function isScopeVisible(scope: ViewContainerScope, appId: string | undefined): boolean {
  if (scope.kind === 'workbench') {
    return true;
  }

  return appId === undefined || scope.appId === appId;
}
