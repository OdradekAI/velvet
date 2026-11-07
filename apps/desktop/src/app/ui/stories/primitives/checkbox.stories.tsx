import type { UiKitStory } from '../../story-registry.js';
import { VelvetCheckbox } from '../../primitives/index.js';

let checkedValue = true;
const noop = () => undefined;

export const checkboxStories: readonly UiKitStory[] = [
  {
    id: 'checkbox/checked', componentId: 'checkbox', name: 'Checked', description: 'Checked setting.',
    render: () => <VelvetCheckbox checked={checkedValue} label="Show metadata" onCheckedChange={(value) => { checkedValue = value; }} />,
    scenario: {
      covers: ['toggle'],
      setup: () => { checkedValue = true; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click(); })],
      checkpoints: [{ name: 'toggle action', afterAction: 0, assert: (runtime) => { if (checkedValue !== false || runtime.root.querySelector('input[type="checkbox"]')?.closest('label') === null) throw new Error('Checkbox toggle/label contract failed'); } }],
    },
  },
  { id: 'checkbox/disabled', componentId: 'checkbox', name: 'Disabled', description: 'Disabled setting.', render: () => <VelvetCheckbox checked={false} disabled label="Unavailable" onCheckedChange={noop} /> },
];
