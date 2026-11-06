import type { ReactNode } from 'react';
import type { UiKitComponentId } from './component-meta.js';
import { velvetUiStories } from './stories/velvet-ui.stories.js';

export interface UiKitStory {
  readonly id: string;
  readonly componentId: UiKitComponentId;
  readonly name: string;
  readonly description: string;
  readonly render: () => ReactNode;
}

export class UiKitStoryRegistry {
  private readonly storiesById: ReadonlyMap<string, UiKitStory>;

  public constructor(stories: readonly UiKitStory[]) {
    const entries = stories.map((story) => [story.id, story] as const);
    if (new Set(entries.map(([id]) => id)).size !== entries.length) {
      throw new Error('Velvet UI Kit story ids must be unique');
    }
    this.storiesById = new Map(entries);
  }

  public list(): readonly UiKitStory[] {
    return [...this.storiesById.values()];
  }

  public forComponent(componentId: UiKitComponentId): readonly UiKitStory[] {
    return this.list().filter((story) => story.componentId === componentId);
  }

  public get(id: string): UiKitStory | undefined {
    return this.storiesById.get(id);
  }
}

export const UI_KIT_STORY_REGISTRY = new UiKitStoryRegistry(velvetUiStories);
