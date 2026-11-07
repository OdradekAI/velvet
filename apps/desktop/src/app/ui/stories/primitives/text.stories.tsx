import type { UiKitStory } from '../../story-registry.js';
import { VelvetStack, VelvetText } from '../../primitives/index.js';

export const textStories: readonly UiKitStory[] = [
  { id: 'text/default', componentId: 'text', name: 'Default', description: 'Primary semantic text.', render: () => <VelvetText>Library</VelvetText> },
  { id: 'text/tones', componentId: 'text', name: 'Tones', description: 'Semantic text tones.', render: () => <VelvetStack gap="sm"><VelvetText tone="secondary">Secondary</VelvetText><VelvetText tone="muted">Muted</VelvetText><VelvetText tone="success">Success</VelvetText></VelvetStack> },
];
