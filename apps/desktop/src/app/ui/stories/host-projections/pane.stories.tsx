import type { UiKitStory } from '../../story-registry.js';
import { VelvetPane } from '../../host-projections/index.js';

export const paneStories: readonly UiKitStory[] = [
  { id: 'pane/navigation', componentId: 'pane', name: 'Navigation', description: 'Navigation pane.', render: () => <VelvetPane region="navigation" title="Library">Content</VelvetPane>, scenario: { actions: [], checkpoints: [{ name: 'named region', assert: (runtime) => { const pane = runtime.root.querySelector('[role="region"]'); if (pane?.getAttribute('aria-labelledby') === null || pane?.querySelector('h2')?.textContent !== 'Library' || pane?.querySelector('.velvet-pane__body')?.textContent !== 'Content') throw new Error('Pane named region contract failed'); } }] } },
  { id: 'pane/context', componentId: 'pane', name: 'Context', description: 'Context pane.', render: () => <VelvetPane region="context" title="Inspector">Content</VelvetPane> },
];
