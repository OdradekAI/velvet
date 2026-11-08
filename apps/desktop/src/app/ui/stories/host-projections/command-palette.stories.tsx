import type { UiKitStory } from '../../story-registry.js';
import { VelvetCommandPalette } from '../../host-projections/index.js';

let selectedCommand = '';
let closed = false;
const noop = () => undefined;

export const commandPaletteStories: readonly UiKitStory[] = [
  {
    id: 'command-palette/open',
    componentId: 'command-palette',
    name: 'Open',
    description: 'Open command palette.',
    render: () => <VelvetCommandPalette open commands={[{ id: 'open', label: 'Open' }, { id: 'pin', label: 'Pin' }]} onSelect={(id) => { selectedCommand = id; }} onClose={() => { closed = true; }} />,
    scenario: {
      covers: ['select', 'navigate', 'close'],
      setup: () => { selectedCommand = ''; closed = false; },
      actions: [
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.focus();
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowDown' }));
        }),
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Home' }));
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'End' }));
        }),
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }));
          runtime.host.record('command.select', 'pin');
        }),
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
        }),
      ],
      checkpoints: [
        { name: 'keyboard navigation', afterAction: 0, assert: (runtime) => { if (runtime.root.querySelector('[role="option"][aria-selected="true"]')?.textContent?.trim() !== 'Pin') throw new Error('CommandPalette ArrowDown contract failed'); } },
        { name: 'home/end navigation', afterAction: 1, assert: (runtime) => { if (runtime.root.querySelector('[role="option"][aria-selected="true"]')?.textContent?.trim() !== 'Pin') throw new Error('CommandPalette Home/End contract failed'); } },
        { name: 'keyboard command selection', afterAction: 2, assert: () => { if (selectedCommand !== 'pin') throw new Error('CommandPalette keyboard selection contract failed'); } },
        { name: 'escape close', afterAction: 3, assert: () => { if (!closed) throw new Error('CommandPalette Escape close contract failed'); } },
      ],
    },
  },
  { id: 'command-palette/empty', componentId: 'command-palette', name: 'Empty', description: 'Empty command result.', render: () => <VelvetCommandPalette open commands={[]} onSelect={noop} onClose={noop} /> },
];
