import type { UiKitStory } from '../../story-registry.js';
import { VelvetButton, VelvetStack, VelvetText } from '../../primitives/index.js';

export const stackStories: readonly UiKitStory[] = [
  { id: 'stack/default', componentId: 'stack', name: 'Default', description: 'Vertical token spacing.', render: () => <VelvetStack><VelvetText>One</VelvetText><VelvetText>Two</VelvetText></VelvetStack> },
  { id: 'stack/directions', componentId: 'stack', name: 'Directions', description: 'Vertical and inline layouts.', render: () => <VelvetStack direction="row"><VelvetButton>One</VelvetButton><VelvetButton variant="quiet">Two</VelvetButton></VelvetStack> },
];
