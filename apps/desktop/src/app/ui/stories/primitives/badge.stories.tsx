import type { UiKitStory } from '../../story-registry.js';
import { VelvetBadge, VelvetStack } from '../../primitives/index.js';

export const badgeStories: readonly UiKitStory[] = [
  { id: 'badge/default', componentId: 'badge', name: 'Default', description: 'Count badge.', render: () => <VelvetBadge>12</VelvetBadge> },
  { id: 'badge/tones', componentId: 'badge', name: 'Tones', description: 'Status badges.', render: () => <VelvetStack direction="row" gap="sm"><VelvetBadge tone="info">Info</VelvetBadge><VelvetBadge tone="success">Ready</VelvetBadge><VelvetBadge tone="danger">Error</VelvetBadge></VelvetStack> },
];
