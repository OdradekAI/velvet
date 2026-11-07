import type { UiKitStory } from '../../story-registry.js';
import { VelvetAssetGrid } from '../../host-projections/index.js';

let selectedAsset = '';
let openedAsset = '';
let draggedAsset = '';
let droppedAsset = '';

export const assetGridStories: readonly UiKitStory[] = [
  {
    id: 'asset-grid/default',
    componentId: 'asset-grid',
    name: 'Default',
    description: 'Asset grid.',
    render: () => <VelvetAssetGrid items={[{ id: 'a', label: 'A', selected: true }, { id: 'b', label: 'B' }]} onOpen={(id) => { openedAsset = id; }} onSelect={(id) => { selectedAsset = id; }} />,
    scenario: {
      setup: () => { selectedAsset = ''; openedAsset = ''; draggedAsset = ''; droppedAsset = ''; },
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
  {
    id: 'asset-grid/layout-modes',
    componentId: 'asset-grid',
    name: 'Layout modes',
    description: 'Eagle-observed list layout with optional drag and drop.',
    render: () => <VelvetAssetGrid layout="list" items={[{ id: 'a', label: 'A', pinned: true }, { id: 'b', label: 'B', aspectRatio: '16 / 9' }]} onSelect={() => undefined} onDragStart={(id) => { draggedAsset = id; }} onDrop={(sourceId, targetId) => { droppedAsset = `${sourceId}:${targetId}`; }} />,
    scenario: {
      setup: () => { draggedAsset = ''; droppedAsset = ''; },
      actions: [(runtime) => runtime.act(async () => {
        const cells = [...runtime.root.querySelectorAll<HTMLButtonElement>('[role="gridcell"]')];
        const source = cells[0];
        const target = cells[1];
        const transfer = new Map<string, string>();
        const dragStart = new Event('dragstart', { bubbles: true });
        Object.defineProperty(dragStart, 'dataTransfer', { value: { setData: (type: string, value: string) => transfer.set(type, value), getData: (type: string) => transfer.get(type) ?? '' } });
        source?.dispatchEvent(dragStart);
        if (!draggedAsset) throw new Error('AssetGrid drag start contract failed');
        const drop = new Event('drop', { bubbles: true });
        Object.defineProperty(drop, 'dataTransfer', { value: { getData: (type: string) => transfer.get(type) ?? '' } });
        target?.dispatchEvent(drop);
      })],
      checkpoints: [{ name: 'layout and drag/drop', afterAction: 0, assert: (runtime) => {
        if (!runtime.root.querySelector('.velvet-asset-grid--list')) throw new Error('AssetGrid list layout contract failed');
        if (runtime.root.querySelector('[data-pinned="true"]')?.getAttribute('aria-label') !== 'A') throw new Error('AssetGrid pinned projection contract failed');
        if (draggedAsset !== 'a' || droppedAsset !== 'a:b') throw new Error('AssetGrid drag/drop contract failed');
      } }],
    },
  },
];
