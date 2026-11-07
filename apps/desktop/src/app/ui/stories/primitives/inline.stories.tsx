import type { UiKitStory } from '../../story-registry.js';
import { VelvetButton, VelvetInline } from '../../primitives/index.js';

export const inlineStories: readonly UiKitStory[] = [
  { id: 'inline/default', componentId: 'inline', name: 'Default', description: 'Inline token spacing.', render: () => <VelvetInline><VelvetButton>One</VelvetButton><VelvetButton variant="quiet">Two</VelvetButton></VelvetInline> },
];
