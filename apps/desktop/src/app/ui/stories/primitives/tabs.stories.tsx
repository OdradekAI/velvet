import type { UiKitStory } from '../../story-registry.js';
import { VelvetTabs } from '../../primitives/index.js';

let selectedTab = '';
const noop = () => undefined;

export const tabsStories: readonly UiKitStory[] = [
  {
    id: 'tabs/default', componentId: 'tabs', name: 'Default', description: 'Roving-focus tabs.',
    render: () => <VelvetTabs tabs={[{ id: 'all', label: 'All' }, { id: 'recent', label: 'Recent' }]} selectedId="all" onSelect={(id) => { selectedTab = id; }} />,
    scenario: {
      covers: ['select', 'navigate'],
      setup: () => { selectedTab = ''; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLElement>('[role="tab"]')?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowRight' })); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[role="tab"]:first-of-type')?.click(); }),
      ],
      checkpoints: [
        { name: 'roving focus navigation', afterAction: 0, assert: () => { if (selectedTab !== 'recent') throw new Error('Tabs ArrowRight contract failed'); } },
        { name: 'tab selection', afterAction: 1, assert: () => { if (selectedTab !== 'all') throw new Error('Tabs click selection contract failed'); } },
      ],
    },
  },
  { id: 'tabs/disabled', componentId: 'tabs', name: 'Disabled', description: 'Tabs with a disabled item.', render: () => <VelvetTabs tabs={[{ id: 'all', label: 'All' }, { id: 'recent', label: 'Recent', disabled: true }]} selectedId="all" onSelect={noop} /> },
];
