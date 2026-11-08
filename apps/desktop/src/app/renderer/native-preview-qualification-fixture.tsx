import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type {
  CoreEvent,
  CoreRequest,
  CoreResolvedPreview,
  CoreResponse,
  CoreTransport,
} from '../../platform/protocol.js';
import { CommandRegistry } from '../../workbench/commands/command-registry.js';
import { ContextKeyService } from '../../workbench/commands/context-key-service.js';
import { JobClient } from '../../workbench/jobs/job-client.js';
import { PreviewHost, type PreviewContext } from '../../workbench/preview/preview-host.js';
import { PreviewRegistry } from '../../workbench/preview/preview-registry.js';
import { PreviewRendererRegistry } from '../../workbench/preview/preview-renderer-registry.js';
import { InMemoryWorkspaceContext } from '../../workbench/services/workspace-context.js';
import { PreviewOverlay } from './preview-overlay.js';

interface NativePreviewProbe {
  attempts: number;
  releases: string[];
  providerMounts: number;
  providerDisposals: number;
}

const PREVIEW_CONTEXT: PreviewContext = {
  assetId: 'native-asset-1',
  contentKind: 'image',
  intent: 'quick',
  surface: 'workspace',
};

/**
 * Browser-only fixture for the native Workbench preview seam. It keeps the
 * Workbench PreviewHost and renderer registries real while replacing only the
 * CoreTransport and the UI Bridge adapter with observable fakes.
 */
export function NativePreviewQualificationFixture() {
  const probeRef = useRef<NativePreviewProbe>({
    attempts: 0,
    releases: [],
    providerMounts: 0,
    providerDisposals: 0,
  });
  const [, rerender] = useState(0);
  const notify = useCallback(() => rerender((value) => value + 1), []);
  const core = useMemo(() => createFakePreviewCore(probeRef.current, notify), [notify]);
  const host = useMemo(() => new PreviewHost(core), [core]);
  const registry = useMemo(() => {
    const next = new PreviewRegistry();
    next.registerProvider({
      id: 'native.image',
      extensionId: 'native.preview',
      contentKinds: ['image'],
      intents: ['quick'],
      surfaces: ['workspace'],
      priority: 100,
    });
    next.registerAugmentation({
      id: 'native.metadata',
      extensionId: 'native.preview',
      contentKinds: ['image'],
      slot: 'metadata',
    });
    return next;
  }, []);
  const rendererRegistry = useMemo(
    () => new PreviewRendererRegistry((_descriptor, mountHost) => ({
      mount(endpoint) {
        probeRef.current.providerMounts += 1;
        const marker = document.createElement('div');
        marker.dataset.nativeProviderRenderer = 'true';
        marker.textContent = 'Native provider renderer mounted';
        mountHost.replaceChildren(marker);
        endpoint.receive({
          type: 'surface.ready',
          bridgeVersion: '1',
          componentContractVersion: 1,
        });
        notify();
      },
      unmount() {
        probeRef.current.providerDisposals += 1;
        mountHost.replaceChildren();
        notify();
      },
    })),
    [notify],
  );
  const workspaceContext = useMemo(() => new InMemoryWorkspaceContext(), []);
  const contextKeys = useMemo(() => new ContextKeyService(), []);
  const commands = useMemo(() => new CommandRegistry(contextKeys), [contextKeys]);
  const jobs = useMemo(() => new JobClient(core), [core]);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const unsubscribe = host.subscribe(notify);
    return () => {
      unsubscribe();
    };
  }, [host, jobs, notify]);

  const probe = probeRef.current;
  return (
    <main
      aria-label="Native Workbench preview qualification fixture"
      data-ui-kit-fixture="native-preview"
      data-ui-kit-viewport="1280x800"
      style={{ display: 'grid', gap: 16, padding: 24, minHeight: '100%', background: 'var(--velvet-surface-app)' }}
    >
      <h1>Native Workbench preview qualification</h1>
      <p>Real PreviewHost + PreviewOverlay with an observable fake CoreTransport.</p>
      <button data-ui-test-id="native-preview-open" onClick={() => setOpen(true)} type="button">
        Open native preview
      </button>
      <section aria-label="Native preview observations" data-native-preview-observations>
        <output data-native-preview-observation="host-state">{host.snapshot.state}</output>
        <output data-native-preview-observation="attempts">{probe.attempts}</output>
        <output data-native-preview-observation="released-sessions">{probe.releases.join(',')}</output>
        <output data-native-preview-observation="provider-renderer">{probe.providerMounts}</output>
        <output data-native-preview-observation="provider-disposals">{probe.providerDisposals}</output>
      </section>
      <PreviewOverlay
        context={PREVIEW_CONTEXT}
        host={host}
        commands={commands}
        contextKeys={contextKeys}
        jobs={jobs}
        onOpenChange={setOpen}
        open={open}
        registry={registry}
        rendererRegistry={rendererRegistry}
        workspaceContext={workspaceContext}
      />
    </main>
  );
}

function createFakePreviewCore(
  probe: NativePreviewProbe,
  notify: () => void,
): CoreTransport {
  const listeners = new Set<(event: CoreEvent) => void>();
  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    async request(request: CoreRequest): Promise<CoreResponse> {
      if (request.kind === 'resolve-preview') {
        probe.attempts += 1;
        notify();
        if (probe.attempts === 1) {
          throw new Error('native preview provider unavailable');
        }
        return { kind: 'preview', preview: resolvedPreview() };
      }
      if (request.kind === 'release-preview-session') {
        probe.releases.push(request.sessionId);
        notify();
        return { kind: 'accepted' };
      }
      return { kind: 'accepted' };
    },
  };
}

function resolvedPreview(): CoreResolvedPreview {
  return {
    candidate: {
      providerId: 'native.image',
      contentKinds: ['image'],
      intents: ['quick'],
      surfaces: ['workspace'],
      priority: 100,
    },
    session: {
      providerId: 'native.image',
      context: PREVIEW_CONTEXT,
      sessionId: 'native.image:native-asset-1',
      renderer: {
        kind: 'ui-bridge',
        entrypoint: 'wasm://preview/native-image',
        bridgeVersion: '1',
      },
      commands: [],
      slots: ['metadata'],
    },
    diagnostic: null,
    augmentations: [
      {
        augmentationId: 'native.metadata',
        slot: 'metadata',
        diagnostic: null,
        session: {
          providerId: 'native.metadata',
          context: PREVIEW_CONTEXT,
          sessionId: 'native.metadata:native-asset-1',
          renderer: {
            kind: 'text',
            text: 'Metadata augmentation mounted',
            truncated: false,
          },
          commands: [],
          slots: [],
        },
      },
    ],
  };
}
