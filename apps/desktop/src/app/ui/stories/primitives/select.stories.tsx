import type { UiKitStory } from '../../story-registry.js';
import { VelvetSelect } from '../../primitives/index.js';
import { useState } from 'react';

let changeSelect: ((value: string) => void) | undefined;
let selectedValue = '';

function SelectDefaultStory() {
  const [value, setValue] = useState('recent');
  changeSelect = (nextValue) => { selectedValue = nextValue; setValue(nextValue); };
  return <VelvetSelect ariaLabel="Sort" onChange={(event) => { selectedValue = event.currentTarget.value; setValue(event.currentTarget.value); }} value={value}><option value="recent">Recent</option><option value="name">Name</option></VelvetSelect>;
}

export const selectStories: readonly UiKitStory[] = [
  {
    id: 'select/default',
    componentId: 'select',
    name: 'Default',
    description: 'Native select.',
    render: () => <SelectDefaultStory />,
    scenario: {
      covers: ['change'],
      setup: () => { selectedValue = 'recent'; },
      actions: [(runtime) => runtime.act(async () => { changeSelect?.('name'); })],
      checkpoints: [{ name: 'selection change', afterAction: 0, assert: (runtime) => { if (selectedValue !== 'name' || runtime.root.querySelector<HTMLSelectElement>('select')?.value !== 'name' || runtime.root.querySelector('select')?.getAttribute('aria-label') !== 'Sort') throw new Error('Select change/accessibility contract failed'); } }],
      cleanup: () => { changeSelect = undefined; selectedValue = ''; },
    },
  },
  { id: 'select/disabled', componentId: 'select', name: 'Disabled', description: 'Disabled select.', render: () => <VelvetSelect disabled ariaLabel="Sort"><option>Unavailable</option></VelvetSelect> },
];
