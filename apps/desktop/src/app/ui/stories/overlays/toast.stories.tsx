import type { UiKitStory } from '../../story-registry.js';
import { VelvetToastHost } from '../../overlays/index.js';

let dismissedToast = '';

export const toastStories: readonly UiKitStory[] = [
  {
    id: 'toast/default',
    componentId: 'toast',
    name: 'Default',
    description: 'Info toast.',
    render: () => <VelvetToastHost items={[{ id: 'toast-1', message: 'Saved', severity: 'info' }]} onDismiss={(id) => { dismissedToast = id; }} />,
    scenario: {
      covers: ['dismiss'],
      setup: () => { dismissedToast = ''; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[aria-label="Dismiss notification"]')?.click(); })],
      checkpoints: [{ name: 'toast dismissal', afterAction: 0, assert: () => { if (dismissedToast !== 'toast-1') throw new Error('Toast dismissal contract failed'); } }],
    },
  },
  { id: 'toast/error', componentId: 'toast', name: 'Error', description: 'Error toast.', render: () => <VelvetToastHost items={[{ id: 'toast-1', message: 'Could not save', severity: 'error' }]} onDismiss={() => undefined} /> },
];
