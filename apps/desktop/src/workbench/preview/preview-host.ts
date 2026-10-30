import type {
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
    this.ensureUsable();
    const token = ++this.requestToken;
    this.setState('resolving');
    this.error = undefined;

    try {
      const response = await this.core.request({
        protocolVersion: 1,
        kind: 'resolve-preview',
        assetId: context.assetId,
        contentKind: context.contentKind,
        intent: context.intent,
        surface: context.surface,
      });

      if (token !== this.requestToken) {
        return this.preview ?? fallbackPreview(context);
      }
      if (response.kind !== 'preview') {
        throw new Error(`Expected a preview response, received ${response.kind}`);
      }

      this.setState('loading');
      this.preview = response.preview;
      this.setState('ready');
      return response.preview;
    } catch (cause) {
      if (token !== this.requestToken) {
        return this.preview ?? fallbackPreview(context);
      }

      this.error = cause instanceof Error ? cause.message : String(cause);
      this.preview = fallbackPreview(context);
      this.setState('failed');
      return this.preview;
    }
  }

  public close(): void {
    if (this.state === 'disposed') {
      return;
    }
    this.requestToken += 1;
    this.preview = undefined;
    this.error = undefined;
    this.setState('idle');
  }

  public dispose(): void {
    if (this.state === 'disposed') {
      return;
    }
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
    },
  };
}
