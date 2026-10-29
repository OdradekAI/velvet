import type { ViewRenderer } from './view-renderer-registry.js';
import { PluginSurfaceHost } from './plugin-surface-host.js';
import type { PluginViewSurfaceBinding } from '../../workbench/plugins/plugin-client.js';
import type { WorkspaceContext } from '../../workbench/services/workspace-context.js';
import type { PluginUiRegistry, UiThemeSnapshot } from '../../workbench/plugins/ui-bridge.js';

export function createPluginViewRenderer(
  binding: PluginViewSurfaceBinding,
  ui: PluginUiRegistry,
  context: WorkspaceContext,
  theme?: UiThemeSnapshot,
): ViewRenderer {
  return () => (
    <PluginSurfaceHost
      context={context}
      extensionId={binding.extensionId}
      surfaceId={binding.surfaceId}
      theme={theme}
      ui={ui}
    />
  );
}
