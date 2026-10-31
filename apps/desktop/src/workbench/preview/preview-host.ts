import type {
  CoreRequest,
  CoreResolvedPreview,
  CoreTransport,
} from '../protocol.js';

export type PreviewIntent = 'quick' | 'inspect' | 'analyze';
export type PreviewSurface = 'workspace' | 'thumbnail' | 'inspector';

export interface PreviewContext {
  readonly assetId: string;
  readonly contentKind: string;
  readonly intent: PreviewIntent;
  readonly surface: PreviewSurface;
}

export type PreviewHostState =
  | 'idle'
  | 'resolving'
  | 'loading'
  | 'ready'
  | 'failed'
  | 'unsupported'
  | 'disabled'
  | 'disposed';

export type ResolvedPreview = CoreResolvedPreview;

export interface PreviewHostSnapshot {
  readonly state: PreviewHostState;
  readonly preview?: ResolvedPreview;
  readonly error?: string;
}

export class PreviewHost {
  private state: PreviewHostState = 'idle';
  private preview: ResolvedPreview | undefined;
  private error: string | undefined;
  private requestToken = 0;
  private readonly listeners = new Set<() => void>();

  public constructor(private readonly core: CoreTransport) {}

  public get snapshot(): PreviewHostSnapshot {
    return {
      state: this.state,
      preview: this.preview,
      error: this.error,
    };
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /**
   * Resolves one primary provider through Rust and keeps a generic fallback
   * preview available when transport or provider resolution fails.
   */
  public async open(context: PreviewContext): Promise<ResolvedPreview> {
    return this.openRequest(
      {
        protocolVersion: 1,
        kind: 'resolve-preview',
        assetId: context.assetId,
        contentKind: context.contentKind,
        intent: context.intent,
        surface: context.surface,
      },
      context,
    );
  }

  /** Resolves a persisted asset by opaque identity; Rust supplies its content kind. */
  public async openAsset(
    assetId: string,
    intent: PreviewIntent,
    surface: PreviewSurface,
  ): Promise<ResolvedPreview> {
    return this.openRequest(
      {
        protocolVersion: 1,
        kind: 'resolve-preview-asset',
        assetId,
        intent,
        surface,
      },
      {
        assetId,
        contentKind: 'application/octet-stream',
        intent,
        surface,
      },
    );
  }

  private async openRequest(request: CoreRequest, context: PreviewContext): Promise<ResolvedPreview> {
    this.ensureUsable();
    const token = ++this.requestToken;
    this.setState('resolving');
    this.error = undefined;

    try {
      const response = await this.core.request(request);

      if (token !== this.requestToken) {
        return this.preview ?? fallbackPreview(context);
      }
      if (response.kind !== 'preview') {
        throw new Error(`Expected a preview response, received ${response.kind}`);
      }

      this.setState('loading');
      this.replacePreview(response.preview);
      this.setState('ready');
      return response.preview;
    } catch (cause) {
      if (token !== this.requestToken) {
        return this.preview ?? fallbackPreview(context);
      }

      this.error = cause instanceof Error ? cause.message : String(cause);
      const fallback = fallbackPreview(context);
      this.replacePreview(fallback);
      this.setState('failed');
      return fallback;
    }
  }

  public close(): void {
    if (this.state === 'disposed') {
      return;
    }
    this.releaseSessions(this.preview);
    this.requestToken += 1;
    this.preview = undefined;
    this.error = undefined;
    this.setState('idle');
  }

  public dispose(): void {
    if (this.state === 'disposed') {
      return;
    }
    this.releaseSessions(this.preview);
    this.requestToken += 1;
    this.preview = undefined;
    this.setState('disposed');
    this.listeners.clear();
  }

  private ensureUsable(): void {
    if (this.state === 'disposed') {
      throw new Error('PreviewHost is disposed');
    }
  }

  private replacePreview(preview: ResolvedPreview): void {
    if (this.preview) {
      this.releaseSessions(this.preview);
    }
    this.preview = preview;
  }

  private releaseSessions(preview: ResolvedPreview | undefined): void {
    if (!preview) {
      return;
    }
    const sessionIds = new Set([
      preview.session.sessionId,
      ...(preview.augmentations ?? []).flatMap((augmentation) =>
        augmentation.session ? [augmentation.session.sessionId] : []),
    ]);
    for (const sessionId of sessionIds) {
      void this.core.request({
        protocolVersion: 1,
        kind: 'release-preview-session',
        sessionId,
      }).catch(() => undefined);
    }
  }

  private setState(state: PreviewHostState): void {
    this.state = state;
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One preview projection must not block unrelated Workbench updates.
      }
    }
  }
}

function fallbackPreview(context: PreviewContext): ResolvedPreview {
  return {
    candidate: {
      providerId: 'builtin.generic-fallback',
      contentKinds: [],
      intents: [context.intent],
      surfaces: [context.surface],
      priority: Number.MIN_SAFE_INTEGER,
    },
    session: {
      providerId: 'builtin.generic-fallback',
      context,
      sessionId: 'builtin.generic-fallback',
      renderer: { kind: 'generic' },
      commands: [],
      slots: [],
    },
    diagnostic: null,
    augmentations: [],
  };
}
