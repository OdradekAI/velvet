import type { UiKitStory } from '../story-registry.js';
import { activityBarStories } from './host-projections/activity-bar.stories.js';
import { assetGridStories } from './host-projections/asset-grid.stories.js';
import { commandPaletteStories } from './host-projections/command-palette.stories.js';
import { dataTableStories } from './host-projections/data-table.stories.js';
import { paneStories } from './host-projections/pane.stories.js';
import { panelStories } from './host-projections/panel.stories.js';
import { previewHostStories } from './host-projections/preview-host.stories.js';
import { contextMenuStories } from './overlays/context-menu.stories.js';
import { dialogStories } from './overlays/dialog.stories.js';
import { toastStories } from './overlays/toast.stories.js';

export const velvetUiStories: readonly UiKitStory[] = [
  ...panelStories,
  ...dialogStories,
  ...contextMenuStories,
  ...toastStories,
  ...activityBarStories,
  ...paneStories,
  ...previewHostStories,
  ...assetGridStories,
  ...dataTableStories,
  ...commandPaletteStories,
];
