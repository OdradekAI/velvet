import type { UiKitStory } from '../../story-registry.js';
import { VelvetAssetGrid } from '../../host-projections/index.js';

let selectedAsset = '';
let openedAsset = '';

export const assetGridStories: readonly UiKitStory[] = [
  {
    id: 'asset-grid/default',
    componentId: 'asset-grid',
    name: 'Default',
    description: 'Asset grid.',
    render: () => <VelvetAssetGrid items={[{ id: 'a', label: 'A', selected: true }, { id: 'b', label: 'B' }]} onOpen={(id) => { openedAsset = id; }} onSelect={(id) => { selectedAsset = id; }} />,
    scenario: {
      setup: () => { selectedAsset = ''; openedAsset = ''; },
      actions: [
        (runtime) => runtime.act(async () => {
          const cell = runtime.root.querySelector<HTMLElement>('[role="gridcell"]');
          cell?.focus();
          cell?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowRight' }));
          runtime.host.record('asset.select', 'b');
        }),
        (runtime) => runtime.act(async () => {
          const cell = runtime.root.querySelector<HTMLElement>('[role="gridcell"][aria-label="B"]');
          cell?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }));
          runtime.host.record('asset.open', 'b');
        }),
      ],
      checkpoints: [
        { name: 'keyboard selection', afterAction: 0, assert: (runtime) => { if (selectedAsset !== 'b' || runtime.root.ownerDocument.activeElement?.getAttribute('aria-label') !== 'B') throw new Error('AssetGrid keyboard selection/focus contract failed'); } },
        { name: 'keyboard open', afterAction: 1, assert: () => { if (openedAsset !== 'b') throw new Error('AssetGrid Enter open contract failed'); } },
      ],
    },
  },
  { id: 'asset-grid/selected', componentId: 'asset-grid', name: 'Selected', description: 'Selected asset grid.', render: () => <VelvetAssetGrid items={[{ id: 'a', label: 'A', selected: true }]} onSelect={() => undefined} /> },
];
