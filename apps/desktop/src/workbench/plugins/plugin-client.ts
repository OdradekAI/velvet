import {
  PROTOCOL_VERSION,
  type CorePluginContribution,
  type CorePluginProjection,
  type CorePluginDisableReason,
  type CorePluginManifest,
  type CorePluginSummary,
  type CoreTransport,
  type Part,
  type Zone,
} from '../../platform/protocol.js';
import {
  ContributionRegistry,
  namespaceFor,
  type PluginUiContribution,
  type ContributionDescriptor,
} from '../contributions/contribution-registry.js';
import {
  type AppSurfaceHost,
  type AppSurfaceLifecycle,
  type WorkbenchAppDescriptor,
  WorkbenchAppRegistry,
} from '../apps/workbench-app.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import { canUsePermission } from '../zones/zone-policy.js';
import {
  asAppSurfaceLifecycle,
  type PluginSurfaceMountOptions,
  PluginUiRegistry,
  type PluginSurfaceDescriptor,
} from './ui-bridge.js';
import {
  ViewRegistry,
  type ViewContainerDescriptor,
  type ViewContainerScope,
  type ViewDescriptor,
} from '../views/view-registry.js';
import {
  CommandRegistry,
  type CommandDescriptor,
  type CommandExecutionContext,
  type CommandScope,
} from '../commands/command-registry.js';
import {
  MenuRegistry,
  type MenuContribution,
  type MenuLocation,
} from '../commands/menu-registry.js';
import {
  OverlayRegistry,
  type OverlayDescriptor,
} from '../overlays/overlay-registry.js';

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
  /** A plugin-owned top-level WorkbenchApp. Its id must be extension-prefixed. */
  readonly workbenchApp?: PluginWorkbenchApp;
  readonly targetApp?: AppTarget;
  readonly contributions: readonly ManifestContribution[];
  readonly permissions: Partial<Record<Zone, readonly string[]>>;
}

export interface PluginWorkbenchApp {
  readonly id: WorkbenchAppId;
  readonly apiVersion?: string;
  readonly title: string;
  readonly activityBarIcon: string;
  readonly activityBarOrder?: number;
  readonly surface: {
    readonly entrypoint: string;
    readonly bridgeVersion: string;
  };
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
  registeredCommands: CommandDescriptor[];
  registeredMenus: MenuContribution[];
  registeredOverlays: OverlayDescriptor[];
  overlaySurfaceBindings: PluginOverlaySurfaceBinding[];
  registeredAppId?: WorkbenchAppId;
  appSurfaceBinding?: PluginAppSurfaceBinding;
  error?: string;
}

export interface PluginViewSurfaceBinding {
  readonly viewId: string;
  readonly extensionId: string;
  readonly surfaceId: string;
  readonly surface: PluginSurfaceDescriptor;
}

export interface PluginOverlaySurfaceBinding {
  readonly overlayId: string;
  readonly namespace: string;
  readonly extensionId: string;
  readonly surfaceId: string;
  readonly surface: PluginSurfaceDescriptor;
}

export interface PluginAppSurfaceBinding {
  readonly appId: WorkbenchAppId;
  readonly extensionId: string;
  readonly surfaceId: string;
  readonly surface: PluginSurfaceDescriptor;
}

export type PluginCommandHandler = (
  commandId: string,
  args: unknown,
  context: CommandExecutionContext,
) => void | Promise<void>;

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
  private readonly coreSummaries = new Map<string, CorePluginSummary>();
  private readonly coreProjections = new Map<string, CorePluginProjection>();
  private readonly commandHandlers = new Map<string, PluginCommandHandler>();
  public readonly ui: PluginUiRegistry;

  public constructor(
    private readonly core: CoreTransport,
    private readonly contributionRegistry: ContributionRegistry = new ContributionRegistry(),
    private readonly appRegistry: WorkbenchAppRegistry = new WorkbenchAppRegistry(),
    private readonly hostApiVersion = '1',
    ui: PluginUiRegistry = new PluginUiRegistry(),
    private readonly viewRegistry: ViewRegistry = new ViewRegistry(),
    private readonly commandRegistry?: CommandRegistry,
    private readonly menuRegistry?: MenuRegistry,
    private readonly overlayRegistry?: OverlayRegistry,
  ) {
    this.ui = ui;
    this.core.subscribe((event) => {
      if (event.kind === 'plugin-disabled') {
        void this.handleCorePluginDisabled(event.extensionId, event.reason);
      }
    });
  }

  private readonly listeners = new Set<() => void>();

  public register(manifest: ExtensionManifest): void {
    if (this.records.has(manifest.id)) {
      throw new PluginLifecycleError(
        `Extension is already registered: ${manifest.id}`,
        manifest.id,
      );
    }

    const normalizedManifest = this.normalizeManifest(manifest);
    this.validateManifest(normalizedManifest);
    this.records.set(manifest.id, {
      manifest: normalizedManifest,
      status: 'registered',
      registered: [],
      registeredViewContainers: [],
      registeredViews: [],
      registeredSurfaceIds: [],
      viewSurfaceBindings: [],
      registeredCommands: [],
      registeredMenus: [],
      registeredOverlays: [],
      overlaySurfaceBindings: [],
      registeredAppId: undefined,
      appSurfaceBinding: undefined,
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
    const registeredCommands: CommandDescriptor[] = [];
    const registeredMenus: MenuContribution[] = [];
    const registeredOverlays: OverlayDescriptor[] = [];
    const overlaySurfaceBindings: PluginOverlaySurfaceBinding[] = [];
    let registeredAppId: WorkbenchAppId | undefined;
    let appSurfaceBinding: PluginAppSurfaceBinding | undefined;
    try {
      if (record.manifest.workbenchApp) {
        const app = this.toWorkbenchAppDescriptor(record.manifest);
        this.appRegistry.register(app);
        registeredAppId = app.id;
      }

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

      for (const descriptor of descriptors.filter((candidate) => candidate.kind === 'command')) {
        if (!this.commandRegistry) {
          throw new PluginLifecycleError(
            `Command registry is unavailable for ${descriptor.id}`,
            descriptor.extensionId,
          );
        }
        const command = this.toCommandDescriptor(descriptor);
        this.commandRegistry.register(command);
        registeredCommands.push(command);
      }

      for (const descriptor of descriptors.filter(isMenuContribution)) {
        if (!this.menuRegistry) {
          throw new PluginLifecycleError(
            `Menu registry is unavailable for ${descriptor.id}`,
            descriptor.extensionId,
          );
        }
        const menu = this.toMenuContribution(descriptor);
        this.menuRegistry.register(menu);
        registeredMenus.push(menu);
      }

      for (const descriptor of descriptors.filter((candidate) => candidate.kind === 'dialog')) {
        if (!this.overlayRegistry) {
          throw new PluginLifecycleError(
            `Overlay registry is unavailable for ${descriptor.id}`,
            descriptor.extensionId,
          );
        }
        const overlay = this.toOverlayDescriptor(descriptor);
        this.overlayRegistry.register(overlay);
        registeredOverlays.push(overlay);
      }

      for (const descriptor of descriptors) {
        if (!descriptor.ui || descriptor.kind === 'app-surface') {
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
        if (descriptor.kind === 'dialog' && surface.kind === 'dialog') {
          overlaySurfaceBindings.push({
            overlayId: descriptor.id,
            namespace: descriptor.namespace,
            extensionId: descriptor.extensionId,
            surfaceId: surface.id,
            surface,
          });
        }
      }

      if (record.manifest.workbenchApp) {
        const surface = this.toAppSurfaceDescriptor(record.manifest);
        this.ui.register(surface);
        registeredSurfaceIds.push(surface.id);
        appSurfaceBinding = {
          appId: record.manifest.workbenchApp.id,
          extensionId: record.manifest.id,
          surfaceId: surface.id,
          surface,
        };
        this.appRegistry.bindSurfaceFactory(
          record.manifest.workbenchApp.id,
          (host) => this.createAppSurfaceLifecycle(record.manifest, host, surface),
        );
      }

      record.registered = registered;
      record.registeredViewContainers = registeredViewContainers;
      record.registeredViews = registeredViews;
      record.registeredSurfaceIds = registeredSurfaceIds;
      record.viewSurfaceBindings = viewSurfaceBindings;
      record.registeredCommands = registeredCommands;
      record.registeredMenus = registeredMenus;
      record.registeredOverlays = registeredOverlays;
      record.overlaySurfaceBindings = overlaySurfaceBindings;
      record.registeredAppId = registeredAppId;
      record.appSurfaceBinding = appSurfaceBinding;
      record.status = 'active';
      record.error = undefined;
      this.notify();
    } catch (cause) {
      this.rollbackActivation(
        extensionId,
        registered,
        registeredViewContainers,
        registeredViews,
        registeredSurfaceIds,
        registeredCommands,
        registeredMenus,
        registeredOverlays,
      );
      if (registeredAppId) {
        this.appRegistry.remove(registeredAppId);
      }
      record.registered = [];
      record.registeredViewContainers = [];
      record.registeredViews = [];
      record.registeredSurfaceIds = [];
      record.viewSurfaceBindings = [];
      record.registeredCommands = [];
      record.registeredMenus = [];
      record.registeredOverlays = [];
      record.overlaySurfaceBindings = [];
      record.registeredAppId = undefined;
      record.appSurfaceBinding = undefined;
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
    if (record.registeredAppId) {
      await this.appRegistry.unregister(record.registeredAppId);
    }
    for (const overlay of [...record.registeredOverlays].reverse()) {
      this.overlayRegistry?.unregister(overlay);
    }
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
    for (const menu of [...record.registeredMenus].reverse()) {
      this.menuRegistry?.unregister(menu);
    }
    for (const command of [...record.registeredCommands].reverse()) {
      this.commandRegistry?.unregister(command);
    }
    record.registered = [];
    record.registeredViewContainers = [];
    record.registeredViews = [];
    record.registeredSurfaceIds = [];
    record.viewSurfaceBindings = [];
    record.registeredCommands = [];
    record.registeredMenus = [];
    record.registeredOverlays = [];
    record.overlaySurfaceBindings = [];
    record.registeredAppId = undefined;
    record.appSurfaceBinding = undefined;
    this.commandHandlers.delete(extensionId);
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
      this.commandHandlers.delete(extensionId);
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

  /** Queries the Rust-owned plugin inventory without importing runtime state into the UI registry. */
  public async queryCoreExtensions(): Promise<readonly CorePluginSummary[]> {
    const response = await this.core.request({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'query-extensions',
    });
    if (response.kind !== 'extensions') {
      throw new PluginLifecycleError(
        `Expected an extension list response, received ${response.kind}`,
        'core',
      );
    }
    this.coreSummaries.clear();
    for (const summary of response.extensions) {
      this.coreSummaries.set(summary.id, summary);
    }
    this.notify();
    return response.extensions;
  }

  public getCoreSummary(extensionId: string): CorePluginSummary | undefined {
    return this.coreSummaries.get(extensionId);
  }

  public getCoreProjection(extensionId: string): CorePluginProjection | undefined {
    return this.coreProjections.get(extensionId);
  }

  /** Reads the Rust-declared contributions for one extension without mutating UI registries. */
  public async queryCoreProjection(extensionId: string): Promise<CorePluginProjection> {
    const response = await this.core.request({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'query-plugin-projection',
      extensionId,
    });
    if (response.kind !== 'plugin-projection') {
      throw new PluginLifecycleError(
        `Expected a plugin projection response, received ${response.kind}`,
        extensionId,
      );
    }
    this.coreProjections.set(extensionId, response.projection);
    this.coreSummaries.set(extensionId, response.projection.summary);
    this.notify();
    return response.projection;
  }

  /** Projects Rust-declared views and UI surfaces through the same host registries as local plugins. */
  public async applyCoreProjection(projection: CorePluginProjection): Promise<void> {
    const manifest = manifestFromCoreProjection(projection);
    const existing = this.records.get(projection.summary.id);
    if (!existing) {
      this.register(manifest);
    }

    this.coreProjections.set(projection.summary.id, projection);
    this.coreSummaries.set(projection.summary.id, projection.summary);

    if (projection.summary.state === 'active') {
      if (this.records.get(projection.summary.id)?.status !== 'active') {
        this.activate(projection.summary.id);
      }
      return;
    }

    const record = this.records.get(projection.summary.id);
    if (record?.status === 'active') {
      await this.deactivate(
        projection.summary.id,
        projection.summary.state === 'failed' ? 'crash' : 'user',
      );
    }
    if (record && projection.summary.state === 'failed') {
      record.status = 'failed';
      record.error = `Rust extension state: ${projection.summary.state}`;
      this.notify();
    } else if (record && projection.summary.state === 'disabled') {
      record.status = 'disabled';
      record.error = undefined;
      this.notify();
    } else if (record && projection.summary.state === 'registered') {
      record.status = 'registered';
      record.error = undefined;
      this.notify();
    }
  }

  /** Registers a manifest with Rust and synchronizes its host-owned projection. */
  public async registerInCore(manifest: CorePluginManifest): Promise<CorePluginSummary> {
    const summary = await this.requestCoreExtension({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'register-plugin',
      manifest,
    });
    await this.syncCoreProjectionState(summary);
    return summary;
  }

  public async activateInCore(extensionId: string): Promise<CorePluginSummary> {
    const summary = await this.requestCoreExtension({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'activate-plugin',
      extensionId,
    });
    await this.syncCoreProjectionState(summary);
    return summary;
  }

  public async disableInCore(
    extensionId: string,
    reason: CorePluginDisableReason = 'user',
  ): Promise<CorePluginSummary> {
    const summary = await this.requestCoreExtension({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'disable-plugin',
      extensionId,
      reason,
    });
    await this.syncCoreProjectionState(summary);
    return summary;
  }

  public async checkCoreHealth(extensionId: string): Promise<CorePluginSummary> {
    const summary = await this.requestCoreExtension({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'check-plugin-health',
      extensionId,
    });
    await this.syncCoreProjectionState(summary);
    return summary;
  }

  public async recoverInCore(extensionId: string): Promise<CorePluginSummary> {
    const summary = await this.requestCoreExtension({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'recover-plugin',
      extensionId,
    });
    await this.syncCoreProjectionState(summary);
    return summary;
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

  public listOverlaySurfaceBindings(): readonly PluginOverlaySurfaceBinding[] {
    return [...this.records.values()]
      .filter((record) => record.status === 'active')
      .flatMap((record) => record.overlaySurfaceBindings);
  }

  public listAppSurfaceBindings(): readonly PluginAppSurfaceBinding[] {
    return [...this.records.values()]
      .filter((record) => record.status === 'active' && record.appSurfaceBinding)
      .map((record) => record.appSurfaceBinding!);
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Binds the isolated runtime handler used when a host menu invokes a plugin command. */
  public bindCommandHandler(
    extensionId: string,
    handler: PluginCommandHandler,
  ): () => void {
    this.require(extensionId);
    this.commandHandlers.set(extensionId, handler);
    return () => {
      if (this.commandHandlers.get(extensionId) === handler) {
        this.commandHandlers.delete(extensionId);
      }
    };
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
    const commandIds = new Set(
      manifest.contributions
        .filter((contribution) => contribution.kind === 'command')
        .map((contribution) => contribution.id),
    );
    if (manifest.workbenchApp) {
      if (
        !isStableId(manifest.workbenchApp.id) ||
        !manifest.workbenchApp.id.startsWith(`${manifest.id}.`)
      ) {
        throw new PluginLifecycleError(
          `WorkbenchApp ${manifest.workbenchApp.id} must be owned by extension ${manifest.id}`,
          manifest.id,
        );
      }
      if (!manifest.workbenchApp.title || !manifest.workbenchApp.activityBarIcon) {
        throw new PluginLifecycleError(
          `WorkbenchApp ${manifest.workbenchApp.id} must declare title and Activity Bar icon metadata`,
          manifest.id,
        );
      }
      if (!manifest.workbenchApp.surface.entrypoint.trim()) {
        throw new PluginLifecycleError(
          `WorkbenchApp ${manifest.workbenchApp.id} must declare an AppSurface entrypoint`,
          manifest.id,
        );
      }
      if (!manifest.workbenchApp.surface.bridgeVersion.trim()) {
        throw new PluginLifecycleError(
          `WorkbenchApp ${manifest.workbenchApp.id} must declare an AppSurface bridge version`,
          manifest.id,
        );
      }
    }
    for (const contribution of manifest.contributions) {
      const expectedNamespace = namespaceFor(
        contribution.scope,
        contribution.appId,
        manifest.id,
      );
      if (contribution.namespace && contribution.namespace !== expectedNamespace) {
        throw new PluginLifecycleError(
          `Contribution ${contribution.id} declares an invalid namespace`,
          manifest.id,
        );
      }

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

      if (contribution.kind === 'command' && !contribution.title) {
        throw new PluginLifecycleError(
          `Command ${contribution.id} must declare title metadata`,
          manifest.id,
        );
      }

      if (isMenuContribution(contribution)) {
        if (!contribution.label && !contribution.title) {
          throw new PluginLifecycleError(
            `Menu ${contribution.id} must declare label metadata`,
            manifest.id,
          );
        }
        if (!contribution.commandId || !commandIds.has(contribution.commandId)) {
          throw new PluginLifecycleError(
            `Menu ${contribution.id} must reference a command from the same extension`,
            manifest.id,
          );
        }
      }

      if (contribution.kind === 'dialog' && !contribution.title) {
        throw new PluginLifecycleError(
          `Dialog ${contribution.id} must declare title metadata`,
          manifest.id,
        );
      }

      if (contribution.kind === 'app-surface') {
        if (!contribution.title || !contribution.icon || !contribution.ui) {
          throw new PluginLifecycleError(
            `AppSurface ${contribution.id} must declare title, icon and UI metadata`,
            manifest.id,
          );
        }
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

    if (manifest.workbenchApp && appIds.size === 1 && !appIds.has(manifest.workbenchApp.id)) {
      throw new PluginLifecycleError(
        `Contribution namespace does not match contributed WorkbenchApp ${manifest.workbenchApp.id}`,
        manifest.id,
      );
    }

    if (manifest.workbenchApp && manifest.targetApp) {
      throw new PluginLifecycleError(
        'A full WorkbenchApp cannot also target another WorkbenchApp',
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
    } else if (!manifest.workbenchApp && appIds.size === 1) {
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

  private normalizeManifest(manifest: ExtensionManifest): ExtensionManifest {
    const appSurface = manifest.contributions.find(
      (contribution) => contribution.kind === 'app-surface',
    );
    if (!appSurface) {
      return manifest;
    }
    if (manifest.workbenchApp) {
      throw new PluginLifecycleError(
        'A WorkbenchApp cannot declare its AppSurface twice',
        manifest.id,
      );
    }
    if (
      appSurface.scope !== 'workbench-app' ||
      !appSurface.appId ||
      !appSurface.title ||
      !appSurface.icon ||
      !appSurface.ui ||
      appSurface.ui.kind !== 'app-surface'
    ) {
      throw new PluginLifecycleError(
        `AppSurface ${appSurface.id} must declare a workbench-app id, title, icon and app-surface UI`,
        manifest.id,
      );
    }

    return {
      ...manifest,
      workbenchApp: {
        id: appSurface.appId,
        title: appSurface.title,
        activityBarIcon: appSurface.icon,
        surface: {
          entrypoint: appSurface.ui.entrypoint,
          bridgeVersion: appSurface.ui.bridgeVersion,
        },
      },
    };
  }

  private toWorkbenchAppDescriptor(
    manifest: ExtensionManifest,
  ): WorkbenchAppDescriptor {
    const app = manifest.workbenchApp!;
    return {
      id: app.id,
      apiVersion: app.apiVersion ?? this.hostApiVersion,
      title: app.title,
      activityBarIcon: app.activityBarIcon,
      activityBarOrder: app.activityBarOrder,
      surface: {
        trust: 'isolated',
        entrypoint: app.surface.entrypoint,
        bridgeVersion: app.surface.bridgeVersion,
      },
    };
  }

  private toAppSurfaceDescriptor(
    manifest: ExtensionManifest,
  ): PluginSurfaceDescriptor {
    const app = manifest.workbenchApp!;
    const contribution = manifest.contributions.find(
      (candidate) => candidate.kind === 'app-surface',
    );
    return {
      id: contribution?.id ?? appSurfaceId(app.id),
      extensionId: manifest.id,
      namespace: `workbench.app.${app.id}`,
      kind: 'app-surface',
      entrypoint: app.surface.entrypoint,
      bridgeVersion: app.surface.bridgeVersion,
      appId: app.id,
    };
  }

  private createAppSurfaceLifecycle(
    manifest: ExtensionManifest,
    host: AppSurfaceHost,
    surface: PluginSurfaceDescriptor,
  ): AppSurfaceLifecycle {
    const session = this.ui.createSession(manifest.id, surface.id);
    const lifecycle = asAppSurfaceLifecycle(session, {
      context: host.context.snapshot(),
      theme: { scheme: 'dark', revision: 0, tokens: {} },
      size: { width: 0, height: 0 },
    } satisfies PluginSurfaceMountOptions);
    let unsubscribe: (() => void) | undefined;
    return {
      mount: async () => {
        unsubscribe = host.context.subscribe((snapshot) => {
          if (
            session.state === 'mounted' ||
            session.state === 'active' ||
            session.state === 'suspended'
          ) {
            session.updateContext(snapshot);
          }
        });
        try {
          await lifecycle.mount();
        } catch (cause) {
          unsubscribe?.();
          unsubscribe = undefined;
          throw cause;
        }
      },
      activate: lifecycle.activate,
      deactivate: lifecycle.deactivate,
      dispose: async () => {
        unsubscribe?.();
        unsubscribe = undefined;
        await lifecycle.dispose();
      },
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

  private toCommandDescriptor(descriptor: ContributionDescriptor): CommandDescriptor {
    if (descriptor.kind !== 'command') {
      throw new PluginLifecycleError(
        `Contribution ${descriptor.id} is not a command`,
        descriptor.extensionId,
      );
    }

    return {
      id: descriptor.id,
      title: descriptor.title!,
      titleKey: descriptor.titleKey,
      scope: descriptor.scope as CommandScope,
      namespace: descriptor.namespace,
      appId: descriptor.appId,
      when: descriptor.when,
      order: descriptor.order,
      execute: async (args, context) => {
        const handler = this.commandHandlers.get(descriptor.extensionId);
        if (handler) {
          await handler(descriptor.id, args, context);
          return;
        }

        if (this.coreProjections.has(descriptor.extensionId)) {
          await this.invokeCoreCommand(descriptor.extensionId, descriptor.id, args, context);
          return;
        }

        throw new PluginLifecycleError(
          `Plugin command handler is not bound: ${descriptor.id}`,
          descriptor.extensionId,
        );
      },
    };
  }

  private toMenuContribution(descriptor: ContributionDescriptor): MenuContribution {
    if (!isMenuContribution(descriptor)) {
      throw new PluginLifecycleError(
        `Contribution ${descriptor.id} is not a context menu`,
        descriptor.extensionId,
      );
    }

    return {
      id: descriptor.id,
      label: descriptor.label ?? descriptor.title!,
      labelKey: descriptor.labelKey ?? descriptor.titleKey,
      location: 'context-menu' satisfies MenuLocation,
      commandId: descriptor.commandId!,
      scope: descriptor.scope as CommandScope,
      namespace: descriptor.namespace,
      appId: descriptor.appId,
      group: descriptor.group,
      order: descriptor.order,
      when: descriptor.when,
      anchor: descriptor.anchor,
    };
  }

  private toOverlayDescriptor(descriptor: ContributionDescriptor): OverlayDescriptor {
    if (descriptor.kind !== 'dialog') {
      throw new PluginLifecycleError(
        `Contribution ${descriptor.id} is not a dialog`,
        descriptor.extensionId,
      );
    }

    return {
      id: descriptor.id,
      title: descriptor.title!,
      kind: 'dialog',
      scope: descriptor.scope as OverlayDescriptor['scope'],
      namespace: descriptor.namespace,
      appId: descriptor.appId,
      modal: true,
      dismissOnEscape: true,
      restoreFocus: true,
      when: descriptor.when,
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
    extensionId: string,
    registered: readonly ContributionDescriptor[],
    registeredViewContainers: readonly string[],
    registeredViews: readonly string[],
    registeredSurfaceIds: readonly string[],
    registeredCommands: readonly CommandDescriptor[],
    registeredMenus: readonly MenuContribution[],
    registeredOverlays: readonly OverlayDescriptor[],
  ): void {
    for (const surfaceId of [...registeredSurfaceIds].reverse()) {
      this.ui.unregister(extensionId, surfaceId);
    }
    for (const viewId of [...registeredViews].reverse()) {
      this.viewRegistry.unregister(viewId);
    }
    for (const containerId of [...registeredViewContainers].reverse()) {
      this.viewRegistry.unregisterContainer(containerId);
    }
    for (const overlay of [...registeredOverlays].reverse()) {
      this.overlayRegistry?.unregister(overlay);
    }
    for (const menu of [...registeredMenus].reverse()) {
      this.menuRegistry?.unregister(menu);
    }
    for (const command of [...registeredCommands].reverse()) {
      this.commandRegistry?.unregister(command);
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

  private async requestCoreExtension(request: Parameters<CoreTransport['request']>[0]): Promise<CorePluginSummary> {
    const response = await this.core.request(request);
    if (response.kind !== 'extension') {
      throw new PluginLifecycleError(
        `Expected an extension response, received ${response.kind}`,
        'core',
      );
    }
    this.coreSummaries.set(response.extension.id, response.extension);
    this.notify();
    return response.extension;
  }

  private async invokeCoreCommand(
    extensionId: string,
    commandId: string,
    args: unknown,
    context: CommandExecutionContext,
  ): Promise<void> {
    const response = await this.core.request({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'invoke-plugin-command',
      extensionId,
      commandId,
      args,
      context: {
        activeAppId: context.activeAppId ?? null,
        keys: context.keys,
      },
    });
    if (response.kind !== 'accepted') {
      throw new PluginLifecycleError(
        `Expected an accepted response for plugin command ${commandId}, received ${response.kind}`,
        extensionId,
      );
    }
  }

  private async handleCorePluginDisabled(
    extensionId: string,
    reason: string,
  ): Promise<void> {
    try {
      const projection = await this.queryCoreProjection(extensionId);
      await this.applyCoreProjection(projection);
    } catch {
      const record = this.records.get(extensionId);
      if (record?.status === 'active') {
        await this.deactivate(extensionId, reason === 'crash' ? 'crash' : 'user');
      }
    }
  }

  private async syncCoreProjectionState(summary: CorePluginSummary): Promise<void> {
    let projection = this.coreProjections.get(summary.id);
    if (summary.state === 'disabled' || summary.state === 'failed') {
      if (projection) {
        await this.applyCoreProjection({ ...projection, summary });
      } else if (this.records.get(summary.id)?.status === 'active') {
        await this.deactivate(summary.id, summary.state === 'failed' ? 'crash' : 'user');
      }
      return;
    }

    if (!projection || summary.state === 'active') {
      projection = await this.queryCoreProjection(summary.id);
    } else {
      projection = { ...projection, summary };
    }
    await this.applyCoreProjection(projection);
  }
}

function manifestFromCoreProjection(projection: CorePluginProjection): ExtensionManifest {
  return {
    id: projection.summary.id,
    version: projection.summary.version,
    hostApi: '1',
    runtime: 'builtin',
    targetApp: projection.targetApp
      ? { appId: projection.targetApp.appId, apiRange: projection.targetApp.api }
      : undefined,
    contributions: projection.contributions
      .filter(isRenderableCoreContribution)
      .map(coreContributionToManifest),
    permissions: {},
  };
}

function isRenderableCoreContribution(contribution: CorePluginContribution): boolean {
  return (
    contribution.kind === 'view-container' ||
    contribution.kind === 'view' ||
    contribution.kind === 'companion-view' ||
    contribution.kind === 'operation-view' ||
    contribution.kind === 'dialog' ||
    contribution.kind === 'command' ||
    contribution.kind === 'context-menu'
  );
}

function coreContributionToManifest(contribution: CorePluginContribution): ManifestContribution {
  const scope = contribution.scope.kind === 'workbench-app' ? 'workbench-app' : contribution.scope.kind;
  const appId = contribution.scope.kind === 'workbench-app'
    ? contribution.scope.appId
    : undefined;
  const ui = contribution.ui
    ? {
        kind: contribution.ui.kind,
        entrypoint: contribution.ui.entrypoint,
        bridgeVersion: contribution.ui.bridgeVersion,
      }
    : undefined;

  return {
    id: contribution.id,
    scope,
    appId,
    kind: coreContributionKind(contribution),
    namespace: contribution.namespace,
    container: contribution.container ?? undefined,
    zone: contribution.zone ?? undefined,
    title: contribution.title ?? contribution.id,
    titleKey: contribution.titleKey ?? undefined,
    icon: contribution.icon ?? 'extension',
    order: contribution.order ?? undefined,
    defaultVisible: contribution.defaultVisible ?? undefined,
    viewContainerId: contribution.viewContainerId ?? undefined,
    activityBar: contribution.activityBar
      ? {
          title: contribution.activityBar.title,
          icon: contribution.activityBar.icon,
          order: contribution.activityBar.order ?? undefined,
        }
      : undefined,
    label: contribution.label ?? undefined,
    labelKey: contribution.labelKey ?? undefined,
    commandId: contribution.commandId ?? undefined,
    group: contribution.group ?? undefined,
    when: contribution.when ?? undefined,
    anchor: contribution.anchor ?? undefined,
    ui,
  };
}

function coreContributionKind(
  contribution: CorePluginContribution,
): ManifestContribution['kind'] {
  switch (contribution.kind) {
    case 'context-menu':
      return contribution.scope.kind === 'workbench-app' ? 'app-context-menu' : 'context-menu';
    case 'inspector':
      return 'inspector-section';
    default:
      return contribution.kind;
  }
}

function isViewContribution(
  kind: ContributionDescriptor['kind'],
): boolean {
  return kind === 'view' || kind === 'companion-view' || kind === 'operation-view';
}

function isMenuContribution(
  contribution: Pick<ContributionDescriptor, 'kind'>,
): boolean {
  return contribution.kind === 'context-menu' || contribution.kind === 'app-context-menu';
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

function appSurfaceId(appId: WorkbenchAppId): string {
  return `${appId}.surface`;
}

function isStableId(value: string): boolean {
  return /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value);
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
