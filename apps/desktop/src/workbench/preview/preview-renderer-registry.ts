import type { ResolvedPreview } from './preview-host.js';
import {
  createTransientPluginSurfaceSession,
  type PluginSurfaceAdapter,
  type PluginSurfaceDescriptor,
  type PluginSurfaceHandlers,
  type PluginSurfaceSession,
} from '../plugins/ui-bridge.js';
import { createPluginSurfaceAdapter } from '../plugins/surface-adapters.js';

export type PreviewRendererSession = PluginSurfaceSession;

export type PreviewRendererFactory = (
  preview: ResolvedPreview,
  host: HTMLElement,
  handlers: PluginSurfaceHandlers,
) => PreviewRendererSession | undefined;

export type PreviewSurfaceAdapterFactory = (
  descriptor: PluginSurfaceDescriptor,
  host: HTMLElement,
) => PluginSurfaceAdapter;

/**
 * Owns the adapter seam between a resolved preview descriptor and a host
 * surface. PreviewHost resolves data; this registry only creates and disposes
 * the renderer session. That separation keeps renderer failures local to the
 * preview overlay and leaves the resolver reusable by non-DOM clients.
 */
export class PreviewRendererRegistry {
  private readonly factories = new Map<string, PreviewRendererFactory>();

  public constructor(
    private readonly adapterFactory: PreviewSurfaceAdapterFactory = (
      descriptor,
      host,
    ) => createPluginSurfaceAdapter(descriptor, host),
  ) {
    this.factories.set('ui-bridge', (preview, host, handlers) => {
      if (preview.session.renderer.kind !== 'ui-bridge') {
        return undefined;
      }
      const descriptor: PluginSurfaceDescriptor = {
        id: `preview-${preview.session.sessionId}`,
        extensionId: preview.candidate.providerId,
        namespace: 'capability',
        kind: 'inspector',
        entrypoint: preview.session.renderer.entrypoint,
        bridgeVersion: preview.session.renderer.bridgeVersion,
      };
      const adapter = this.adapterFactory(descriptor, host);
      return createTransientPluginSurfaceSession(descriptor, adapter, handlers);
    });
  }

  /** Registers a host-owned renderer kind, returning an isolated cleanup seam. */
  public register(kind: string, factory: PreviewRendererFactory): () => void {
    if (!kind || kind === 'generic' || this.factories.has(kind)) {
      throw new Error(`Duplicate or reserved preview renderer kind: ${kind}`);
    }
    this.factories.set(kind, factory);
    return () => {
      if (this.factories.get(kind) === factory) {
        this.factories.delete(kind);
      }
    };
  }

  public createSession(
    preview: ResolvedPreview,
    host: HTMLElement,
    handlers: PluginSurfaceHandlers = {},
  ): PreviewRendererSession | undefined {
    return this.factories.get(preview.session.renderer.kind)?.(preview, host, handlers);
  }
}
