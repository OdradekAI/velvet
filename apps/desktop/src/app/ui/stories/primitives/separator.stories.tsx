import type { UiKitStory } from '../../story-registry.js';
import { VelvetSeparator } from '../../primitives/index.js';

export const separatorStories: readonly UiKitStory[] = [
  { id: 'separator/horizontal', componentId: 'separator', name: 'Horizontal', description: 'Horizontal divider.', render: () => <VelvetSeparator /> },
  { id: 'separator/vertical', componentId: 'separator', name: 'Vertical', description: 'Vertical divider.', render: () => <VelvetSeparator orientation="vertical" /> },
];
