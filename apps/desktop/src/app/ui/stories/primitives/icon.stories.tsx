import type { UiKitStory } from '../../story-registry.js';
import { VelvetIcon } from '../../primitives/index.js';

export const iconStories: readonly UiKitStory[] = [
  { id: 'icon/default', componentId: 'icon', name: 'Default', description: 'Decorative icon.', render: () => <VelvetIcon>✦</VelvetIcon> },
  { id: 'icon/labelled', componentId: 'icon', name: 'Labelled', description: 'Named icon.', render: () => <VelvetIcon label="Favorite">★</VelvetIcon> },
];
