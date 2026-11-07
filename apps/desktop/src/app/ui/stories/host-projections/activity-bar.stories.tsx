import type { UiKitStory } from '../../story-registry.js';
import { VelvetActivityBar } from '../../host-projections/index.js';

let selectedActivity = '';
const noop = () => undefined;

export const activityBarStories: readonly UiKitStory[] = [
  {
    id: 'activity-bar/default', componentId: 'activity-bar', name: 'Default', description: 'Workbench navigation.',
    render: () => <VelvetActivityBar items={[{ id: 'library', label: 'Library', icon: '▦', active: true }, { id: 'feed', label: 'Feed', icon: '◌' }]} onSelect={(id) => { selectedActivity = id; }} />,
    scenario: {
      setup: () => { selectedActivity = ''; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLElement>('[aria-label="Library"]')?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowRight' })); })],
      checkpoints: [{ name: 'activity navigation', afterAction: 0, assert: () => { if (selectedActivity !== 'feed') throw new Error('ActivityBar ArrowRight contract failed'); } }],
    },
  },
  { id: 'activity-bar/badges', componentId: 'activity-bar', name: 'Badges', description: 'Navigation badges.', render: () => <VelvetActivityBar items={[{ id: 'library', label: 'Library', icon: '▦', badge: 3 }]} onSelect={noop} /> },
];
