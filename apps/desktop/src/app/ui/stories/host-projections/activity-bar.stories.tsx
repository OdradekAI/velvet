import type { UiKitStory } from '../../story-registry.js';
import { VelvetActivityBar } from '../../host-projections/index.js';

let selectedActivity = '';
const noop = () => undefined;

export const activityBarStories: readonly UiKitStory[] = [
  {
    id: 'activity-bar/default', componentId: 'activity-bar', name: 'Default', description: 'Workbench navigation.',
    render: () => <VelvetActivityBar items={[{ id: 'library', label: 'Library', icon: '▦', active: true }, { id: 'feed', label: 'Feed', icon: '◌' }]} onSelect={(id) => { selectedActivity = id; }} />,
    scenario: {
      covers: ['select', 'navigate'],
      setup: () => { selectedActivity = ''; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLElement>('[aria-label="Library"]')?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowRight' })); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[aria-label="Library"]')?.click(); }),
      ],
      checkpoints: [
        { name: 'activity navigation', afterAction: 0, assert: () => { if (selectedActivity !== 'feed') throw new Error('ActivityBar ArrowRight contract failed'); } },
        { name: 'activity selection', afterAction: 1, assert: () => { if (selectedActivity !== 'library') throw new Error('ActivityBar click selection contract failed'); } },
      ],
    },
  },
  { id: 'activity-bar/badges', componentId: 'activity-bar', name: 'Badges', description: 'Navigation badges.', render: () => <VelvetActivityBar items={[{ id: 'library', label: 'Library', icon: '▦', badge: 3 }]} onSelect={noop} /> },
];
