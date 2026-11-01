import { LayoutService } from './layout/layout-service.js';
import { PARTS } from './parts/part-registry.js';
import { PluginClient } from './plugins/plugin-client.js';
import { JobClient } from './jobs/job-client.js';
import { PreviewHost } from './preview/preview-host.js';
import { PreviewRegistry } from './preview/preview-registry.js';
import { PreviewRendererRegistry } from './preview/preview-renderer-registry.js';
import type {
  CoreAuthoritativeSnapshot,
  CoreActivationRequest,
  CorePluginSummary,
  CoreTransport,
  CoreTransportStatus,
  SessionAwareCoreTransport,
} from './protocol.js';
import {
  ActivityBarContributionRegistry,
  WorkbenchAppRegistry,
} from './apps/workbench-app.js';
import { ContributionRegistry } from './contributions/contribution-registry.js';
import { CommandRegistry } from './commands/command-registry.js';
import { ContextKeyService } from './commands/context-key-service.js';
import { KeybindingRegistry } from './commands/keybinding-registry.js';
import { MenuRegistry } from './commands/menu-registry.js';
import { InMemoryWorkspaceContext } from './services/workspace-context.js';
import { OverlayRegistry } from './overlays/overlay-registry.js';
import { PluginUiRegistry } from './plugins/ui-bridge.js';
import { ViewRegistry } from './views/view-registry.js';
import { FocusService } from './focus/focus-service.js';
import { BUILTIN_WORKBENCH_APPS } from '../workspaces/index.js';
import { LocalizationService } from './localization/localization-service.js';
import { RecoveryClient } from './recovery/recovery-client.js';
import {
  routeActivationRequest,
  workspaceRoutingIntentKey,
  type WorkspaceRoutingIntent,
} from './activation-routing.js';

export class Workbench {
  public readonly layout: LayoutService;
  public readonly preview: PreviewHost;
  public readonly previewRegistry = new PreviewRegistry();
  public readonly previewRenderers = new PreviewRendererRegistry();
  public readonly plugins: PluginClient;
  public readonly jobs: JobClient;
  public readonly views = new ViewRegistry();
  public readonly apps = new WorkbenchAppRegistry(BUILTIN_WORKBENCH_APPS);
  public readonly activityBar = new ActivityBarContributionRegistry();
  public readonly contributions = new ContributionRegistry();
  public readonly context = new InMemoryWorkspaceContext();
  public readonly localization = new LocalizationService();
  public readonly recovery: RecoveryClient;
  public readonly focus = new FocusService();
  public readonly contextKeys: ContextKeyService;
  public readonly commands: CommandRegistry;
  public readonly keybindings: KeybindingRegistry;
  public readonly menus: MenuRegistry;
  public readonly overlays: OverlayRegistry;
  public readonly pluginUi: PluginUiRegistry;
  public readonly parts = PARTS;
  public readonly workspaces = BUILTIN_WORKBENCH_APPS;
  private layoutSyncScheduled = false;
  private layoutSyncPromise: Promise<void> = Promise.resolve();
  private transportStatusValue: CoreTransportStatus = { state: 'connected' };
  private readonly transportStatusListeners = new Set<(status: CoreTransportStatus) => void>();
  private readonly unsubscribeTransportStatus: (() => void) | undefined;
  private transportRecoveryLifecycle: Promise<void> = Promise.resolve();
  private pendingCoreExtensionSummaries: readonly CorePluginSummary[] | undefined;
  private readonly activationRequestsValue: CoreActivationRequest[] = [];
  private readonly activationListeners = new Set<(request: CoreActivationRequest) => void>();
  private readonly workspaceRoutingIntentsValue: WorkspaceRoutingIntent[] = [];
  private readonly workspaceRoutingListeners = new Set<
    (intent: WorkspaceRoutingIntent) => void
  >();
  private readonly workspaceRoutingKeys = new Set<string>();

  public constructor(core: CoreTransport) {
    this.recovery = new RecoveryClient(core);
    this.localization.bindCore(core);
    this.layout = new LayoutService(core);
    this.preview = new PreviewHost(core);
    this.contextKeys = new ContextKeyService();
    this.commands = new CommandRegistry(this.contextKeys);
    this.keybindings = new KeybindingRegistry(this.commands, this.contextKeys);
    this.menus = new MenuRegistry(this.commands, this.contextKeys);
    this.overlays = new OverlayRegistry(this.contextKeys);
    this.plugins = new PluginClient(
      core,
      this.contributions,
      this.apps,
      '1',
      new PluginUiRegistry(),
      this.views,
      this.commands,
      this.menus,
      this.overlays,
      this.localization,
      this.previewRegistry,
    );
    this.jobs = new JobClient(core);
    this.pluginUi = this.plugins.ui;
    if (isSessionAwareCoreTransport(core)) {
      this.unsubscribeTransportStatus = core.subscribeStatus((status) => {
        this.handleTransportStatus(status);
      });
    }
    this.bindTrustedAppSurfaceFactories();
    this.registerWorkbenchViews();
    this.registerBuiltInLayoutCommands();
    this.layout.subscribe(() => this.scheduleLayoutContributionSync());
    this.views.subscribe(() => this.scheduleLayoutContributionSync());
  }

  public get transportStatus(): CoreTransportStatus {
    return this.transportStatusValue;
  }

  public get isReadOnly(): boolean {
    return this.transportStatusValue.state !== 'connected';
  }

  public subscribeTransportStatus(
    listener: (status: CoreTransportStatus) => void,
  ): () => void {
    this.transportStatusListeners.add(listener);
    listener(this.transportStatusValue);
    return () => this.transportStatusListeners.delete(listener);
  }

  /** Returns validated OS activation requests that have not been consumed by a workspace. */
  public activationRequests(): readonly CoreActivationRequest[] {
    return [...this.activationRequestsValue];
  }

  public acceptActivationRequest(request: CoreActivationRequest): void {
    this.activationRequestsValue.push(request);
    while (this.activationRequestsValue.length > 32) {
      this.activationRequestsValue.shift();
    }
    for (const listener of this.activationListeners) {
      try {
        listener(request);
      } catch {
        // One activation consumer must not block other workspaces.
      }
    }
    for (const intent of routeActivationRequest(request)) {
      const key = workspaceRoutingIntentKey(intent);
      if (this.workspaceRoutingKeys.has(key)) {
        continue;
      }
      this.workspaceRoutingKeys.add(key);
      this.workspaceRoutingIntentsValue.push(intent);
      while (this.workspaceRoutingIntentsValue.length > 32) {
        const removed = this.workspaceRoutingIntentsValue.shift();
        if (removed) {
          this.workspaceRoutingKeys.delete(workspaceRoutingIntentKey(removed));
        }
      }
      for (const listener of this.workspaceRoutingListeners) {
        try {
          listener(intent);
        } catch {
          // One workspace must not block other routing consumers.
        }
      }
    }
  }

  public subscribeActivation(listener: (request: CoreActivationRequest) => void): () => void {
    this.activationListeners.add(listener);
    return () => this.activationListeners.delete(listener);
  }

  /** Returns host-owned routing intents that have not been consumed by a workspace. */
  public workspaceRoutingIntents(): readonly WorkspaceRoutingIntent[] {
    return [...this.workspaceRoutingIntentsValue];
  }

  public subscribeWorkspaceRouting(
    listener: (intent: WorkspaceRoutingIntent) => void,
  ): () => void {
    this.workspaceRoutingListeners.add(listener);
    return () => this.workspaceRoutingListeners.delete(listener);
  }

  /**
   * Lets a workspace explicitly consume one pending routing intent. The host
   * activates the target workspace first, then delegates the intent. A failed
   * consumer leaves the intent pending so a later retry can recover it.
   */
  public async consumeWorkspaceRoutingIntent(
    intent: WorkspaceRoutingIntent,
    consumer: (intent: WorkspaceRoutingIntent) => void | Promise<void>,
  ): Promise<boolean> {
    if (intent.kind === 'unrouted-deep-link' || !this.hasWorkspaceRoutingIntent(intent)) {
      return false;
    }
    if (!this.layout.snapshot()) {
      return false;
    }

    try {
      await this.layout.activateWorkspace(intent.workspaceId);
      this.context.apply({ kind: 'activate-app', appId: intent.workspaceId });
      await consumer(intent);
    } catch {
      return false;
    }

    const index = this.workspaceRoutingIntentsValue.findIndex(
      (candidate) => workspaceRoutingIntentKey(candidate) === workspaceRoutingIntentKey(intent),
    );
    if (index >= 0) {
      this.workspaceRoutingIntentsValue.splice(index, 1);
      this.workspaceRoutingKeys.delete(workspaceRoutingIntentKey(intent));
    }
    return true;
  }

  /** Applies one Rust-owned snapshot without replaying renderer mutations. */
  public applyAuthoritativeSnapshot(snapshot: CoreAuthoritativeSnapshot): void {
    this.layout.acceptSnapshot(snapshot.layout);
    this.localization.acceptProfile(snapshot.profile);
    this.recovery.acceptSnapshot(snapshot.recovery);
    this.jobs.acceptSnapshots(snapshot.jobs);
    this.plugins.acceptCoreExtensions(snapshot.extensions);
    this.pendingCoreExtensionSummaries = snapshot.extensions;
  }

  public dispose(): void {
    this.unsubscribeTransportStatus?.();
    this.transportStatusListeners.clear();
    this.activationListeners.clear();
    this.workspaceRoutingListeners.clear();
    this.preview.dispose();
  }

  private hasWorkspaceRoutingIntent(intent: WorkspaceRoutingIntent): boolean {
    return this.workspaceRoutingKeys.has(workspaceRoutingIntentKey(intent));
  }

  private handleTransportStatus(status: CoreTransportStatus): void {
    this.transportStatusValue = status;
    for (const listener of this.transportStatusListeners) {
      try {
        listener(status);
      } catch {
        // A status projection must not interfere with recovery sequencing.
      }
    }

    if (status.state === 'resyncing' || status.state === 'degraded' || status.state === 'disconnected') {
      this.transportRecoveryLifecycle = this.transportRecoveryLifecycle
        .then(async () => {
          await this.pluginUi.suspendForTransportRecovery();
          this.preview.suspendForTransportRecovery();
        })
        .catch(() => undefined);
      return;
    }

    if (status.state === 'connected') {
      this.transportRecoveryLifecycle = this.transportRecoveryLifecycle
        .then(async () => {
          const summaries = this.pendingCoreExtensionSummaries;
          this.pendingCoreExtensionSummaries = undefined;
          if (summaries) {
            await this.plugins.reconcileCoreExtensions(summaries);
          }
          this.pluginUi.resumeAfterTransportRecovery();
          this.preview.resumeAfterTransportRecovery();
        })
        .catch(() => undefined);
    }
  }

  private registerWorkbenchViews(): void {
    for (const app of this.apps.list()) {
      for (const container of app.viewContainers ?? []) {
        this.views.registerContainer(container);
      }
      for (const view of app.views ?? []) {
        this.views.register(view);
      }
    }
  }

  /**
   * Built-in apps render through the trusted React adapter, but still enter
   * the same registry lifecycle as isolated plugin AppSurfaces.
   */
  private bindTrustedAppSurfaceFactories(): void {
    for (const app of this.apps.list()) {
      if (app.surface.trust !== 'trusted') {
        continue;
      }
      this.apps.bindSurfaceFactory(app.id, () => ({
        mount: () => undefined,
        activate: () => undefined,
        deactivate: () => undefined,
        dispose: () => undefined,
      }));
    }
  }

  private registerBuiltInLayoutCommands(): void {
    const commands = [
      {
        id: 'workbench.toggle-primary-sidebar',
        title: 'Toggle Primary Sidebar',
        keybinding: 'ctrl+b',
        part: 'primary-sidebar' as const,
      },
      {
        id: 'workbench.toggle-operations-panel',
        title: 'Toggle Operations Panel',
        keybinding: 'ctrl+j',
        part: 'operations-panel' as const,
      },
    ];

    for (const descriptor of commands) {
      this.commands.register({
        id: descriptor.id,
        title: descriptor.title,
        scope: 'workbench',
        namespace: 'workbench',
        execute: async () => {
          await this.layout.apply({ id: 'workbench.toggle-part', part: descriptor.part });
        },
      });
      this.keybindings.register({
        id: `${descriptor.id}.keybinding`,
        commandId: descriptor.id,
        keybinding: descriptor.keybinding,
        scope: 'workbench',
        namespace: 'workbench',
      });
    }
  }

  /**
   * Registers descriptor defaults with Rust after a layout snapshot exists.
   * Existing entries are left untouched so user visibility, ordering and moves
   * remain durable even when a renderer or extension is restarted.
   */
  private scheduleLayoutContributionSync(): void {
    if (this.layoutSyncScheduled) {
      return;
    }
    this.layoutSyncScheduled = true;
    queueMicrotask(() => {
      this.layoutSyncScheduled = false;
      this.layoutSyncPromise = this.layoutSyncPromise
        .then(() => this.syncLayoutContributions())
        .catch(() => undefined);
    });
  }

  private async syncLayoutContributions(): Promise<void> {
    let snapshot = this.layout.snapshot();
    if (!snapshot) {
      return;
    }

    for (const container of this.views.listContainers()) {
      if (snapshot.containers[container.id]) {
        continue;
      }
      snapshot = await this.layout.upsertViewContainer(
        container.id,
        container.defaultVisible !== false,
        container.order ?? 0,
      );
    }

    for (const view of this.views.list()) {
      if (snapshot.views[view.id]) {
        continue;
      }
      if (!snapshot.containers[view.containerId]) {
        continue;
      }
      snapshot = await this.layout.upsertView(
        view.id,
        view.containerId,
        view.defaultVisible !== false,
        view.order ?? 0,
      );
    }
  }
}

function isSessionAwareCoreTransport(core: CoreTransport): core is SessionAwareCoreTransport {
  return 'subscribeStatus' in core && typeof core.subscribeStatus === 'function';
}
