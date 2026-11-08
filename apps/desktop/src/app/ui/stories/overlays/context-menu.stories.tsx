import type { UiKitStory } from '../../story-registry.js';
import { VelvetButton } from '../../primitives/index.js';
import { VelvetContextMenu } from '../../overlays/index.js';

const noop = () => undefined;

export const contextMenuStories: readonly UiKitStory[] = [
  { id: 'context-menu/default', componentId: 'context-menu', name: 'Default', description: 'Contextual actions.', render: () => <VelvetContextMenu actions={[{ id: 'open', label: 'Open', onSelect: noop }]}><VelvetButton variant="quiet">Right click</VelvetButton></VelvetContextMenu> },
  { id: 'context-menu/checked', componentId: 'context-menu', name: 'Checked', description: 'Checked contextual action.', render: () => <VelvetContextMenu actions={[{ id: 'pin', label: 'Pinned', checked: true }]}><VelvetButton variant="quiet">Right click</VelvetButton></VelvetContextMenu> },
  { id: 'context-menu/submenu', componentId: 'context-menu', name: 'Submenu', description: 'Nested contextual actions with Radix-managed roving focus.', render: () => <VelvetContextMenu actions={[{ id: 'more', label: 'More actions', submenu: [{ id: 'reveal', label: 'Reveal in library', onSelect: noop }, { id: 'copy', label: 'Copy link', shortcutHint: '⌘L', onSelect: noop }] }]}><VelvetButton variant="quiet">Right click</VelvetButton></VelvetContextMenu> },
];
