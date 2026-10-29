import type { CoreTransport, Part, Zone } from '../protocol.js';
import {
  ContributionRegistry,
  namespaceFor,
  type PluginUiContribution,
  type ContributionDescriptor,
} from '../contributions/contribution-registry.js';
import {
  WorkbenchAppRegistry,
} from '../apps/workbench-app.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import { canUsePermission } from '../zones/zone-policy.js';
import {
  PluginUiRegistry,
  type PluginSurfaceDescriptor,
} from './ui-bridge.js';
import {
  ViewRegistry,
  type ViewContainerDescriptor,
  type ViewContainerScope,
  type ViewDescriptor,
} from '../views/view-registry.js';

export type PluginRuntime = 'builtin' | 'wasm' | 'sidecar';

export interface AppTarget {
  readonly appId: WorkbenchAppId;
  readonly apiRange: string;
}

export type ManifestContribution = Omit<
  ContributionDescriptor,
  'extensionId' | 'namespace'
> & {
  readonly namespace?: string;
};

export interface ExtensionManifest {
  readonly id: string;
  readonly version: string;
  readonly hostApi: string;
  readonly runtime?: PluginRuntime;
  readonly targetApp?: AppTarget;
  readonly contributions: readonly ManifestContribution[];
  readonly permissions: Partial<Record<Zone, readonly string[]>>;
}

export type PluginStatus = 'registered' | 'active' | 'disabled' | 'failed';

export interface PluginSummary {
  readonly id: string;
  readonly version: string;
  readonly status: PluginStatus;
  readonly error?: string;
}

interface PluginRecord {
  readonly manifest: ExtensionManifest;
  status: PluginStatus;
  registered: ContributionDescriptor[];
  registeredViewContainers: string[];
  registeredViews: string[];
  registeredSurfaceIds: string[];
  viewSurfaceBindings: PluginViewSurfaceBinding[];
  error?: string;
}

export interface PluginViewSurfaceBinding {
  readonly viewId: string;
  readonly extensionId: string;
  readonly surfaceId: string;
  readonly surface: PluginSurfaceDescriptor;
}

export class PluginLifecycleError extends Error {
  public constructor(
    message: string,
    public readonly extensionId: string,
    public readonly cause?: unknown,
  ) {
    super(message);
    this.name = 'PluginLifecycleError';
  }
}

export class PluginClient {
  private readonly records = new Map<string, PluginRecord>();
  public readonly ui: PluginUiRegistry;

  public constructor(
    private readonly _core: CoreTransport,
    private readonly contributionRegistry: ContributionRegistry = new ContributionRegistry(),
    private readonly appRegistry: WorkbenchAppRegistry = new WorkbenchAppRegistry(),
    private readonly hostApiVersion = '1',
    ui: PluginUiRegistry = new PluginUiRegistry(),
    private readonly viewRegistry: ViewRegistry = new ViewRegistry(),
  ) {
    this.ui = ui;
  }

  private readonly listeners = new Set<() => void>();

  public register(manifest: ExtensionManifest): void {
    if (this.records.has(manifest.id)) {
      throw new PluginLifecycleError(
        `Extension is already registered: ${manifest.id}`,
        manifest.id,
      );
    }

    this.validateManifest(manifest);
    this.records.set(manifest.id, {
      manifest,
      status: 'registered',
      registered: [],
      registeredViewContainers: [],
      registeredViews: [],
      registeredSurfaceIds: [],
      viewSurfaceBindings: [],
    });
    this.notify();
  }

  public activate(extensionId: string): void {
    const record = this.require(extensionId);
    if (record.status === 'active') {
      return;
    }

    const registered: ContributionDescriptor[] = [];
    const registeredViewContainers: string[] = [];
    const registeredViews: string[] = [];
    const registeredSurfaceIds: string[] = [];
    const viewSurfaceBindings: PluginViewSurfaceBinding[] = [];
    try {
      const descriptors = record.manifest.contributions.map((contribution) =>
        this.toDescriptor(record.manifest, contribution),
      );

      for (const descriptor of descriptors) {
        this.contributionRegistry.register(descriptor);
        registered.push(descriptor);
      }

      for (const descriptor of descriptors.filter((candidate) => candidate.kind === 'view-container')) {
        const container = this.toViewContainer(descriptor);
        this.viewRegistry.registerContainer(container);
        registeredViewContainers.push(container.id);
      }

      for (const descriptor of descriptors.filter((candidate) => isViewContribution(candidate.kind))) {
        const container = this.toSyntheticViewContainer(descriptor);
        if (container && !this.viewRegistry.getContainer(container.id)) {
          this.viewRegistry.registerContainer(container);
          registeredViewContainers.push(container.id);
        }

        const view = this.toView(descriptor);
        this.viewRegistry.register(view);
        registeredViews.push(view.id);
      }

      for (const descriptor of descriptors) {
        if (!descriptor.ui) {
          continue;
        }

        const surface = this.toSurfaceDescriptor(descriptor);
        this.ui.register(surface);
        registeredSurfaceIds.push(surface.id);
        if (isViewContribution(descriptor.kind) && surface.kind === 'view') {
          viewSurfaceBindings.push({
            viewId: contributedId(descriptor.extensionId, descriptor.id),
            extensionId: descriptor.extensionId,
            surfaceId: surface.id,
            surface,
          });
        }
      }

      record.registered = registered;
      record.registeredViewContainers = registeredViewContainers;
      record.registeredViews = registeredViews;
      record.registeredSurfaceIds = registeredSurfaceIds;
      record.viewSurfaceBindings = viewSurfaceBindings;
      record.status = 'active';
      record.error = undefined;
      this.notify();
    } catch (cause) {
      this.rollbackActivation(
        registered,
        registeredViewContainers,
        registeredViews,
        registeredSurfaceIds,
      );
      record.registered = [];
      record.registeredViewContainers = [];
      record.registeredViews = [];
      record.registeredSurfaceIds = [];
      record.viewSurfaceBindings = [];
      record.status = 'failed';
      record.error = cause instanceof Error ? cause.message : String(cause);
      this.notify();
      throw new PluginLifecycleError(
        `Extension activation failed: ${extensionId}`,
        extensionId,
        cause,
      );
    }
  }

  public async deactivate(
    extensionId: string,
    _reason: 'user' | 'crash' | 'incompatible' = 'user',
  ): Promise<void> {
    const record = this.require(extensionId);
    await this.ui.disposeExtension(extensionId, _reason === 'user' ? 'unmount' : _reason);
    for (const viewId of [...record.registeredViews].reverse()) {
      this.viewRegistry.unregister(viewId);
    }
    for (const containerId of [...record.registeredViewContainers].reverse()) {
      this.viewRegistry.unregisterContainer(containerId);
    }
    for (const contribution of [...record.registered].reverse()) {
      this.contributionRegistry.unregister(contribution);
    }
    record.registered = [];
    record.registeredViewContainers = [];
    record.registeredViews = [];
    record.registeredSurfaceIds = [];
    record.viewSurfaceBindings = [];
    record.status = 'disabled';
    this.notify();
  }

  public async unregister(extensionId: string): Promise<boolean> {
    const record = this.records.get(extensionId);
    if (!record) {
      return false;
    }

    if (record.status === 'active') {
      await this.deactivate(extensionId);
    }
    const removed = this.records.delete(extensionId);
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public get(extensionId: string): PluginSummary | undefined {
    const record = this.records.get(extensionId);
    if (!record) {
      return undefined;
    }

    return this.summary(record);
  }

  public discover(): readonly PluginSummary[] {
    return [...this.records.values()].map((record) => this.summary(record));
  }

  public listVisible(activeAppId?: WorkbenchAppId): readonly ContributionDescriptor[] {
    const activeExtensions = new Set(
      [...this.records.values()]
        .filter((record) => record.status === 'active')
        .map((record) => record.manifest.id),
    );

    const visible = activeAppId === undefined
      ? this.contributionRegistry.list().filter(
          (contribution) => contribution.scope !== 'workbench-app',
        )
      : this.contributionRegistry.list({ activeAppId });

    return visible
      .filter((contribution) => activeExtensions.has(contribution.extensionId));
  }

  public listViewSurfaceBindings(): readonly PluginViewSurfaceBinding[] {
    return [...this.records.values()]
      .filter((record) => record.status === 'active')
      .flatMap((record) => record.viewSurfaceBindings);
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Compatibility check retained for the original zone-only skeleton. */
  public validateZone(manifest: ExtensionManifest): void {
    for (const contribution of manifest.contributions) {
      if (contribution.zone && !['navigation', 'contextual', 'operations'].includes(contribution.zone)) {
        throw new Error(`Unsupported extension zone: ${contribution.zone}`);
      }

      for (const permission of manifest.permissions[contribution.zone ?? 'navigation'] ?? []) {
        if (contribution.zone && !canUsePermission(contribution.zone, permission)) {
          throw new Error(
            `Permission ${permission} is not allowed in zone ${contribution.zone}`,
          );
        }
      }
    }
  }

  private validateManifest(manifest: ExtensionManifest): void {
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(manifest.id)) {
      throw new PluginLifecycleError(`Invalid extension id: ${manifest.id}`, manifest.id);
    }

    if (!apiRangeMatches(manifest.hostApi, this.hostApiVersion)) {
      throw new PluginLifecycleError(
        `Extension requires host API ${manifest.hostApi}; host provides ${this.hostApiVersion}`,
        manifest.id,
      );
    }

    this.validateZone(manifest);
    const appIds = new Set<WorkbenchAppId>();
    const viewContainerIds = new Set<string>();
    for (const contribution of manifest.contributions) {
      if (contribution.scope === 'workbench-app') {
        if (!contribution.appId) {
          throw new PluginLifecycleError(
            `App contribution ${contribution.id} must declare an app id`,
            manifest.id,
          );
        }
        appIds.add(contribution.appId);
      }

      if (contribution.kind === 'view-container') {
        if (!contribution.title || !contribution.icon) {
          throw new PluginLifecycleError(
            `ViewContainer ${contribution.id} must declare title and icon metadata`,
            manifest.id,
          );
        }
        viewContainerIds.add(contribution.id);
      }

      if (isViewContribution(contribution.kind)) {
        if (!contribution.title || !contribution.icon) {
          throw new PluginLifecycleError(
            `View ${contribution.id} must declare title and icon metadata`,
            manifest.id,
          );
        }
        if (contribution.kind === 'view' && !contribution.viewContainerId) {
          throw new PluginLifecycleError(
            `View ${contribution.id} must declare a viewContainerId`,
            manifest.id,
          );
        }
        if (contribution.ui && contribution.ui.kind !== 'view') {
          throw new PluginLifecycleError(
            `View ${contribution.id} must bind a view UI surface`,
            manifest.id,
          );
        }
        if (contribution.kind === 'view' && !contribution.ui) {
          throw new PluginLifecycleError(
            `View ${contribution.id} must declare an isolated UI surface`,
            manifest.id,
          );
        }
      }

      if (contribution.ui) {
        this.validateUiContribution(manifest.id, contribution, contribution.ui);
      }
    }

    for (const contribution of manifest.contributions) {
      if (
        contribution.kind === 'view' &&
        contribution.viewContainerId &&
        !viewContainerIds.has(contribution.viewContainerId)
      ) {
        throw new PluginLifecycleError(
          `View ${contribution.id} references an unknown ViewContainer ${contribution.viewContainerId}`,
          manifest.id,
        );
      }

      if (contribution.kind === 'view' && contribution.viewContainerId) {
        const target = manifest.contributions.find(
          (candidate) =>
            candidate.kind === 'view-container' &&
            candidate.id === contribution.viewContainerId,
        );
        if (
          target &&
          (target.scope !== contribution.scope || target.appId !== contribution.appId)
        ) {
          throw new PluginLifecycleError(
            `View ${contribution.id} must use a ViewContainer in the same namespace`,
            manifest.id,
          );
        }
      }
    }

    if (appIds.size > 1) {
      throw new PluginLifecycleError(
        'One extension cannot target multiple WorkbenchApp namespaces',
        manifest.id,
      );
    }

    if (manifest.targetApp) {
      const app = this.appRegistry.get(manifest.targetApp.appId);
      if (!app) {
        throw new PluginLifecycleError(
          `Target WorkbenchApp is not installed: ${manifest.targetApp.appId}`,
          manifest.id,
        );
      }
      if (!apiRangeMatches(manifest.targetApp.apiRange, app.apiVersion ?? '1')) {
        throw new PluginLifecycleError(
          `Target WorkbenchApp ${manifest.targetApp.appId} is incompatible with ${manifest.targetApp.apiRange}`,
          manifest.id,
        );
      }
      if (appIds.size === 1 && !appIds.has(manifest.targetApp.appId)) {
        throw new PluginLifecycleError(
          `Contribution namespace does not match target WorkbenchApp ${manifest.targetApp.appId}`,
          manifest.id,
        );
      }
    } else if (appIds.size === 1) {
      throw new PluginLifecycleError(
        'WorkbenchApp contributions require an explicit targetApp declaration',
        manifest.id,
      );
    }
  }

  private toDescriptor(
    manifest: ExtensionManifest,
    contribution: ManifestContribution,
  ): ContributionDescriptor {
    const { namespace: declaredNamespace, ...descriptor } = contribution;
    const namespace = namespaceFor(
      descriptor.scope,
      descriptor.appId,
      manifest.id,
    );
    if (declaredNamespace && declaredNamespace !== namespace) {
      throw new PluginLifecycleError(
        `Contribution ${descriptor.id} declares an invalid namespace`,
        manifest.id,
      );
    }

    return {
      ...descriptor,
      extensionId: manifest.id,
      namespace,
    };
  }

  private toViewContainer(descriptor: ContributionDescriptor): ViewContainerDescriptor {
    if (descriptor.kind !== 'view-container') {
      throw new PluginLifecycleError(
        `Contribution ${descriptor.id} is not a ViewContainer`,
        descriptor.extensionId,
      );
    }

    return {
      id: contributedId(descriptor.extensionId, descriptor.id),
      title: descriptor.title!,
      titleKey: descriptor.titleKey,
      icon: descriptor.icon!,
      zone: descriptor.zone!,
      preferredPart: descriptor.container as Part,
      scope: toViewScope(descriptor),
      order: descriptor.order,
      defaultVisible: descriptor.defaultVisible,
      enablement: descriptor.enablement,
      requiredCapabilities: descriptor.requiredCapabilities,
      activityBar: descriptor.activityBar,
    };
  }

  private toSyntheticViewContainer(
    descriptor: ContributionDescriptor,
  ): ViewContainerDescriptor | undefined {
    if (descriptor.kind !== 'companion-view' && descriptor.kind !== 'operation-view') {
      return undefined;
    }

    return {
      id: `${contributedId(descriptor.extensionId, descriptor.id)}.container`,
      title: descriptor.title!,
      titleKey: descriptor.titleKey,
      icon: descriptor.icon!,
      zone: descriptor.kind === 'companion-view' ? 'contextual' : 'operations',
      preferredPart: descriptor.container as Part,
      scope: { kind: 'workbench' },
      order: descriptor.order,
      defaultVisible: descriptor.defaultVisible,
      enablement: descriptor.enablement,
      requiredCapabilities: descriptor.requiredCapabilities,
    };
  }

  private toView(descriptor: ContributionDescriptor): ViewDescriptor {
    if (!isViewContribution(descriptor.kind)) {
      throw new PluginLifecycleError(
        `Contribution ${descriptor.id} is not a view`,
        descriptor.extensionId,
      );
    }

    const id = contributedId(descriptor.extensionId, descriptor.id);
    const containerId = descriptor.kind === 'view'
      ? contributedId(descriptor.extensionId, descriptor.viewContainerId!)
      : `${id}.container`;
    const zone = descriptor.kind === 'companion-view'
      ? 'contextual'
      : descriptor.kind === 'operation-view'
        ? 'operations'
        : descriptor.zone!;

    return {
      id,
      containerId,
      title: descriptor.title!,
      titleKey: descriptor.titleKey,
      icon: descriptor.icon!,
      zone,
      scope: toViewScope(descriptor),
      order: descriptor.order,
      defaultVisible: descriptor.defaultVisible,
      enablement: descriptor.enablement,
      activation: descriptor.activation,
      requiredCapabilities: descriptor.requiredCapabilities,
    };
  }

  private toSurfaceDescriptor(descriptor: ContributionDescriptor): PluginSurfaceDescriptor {
    const ui = descriptor.ui!;
    return {
      id: descriptor.id,
      extensionId: descriptor.extensionId,
      namespace: descriptor.namespace,
      kind: ui.kind,
      entrypoint: ui.entrypoint,
      bridgeVersion: ui.bridgeVersion,
      ...(descriptor.appId ? { appId: descriptor.appId } : {}),
    };
  }

  private validateUiContribution(
    extensionId: string,
    contribution: ManifestContribution,
    ui: PluginUiContribution,
  ): void {
    if (!ui.entrypoint.trim()) {
      throw new PluginLifecycleError(
        `Contribution ${contribution.id} must declare a UI entrypoint`,
        extensionId,
      );
    }
    if (!ui.bridgeVersion.trim()) {
      throw new PluginLifecycleError(
        `Contribution ${contribution.id} must declare a UI bridge version`,
        extensionId,
      );
    }
    if (contribution.scope === 'capability') {
      throw new PluginLifecycleError(
        `Capability ${contribution.id} cannot register a UI surface`,
        extensionId,
      );
    }

    const expectedKind = expectedSurfaceKind(contribution.kind);
    if (!expectedKind) {
      throw new PluginLifecycleError(
        `${contribution.kind} ${contribution.id} cannot own a UI surface in this host version`,
        extensionId,
      );
    }
    if (expectedKind && expectedKind !== ui.kind) {
      throw new PluginLifecycleError(
        `${contribution.kind} ${contribution.id} must use a ${expectedKind} UI surface`,
        extensionId,
      );
    }
  }

  private rollbackActivation(
    registered: readonly ContributionDescriptor[],
    registeredViewContainers: readonly string[],
    registeredViews: readonly string[],
    registeredSurfaceIds: readonly string[],
  ): void {
    for (const surfaceId of [...registeredSurfaceIds].reverse()) {
      this.ui.unregister(registered[0]?.extensionId ?? '', surfaceId);
    }
    for (const viewId of [...registeredViews].reverse()) {
      this.viewRegistry.unregister(viewId);
    }
    for (const containerId of [...registeredViewContainers].reverse()) {
      this.viewRegistry.unregisterContainer(containerId);
    }
    for (const contribution of [...registered].reverse()) {
      this.contributionRegistry.unregister(contribution);
    }
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One renderer projection must not prevent other plugin projections from updating.
      }
    }
  }

  private require(extensionId: string): PluginRecord {
    const record = this.records.get(extensionId);
    if (!record) {
      throw new PluginLifecycleError(
        `Extension is not registered: ${extensionId}`,
        extensionId,
      );
    }

    return record;
  }

  private summary(record: PluginRecord): PluginSummary {
    return {
      id: record.manifest.id,
      version: record.manifest.version,
      status: record.status,
      ...(record.error ? { error: record.error } : {}),
    };
  }
}

function isViewContribution(
  kind: ContributionDescriptor['kind'],
): boolean {
  return kind === 'view' || kind === 'companion-view' || kind === 'operation-view';
}

function expectedSurfaceKind(
  kind: ContributionDescriptor['kind'],
): PluginUiContribution['kind'] | undefined {
  switch (kind) {
    case 'view':
    case 'companion-view':
    case 'operation-view':
      return 'view';
    case 'app-surface':
      return 'app-surface';
    case 'dialog':
      return 'dialog';
    case 'inspector-section':
      return 'inspector';
    default:
      return undefined;
  }
}

function toViewScope(descriptor: ContributionDescriptor): ViewContainerScope {
  if (descriptor.scope === 'workbench') {
    return { kind: 'workbench' };
  }
  if (descriptor.scope === 'workbench-app' && descriptor.appId) {
    return { kind: 'workbench-app', appId: descriptor.appId };
  }
  throw new PluginLifecycleError(
    `Capability ${descriptor.id} cannot register a Workbench view`,
    descriptor.extensionId,
  );
}

function contributedId(extensionId: string, contributionId: string): string {
  return `${extensionId}.${contributionId}`;
}

function apiRangeMatches(range: string, version: string): boolean {
  const normalizedRange = range.trim();
  if (normalizedRange === '*' || normalizedRange === version) {
    return true;
  }

  if (normalizedRange.startsWith('^')) {
    return normalizedRange.slice(1).split('.')[0] === version.split('.')[0];
  }

  return false;
}
