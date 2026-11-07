import type { UiKitStory } from '../../story-registry.js';
import { VelvetIconButton } from '../../primitives/index.js';

let activated = 0;

export const iconButtonStories: readonly UiKitStory[] = [
  {
    id: 'icon-button/default',
    componentId: 'icon-button',
    name: 'Default',
    description: 'Icon action.',
    render: () => <VelvetIconButton icon="+" label="Add" onClick={() => { activated += 1; }} />,
    scenario: {
      covers: ['activate'],
      setup: () => { activated = 0; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[aria-label="Add"]')?.click(); })],
      checkpoints: [{ name: 'icon activation', afterAction: 0, assert: () => { if (activated !== 1) throw new Error('IconButton activation contract failed'); } }],
    },
  },
  { id: 'icon-button/disabled', componentId: 'icon-button', name: 'Disabled', description: 'Disabled icon action.', render: () => <VelvetIconButton disabled icon="×" label="Remove" /> },
];
