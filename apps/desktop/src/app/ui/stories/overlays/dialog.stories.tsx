import type { UiKitStory } from '../../story-registry.js';
import { VelvetDialog } from '../../overlays/index.js';

const noop = () => undefined;

export const dialogStories: readonly UiKitStory[] = [
  { id: 'dialog/closed', componentId: 'dialog', name: 'Closed', description: 'Closed dialog lifecycle.', render: () => <VelvetDialog open={false} title="Confirm" onOpenChange={noop}>Content</VelvetDialog> },
  { id: 'dialog/open', componentId: 'dialog', name: 'Open', description: 'Open dialog lifecycle.', render: () => <VelvetDialog open title="Confirm" description="Review this action." onOpenChange={noop}>Content</VelvetDialog> },
];
