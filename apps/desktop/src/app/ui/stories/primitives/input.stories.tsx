import type { UiKitStory } from '../../story-registry.js';
import { VelvetInput } from '../../primitives/index.js';
import { useState } from 'react';

let changeInput: ((value: string) => void) | undefined;
let changedInputValue = '';

function InputChangeStory() {
  const [value, setValue] = useState('Search');
  changeInput = (nextValue) => { changedInputValue = nextValue; setValue(nextValue); };
  return <VelvetInput ariaLabel="Search" onChange={(event) => setValue(event.currentTarget.value)} value={value} />;
}

export const inputStories: readonly UiKitStory[] = [
  { id: 'input/default', componentId: 'input', name: 'Default', description: 'Text input.', render: () => <VelvetInput ariaLabel="Search" placeholder="Search" /> },
  {
    id: 'input/change',
    componentId: 'input',
    name: 'Change',
    description: 'Controlled text change.',
    render: () => <InputChangeStory />,
    scenario: {
      setup: () => { changedInputValue = 'Search'; },
      actions: [(runtime) => runtime.act(async () => { changeInput?.('Astronaut'); })],
      checkpoints: [{ name: 'controlled value change', afterAction: 0, assert: (runtime) => { const domValue = runtime.root.querySelector<HTMLInputElement>('input')?.value; if (changedInputValue !== 'Astronaut' || domValue !== 'Astronaut') throw new Error('Input change contract failed'); } }],
      cleanup: () => { changeInput = undefined; changedInputValue = ''; },
    },
  },
  { id: 'input/busy', componentId: 'input', name: 'Busy', description: 'Busy text input.', render: () => <VelvetInput busy ariaLabel="Search" value="Loading" readOnly /> },
];
