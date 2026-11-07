import type { UiKitStory } from '../../story-registry.js';
import { VelvetProgress } from '../../primitives/index.js';

export const progressStories: readonly UiKitStory[] = [
  { id: 'progress/determinate', componentId: 'progress', name: 'Determinate', description: 'Determinate progress.', render: () => <VelvetProgress label="Indexing" value={60} /> },
  { id: 'progress/indeterminate', componentId: 'progress', name: 'Indeterminate', description: 'Indeterminate progress.', render: () => <VelvetProgress label="Working" /> },
];
