import type { PreviewIntent, PreviewSurface } from './preview-host.js';

export interface PreviewProviderDescriptor {
  id: string;
  contentKinds: readonly string[];
  intents: readonly PreviewIntent[];
  surfaces: readonly PreviewSurface[];
  priority: number;
}

export interface PreviewAugmentationDescriptor {
  id: string;
  contentKinds: readonly string[];
  slot: 'workspace.overlay' | 'inspector' | 'toolbar';
}

export class PreviewRegistry {
  private readonly providers = new Map<string, PreviewProviderDescriptor>();
  private readonly augmentations = new Map<string, PreviewAugmentationDescriptor>();

  public registerProvider(provider: PreviewProviderDescriptor): void {
    if (this.providers.has(provider.id)) {
      throw new Error(`Duplicate preview provider id: ${provider.id}`);
    }

    this.providers.set(provider.id, provider);
  }

  public registerAugmentation(augmentation: PreviewAugmentationDescriptor): void {
    if (this.augmentations.has(augmentation.id)) {
      throw new Error(`Duplicate preview augmentation id: ${augmentation.id}`);
    }

    this.augmentations.set(augmentation.id, augmentation);
  }
}
