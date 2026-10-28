import type { CoreTransport } from '../protocol.js';

export type PreviewIntent = 'quick' | 'inspect' | 'analyze';
export type PreviewSurface = 'workspace' | 'thumbnail' | 'inspector';

export interface PreviewContext {
  assetId: string;
  contentKind: string;
  intent: PreviewIntent;
  surface: PreviewSurface;
}

export interface ResolvedPreview {
  providerId: string;
  context: PreviewContext;
}

export class PreviewHost {
  public constructor(private readonly core: CoreTransport) {}

  public async open(context: PreviewContext): Promise<ResolvedPreview> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'resolve-preview',
      assetId: context.assetId,
      contentKind: context.contentKind,
      intent: context.intent,
      surface: context.surface,
    });

    if (response.kind !== 'preview') {
      throw new Error(`Expected a preview response, received ${response.kind}`);
    }

    return response.preview as ResolvedPreview;
  }
}
