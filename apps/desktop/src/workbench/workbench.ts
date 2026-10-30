import { LayoutService } from './layout/layout-service.js';
import { PARTS } from './parts/part-registry.js';
import { PluginClient } from './plugins/plugin-client.js';
import { JobClient } from './jobs/job-client.js';
import { PreviewHost } from './preview/preview-host.js';
import { PreviewRegistry } from './preview/preview-registry.js';
import type { CoreTransport } from './protocol.js';
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

export class Workbench {
  public readonly layout: LayoutService;
  public readonly preview: PreviewHost;
  public readonly previewRegistry = new PreviewRegistry();
  public readonly plugins: PluginClient;
  public readonly jobs: JobClient;
  public readonly views = new ViewRegistry();
  public readonly apps = new WorkbenchAppRegistry(BUILTIN_WORKBENCH_APPS);
  public readonly activityBar = new ActivityBarContributionRegistry();
  public readonly contributions = new ContributionRegistry();
  public readonly context = new InMemoryWorkspaceContext();
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

  public constructor(core: CoreTransport) {
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
    );
    this.jobs = new JobClient(core);
    this.pluginUi = this.plugins.ui;
    this.registerWorkbenchViews();
    this.registerBuiltInLayoutCommands();
    this.layout.subscribe(() => this.scheduleLayoutContributionSync());
    this.views.subscribe(() => this.scheduleLayoutContributionSync());
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
