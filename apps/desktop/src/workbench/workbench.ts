import { LayoutService } from './layout/layout-service.js';
import { PARTS } from './parts/part-registry.js';
import { PluginClient } from './plugins/plugin-client.js';
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
import { MenuRegistry } from './commands/menu-registry.js';
import { InMemoryWorkspaceContext } from './services/workspace-context.js';
import { OverlayRegistry } from './overlays/overlay-registry.js';
import { PluginUiRegistry } from './plugins/ui-bridge.js';
import { ViewRegistry } from './views/view-registry.js';
import { BUILTIN_WORKBENCH_APPS } from '../workspaces/index.js';

export class Workbench {
  public readonly layout: LayoutService;
  public readonly preview: PreviewHost;
  public readonly previewRegistry = new PreviewRegistry();
  public readonly plugins: PluginClient;
  public readonly views = new ViewRegistry();
  public readonly apps = new WorkbenchAppRegistry(BUILTIN_WORKBENCH_APPS);
  public readonly activityBar = new ActivityBarContributionRegistry();
  public readonly contributions = new ContributionRegistry();
  public readonly context = new InMemoryWorkspaceContext();
  public readonly contextKeys: ContextKeyService;
  public readonly commands: CommandRegistry;
  public readonly menus: MenuRegistry;
  public readonly overlays: OverlayRegistry;
  public readonly pluginUi: PluginUiRegistry;
  public readonly parts = PARTS;
  public readonly workspaces = BUILTIN_WORKBENCH_APPS;

  public constructor(core: CoreTransport) {
    this.layout = new LayoutService(core);
    this.preview = new PreviewHost(core);
    this.plugins = new PluginClient(core, this.contributions, this.apps);
    this.contextKeys = new ContextKeyService();
    this.commands = new CommandRegistry(this.contextKeys);
    this.menus = new MenuRegistry(this.commands, this.contextKeys);
    this.overlays = new OverlayRegistry(this.contextKeys);
    this.pluginUi = this.plugins.ui;
  }
}
