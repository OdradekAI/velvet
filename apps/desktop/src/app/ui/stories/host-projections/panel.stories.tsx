import type { UiKitStory } from '../../story-registry.js';
import { VelvetPanel } from '../../host-projections/index.js';

export const panelStories: readonly UiKitStory[] = [
  { id: 'panel/panel', componentId: 'panel', name: 'Panel', description: 'Panel surface.', render: () => <VelvetPanel title="Library">Content</VelvetPanel>, scenario: { actions: [], checkpoints: [{ name: 'named surface', assert: (runtime) => { const panel = runtime.root.querySelector('[role="region"]'); if (panel?.getAttribute('aria-labelledby') === null || panel?.querySelector('h2')?.textContent !== 'Library' || panel?.querySelector('.velvet-panel__body')?.textContent !== 'Content') throw new Error('Panel named region contract failed'); } }] } },
  { id: 'panel/raised', componentId: 'panel', name: 'Raised', description: 'Raised surface.', render: () => <VelvetPanel surface="raised" title="Inspector">Content</VelvetPanel> },
];
