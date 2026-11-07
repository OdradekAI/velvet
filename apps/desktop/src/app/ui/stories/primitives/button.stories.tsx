import type { UiKitStory } from '../../story-registry.js';
import { VelvetButton } from '../../primitives/index.js';

let activated = 0;
const noop = () => undefined;

export const buttonStories: readonly UiKitStory[] = [
  {
    id: 'button/default', componentId: 'button', name: 'Default', description: 'Primary action.',
    render: () => <VelvetButton onClick={() => { activated += 1; }}>Import</VelvetButton>,
    scenario: {
      covers: ['activate'],
      setup: () => { activated = 0; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('button')?.click(); runtime.host.record('button.activate'); })],
      checkpoints: [{ name: 'committed activation', afterAction: 0, assert: (runtime) => { if (activated !== 1 || runtime.root.querySelector('button')?.getAttribute('type') !== 'button') throw new Error('Button activation/semantic contract failed'); } }],
      cleanup: () => { activated = 0; },
    },
  },
  { id: 'button/busy', componentId: 'button', name: 'Busy', description: 'Busy action.', render: () => <VelvetButton busy>Importing</VelvetButton> },
  { id: 'button/disabled', componentId: 'button', name: 'Disabled', description: 'Disabled action.', render: () => <VelvetButton disabled>Unavailable</VelvetButton> },
];
