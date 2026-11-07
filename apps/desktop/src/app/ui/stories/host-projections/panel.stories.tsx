import type { UiKitStory } from '../../story-registry.js';
import { VelvetPanel } from '../../host-projections/index.js';

export const panelStories: readonly UiKitStory[] = [
  { id: 'panel/panel', componentId: 'panel', name: 'Panel', description: 'Panel surface.', render: () => <VelvetPanel title="Library">Content</VelvetPanel> },
  { id: 'panel/raised', componentId: 'panel', name: 'Raised', description: 'Raised surface.', render: () => <VelvetPanel surface="raised" title="Inspector">Content</VelvetPanel> },
];
