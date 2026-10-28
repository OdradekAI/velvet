import { LayoutService } from './layout/layout-service.js';
import { PARTS } from './parts/part-registry.js';
import { PluginClient } from './plugins/plugin-client.js';
import { PreviewHost } from './preview/preview-host.js';
import { PreviewRegistry } from './preview/preview-registry.js';
import type { CoreTransport } from './protocol.js';
import { ViewRegistry } from './views/view-registry.js';
import { BUILTIN_WORKSPACES } from '../workspaces/index.js';

export class Workbench {
  public readonly layout: LayoutService;
  public readonly preview: PreviewHost;
  public readonly previewRegistry = new PreviewRegistry();
  public readonly plugins: PluginClient;
  public readonly views = new ViewRegistry();
  public readonly parts = PARTS;
  public readonly workspaces = BUILTIN_WORKSPACES;

  public constructor(core: CoreTransport) {
    this.layout = new LayoutService(core);
    this.preview = new PreviewHost(core);
    this.plugins = new PluginClient(core);
  }
}
