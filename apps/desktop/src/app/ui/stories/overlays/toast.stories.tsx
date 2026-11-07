import type { UiKitStory } from '../../story-registry.js';
import { VelvetToastHost } from '../../overlays/index.js';

const noop = () => undefined;

export const toastStories: readonly UiKitStory[] = [
  { id: 'toast/default', componentId: 'toast', name: 'Default', description: 'Info toast.', render: () => <VelvetToastHost items={[{ id: 'toast-1', message: 'Saved', severity: 'info' }]} onDismiss={noop} /> },
  { id: 'toast/error', componentId: 'toast', name: 'Error', description: 'Error toast.', render: () => <VelvetToastHost items={[{ id: 'toast-1', message: 'Could not save', severity: 'error' }]} onDismiss={noop} /> },
];
