import type { UiKitStory } from '../../story-registry.js';
import { VelvetSelect } from '../../primitives/index.js';

export const selectStories: readonly UiKitStory[] = [
  { id: 'select/default', componentId: 'select', name: 'Default', description: 'Native select.', render: () => <VelvetSelect ariaLabel="Sort" defaultValue="recent"><option value="recent">Recent</option><option value="name">Name</option></VelvetSelect> },
  { id: 'select/disabled', componentId: 'select', name: 'Disabled', description: 'Disabled select.', render: () => <VelvetSelect disabled ariaLabel="Sort"><option>Unavailable</option></VelvetSelect> },
];
