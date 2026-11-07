import type { UiKitStory } from '../../story-registry.js';
import { VelvetIconButton } from '../../primitives/index.js';

export const iconButtonStories: readonly UiKitStory[] = [
  { id: 'icon-button/default', componentId: 'icon-button', name: 'Default', description: 'Icon action.', render: () => <VelvetIconButton icon="+" label="Add" /> },
  { id: 'icon-button/disabled', componentId: 'icon-button', name: 'Disabled', description: 'Disabled icon action.', render: () => <VelvetIconButton disabled icon="×" label="Remove" /> },
];
