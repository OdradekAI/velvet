import type { PreviewIntent, PreviewSurface } from './preview-host.js';

export interface PreviewProviderDescriptor {
  readonly id: string;
  readonly extensionId?: string;
  readonly namespace?: string;
  contentKinds: readonly string[];
  intents: readonly PreviewIntent[];
  surfaces: readonly PreviewSurface[];
  priority: number;
}

export type PreviewAugmentationSlot =
  | 'preview-overlay'
  | 'inspector'
  | 'metadata'
  | 'histogram'
  | 'toolbar-action'
  | 'context-menu'
  | 'analysis-action';

export interface PreviewAugmentationDescriptor {
  readonly id: string;
  readonly extensionId?: string;
  readonly namespace?: string;
  readonly contentKinds: readonly string[];
  readonly slot: PreviewAugmentationSlot;
}

export class PreviewRegistry {
  private readonly providers = new Map<string, PreviewProviderDescriptor>();
  private readonly augmentations = new Map<string, PreviewAugmentationDescriptor>();

  public registerProvider(provider: PreviewProviderDescriptor): () => void {
    if (this.providers.has(provider.id)) {
      throw new Error(`Duplicate preview provider id: ${provider.id}`);
    }

    this.providers.set(provider.id, provider);
    return () => this.unregisterProvider(provider.id);
  }

  public registerAugmentation(augmentation: PreviewAugmentationDescriptor): () => void {
    if (this.augmentations.has(augmentation.id)) {
      throw new Error(`Duplicate preview augmentation id: ${augmentation.id}`);
    }

    this.augmentations.set(augmentation.id, augmentation);
    return () => this.unregisterAugmentation(augmentation.id);
  }

  public unregisterProvider(id: string): boolean {
    return this.providers.delete(id);
  }

  public unregisterAugmentation(id: string): boolean {
    return this.augmentations.delete(id);
  }

  public unregisterExtension(extensionId: string): void {
    for (const [id, provider] of this.providers) {
      if (provider.extensionId === extensionId) {
        this.providers.delete(id);
      }
    }
    for (const [id, augmentation] of this.augmentations) {
      if (augmentation.extensionId === extensionId) {
        this.augmentations.delete(id);
      }
    }
  }

  public listProviders(): readonly PreviewProviderDescriptor[] {
    return [...this.providers.values()].sort((left, right) =>
      right.priority - left.priority || left.id.localeCompare(right.id));
  }

  public listAugmentations(
    contentKind?: string,
  ): readonly PreviewAugmentationDescriptor[] {
    return [...this.augmentations.values()]
      .filter((augmentation) =>
        !contentKind || augmentation.contentKinds.length === 0 || augmentation.contentKinds.includes(contentKind),
      )
      .sort((left, right) => left.id.localeCompare(right.id));
  }
}
