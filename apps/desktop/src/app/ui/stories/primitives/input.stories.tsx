import type { UiKitStory } from '../../story-registry.js';
import { VelvetInput } from '../../primitives/index.js';

export const inputStories: readonly UiKitStory[] = [
  { id: 'input/default', componentId: 'input', name: 'Default', description: 'Text input.', render: () => <VelvetInput ariaLabel="Search" placeholder="Search" /> },
  { id: 'input/busy', componentId: 'input', name: 'Busy', description: 'Busy text input.', render: () => <VelvetInput busy ariaLabel="Search" value="Loading" readOnly /> },
];
