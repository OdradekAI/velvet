import type { UiKitStory } from '../../story-registry.js';
import { VelvetPane } from '../../host-projections/index.js';

export const paneStories: readonly UiKitStory[] = [
  { id: 'pane/navigation', componentId: 'pane', name: 'Navigation', description: 'Navigation pane.', render: () => <VelvetPane region="navigation" title="Library">Content</VelvetPane> },
  { id: 'pane/context', componentId: 'pane', name: 'Context', description: 'Context pane.', render: () => <VelvetPane region="context" title="Inspector">Content</VelvetPane> },
];
