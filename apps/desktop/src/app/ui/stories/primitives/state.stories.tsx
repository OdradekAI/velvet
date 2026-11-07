import type { UiKitStory } from '../../story-registry.js';
import { VelvetState } from '../../primitives/index.js';

let retried = 0;
const noop = () => undefined;

export const stateStories: readonly UiKitStory[] = [
  { id: 'state/loading', componentId: 'state', name: 'Loading', description: 'Loading state.', render: () => <VelvetState kind="loading" title="Loading" /> },
  { id: 'state/empty', componentId: 'state', name: 'Empty', description: 'Empty state with an action slot.', render: () => <VelvetState action={<button type="button">Add asset</button>} kind="empty" title="Nothing here" description="Add an asset to begin." /> },
  {
    id: 'state/error-retry', componentId: 'state', name: 'Error retry', description: 'Retryable error state.',
    render: () => <VelvetState kind="error" title="Failed" onRetry={() => { retried += 1; }} />,
    scenario: {
      covers: ['retry'],
      setup: () => { retried = 0; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('button')?.click(); })],
      checkpoints: [{ name: 'retry action', afterAction: 0, assert: (runtime) => { if (retried !== 1 || runtime.root.querySelector('[role="alert"]') === null) throw new Error('State retry/live-region contract failed'); } }],
    },
  },
];
