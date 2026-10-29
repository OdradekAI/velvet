import { useEffect, useState } from 'react';
import type { PluginClient } from '../../workbench/plugins/plugin-client.js';
import type { OverlayRegistry, OverlayInstance } from '../../workbench/overlays/overlay-registry.js';
import type { WorkspaceContext } from '../../workbench/services/workspace-context.js';
import { VelvetDialog } from '../ui/velvet-ui.js';
import { PluginSurfaceHost } from './plugin-surface-host.js';

export interface PluginOverlayHostProps {
  readonly plugins: PluginClient;
  readonly overlays: OverlayRegistry;
  readonly context: WorkspaceContext;
}

/** Renders plugin-owned dialog overlays while keeping placement and dismissal host-owned. */
export function PluginOverlayHost({
  plugins,
  overlays,
  context,
}: PluginOverlayHostProps) {
  const [, rerender] = useState(0);

  useEffect(() => {
    const update = () => rerender((value) => value + 1);
    const unsubscribePlugin = plugins.subscribe(update);
    const unsubscribeOverlays = overlays.subscribe(update);
    return () => {
      unsubscribePlugin();
      unsubscribeOverlays();
    };
  }, [overlays, plugins]);

  const bindings = plugins.listOverlaySurfaceBindings();
  return (
    <>
      {overlays.active().map((instance) => (
        <PluginOverlayInstance
          context={context}
          instance={instance}
          key={instance.instanceId}
          overlays={overlays}
          bindings={bindings}
          plugins={plugins}
        />
      ))}
    </>
  );
}

interface PluginOverlayInstanceProps extends PluginOverlayHostProps {
  readonly instance: OverlayInstance;
  readonly bindings: ReturnType<PluginClient['listOverlaySurfaceBindings']>;
}

function PluginOverlayInstance({
  context,
  instance,
  overlays,
  bindings,
  plugins,
}: PluginOverlayInstanceProps) {
  const binding = bindings.find(
    (candidate) =>
      candidate.overlayId === instance.descriptorId &&
      candidate.namespace === instance.namespace,
  );
  const descriptor = overlays.get(instance.namespace, instance.descriptorId);
  if (!descriptor) {
    return null;
  }

  return (
    <VelvetDialog
      description={binding ? 'Isolated plugin dialog surface.' : 'Plugin dialog surface is waiting for its UI adapter.'}
      onOpenChange={(open) => {
        if (!open) {
          overlays.close(instance.instanceId);
        }
      }}
      open
      title={descriptor.title}
    >
      {binding ? (
        <PluginSurfaceHost
          context={context}
          extensionId={binding.extensionId}
          surfaceId={binding.surfaceId}
          ui={plugins.ui}
        />
      ) : (
        <p className="velvet-view-placeholder">No isolated UI surface is registered.</p>
      )}
    </VelvetDialog>
  );
}
