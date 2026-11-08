import { useState } from 'react';
import type { UiKitStory } from '../../story-registry.js';
import { VelvetAssetGrid, VelvetZoomControl } from '../../host-projections/index.js';

let selectedAsset = '';
let openedAsset = '';
let draggedAsset = '';
let droppedAsset = '';
let selectionMode = '';

export const assetGridStories: readonly UiKitStory[] = [
  {
    id: 'asset-grid/default',
    componentId: 'asset-grid',
    name: 'Default',
    description: 'Asset grid.',
    render: () => <VelvetAssetGrid items={[{ id: 'a', label: 'A', selected: true }, { id: 'b', label: 'B' }]} onOpen={(id) => { openedAsset = id; }} onSelect={(id) => { selectedAsset = id; }} />,
    scenario: {
      covers: ['select', 'navigate', 'open'],
      setup: () => { selectedAsset = ''; openedAsset = ''; draggedAsset = ''; droppedAsset = ''; selectionMode = ''; },
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
    id: 'asset-grid/grid-semantics',
    componentId: 'asset-grid',
    name: 'Grid semantics',
    description: 'Multi-select and row/column metrics exposed to assistive technology.',
    render: () => <VelvetAssetGrid columns={2} items={[{ id: 'a', label: 'A', selected: true }, { id: 'b', label: 'B' }, { id: 'c', label: 'C' }]} onSelect={() => undefined} />,
    scenario: {
      covers: ['navigate'],
      setup: () => undefined,
      actions: [],
      checkpoints: [{ name: 'grid metrics', assert: (runtime) => {
        const grid = runtime.root.querySelector<HTMLElement>('[role="grid"]');
        const cells = [...runtime.root.querySelectorAll<HTMLElement>('[role="gridcell"]')];
        if (grid?.getAttribute('aria-multiselectable') !== 'true' || grid?.getAttribute('aria-rowcount') !== '2' || grid?.getAttribute('aria-colcount') !== '2') throw new Error('AssetGrid grid metrics contract failed');
        if (cells[0]?.getAttribute('aria-rowindex') !== '1' || cells[0]?.getAttribute('aria-colindex') !== '1' || cells[2]?.getAttribute('aria-rowindex') !== '2' || cells[2]?.getAttribute('aria-colindex') !== '1') throw new Error('AssetGrid cell position contract failed');
      } }],
    },
  },
  {
    id: 'asset-grid/layout-modes',
    componentId: 'asset-grid',
    name: 'Layout modes',
    description: 'List layout with optional drag and drop.',
    render: () => <VelvetAssetGrid layout="list" items={[{ id: 'a', label: 'A', pinned: true, metadata: '2 tags · 1920 × 1080' }, { id: 'b', label: 'B', aspectRatio: '16 / 9' }]} onSelect={() => undefined} onDragStart={(id) => { draggedAsset = id; }} onDrop={(sourceId, targetId) => { droppedAsset = `${sourceId}:${targetId}`; }} />,
    scenario: {
      covers: ['drag', 'drop'],
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
        if (runtime.root.querySelector('[aria-label="Pinned"]')?.textContent !== '⌖') throw new Error('AssetGrid pinned indicator contract failed');
        if (runtime.root.querySelector('.velvet-asset-grid__metadata')?.textContent !== '2 tags · 1920 × 1080') throw new Error('AssetGrid metadata slot contract failed');
        if (draggedAsset !== 'a' || droppedAsset !== 'a:b') throw new Error('AssetGrid drag/drop contract failed');
      } }],
    },
  },
  {
    id: 'asset-grid/selection-modifiers',
    componentId: 'asset-grid',
    name: 'Selection modifiers',
    description: 'Host receives additive and range selection modifiers without taking ownership of the anchor.',
    render: () => <VelvetAssetGrid items={[{ id: 'a', label: 'A', selected: true }, { id: 'b', label: 'B' }, { id: 'c', label: 'C' }]} onSelect={(id, additive, range) => { selectedAsset = id; selectionMode = `${additive ? 'additive' : 'replace'}:${range ? 'range' : 'single'}`; }} />,
    scenario: {
      covers: ['select'],
      setup: () => { selectedAsset = ''; selectionMode = ''; },
      actions: [
        (runtime) => runtime.act(async () => {
          const cell = runtime.root.querySelector<HTMLElement>('[role="gridcell"][aria-label="B"]');
          const MouseEventCtor = cell?.ownerDocument.defaultView?.MouseEvent;
          if (cell && MouseEventCtor) cell.dispatchEvent(new MouseEventCtor('click', { bubbles: true, shiftKey: true }));
        }),
      ],
      checkpoints: [{ name: 'range modifier', afterAction: 0, assert: () => {
        if (selectedAsset !== 'b' || selectionMode !== 'replace:range') throw new Error('AssetGrid range selection modifier contract failed');
      } }],
    },
  },
  {
    id: 'asset-grid/zoom-layout',
    componentId: 'asset-grid',
    name: 'Zoom layout projection',
    description: 'Host-controlled grid reflow using deterministic geometry checkpoints.',
    render: () => <ZoomLayoutStory />,
    scenario: {
      covers: ['navigate'],
      setup: () => undefined,
      actions: [async (runtime) => {
        for (let index = 0; index < 10; index += 1) {
          await runtime.act(async () => runtime.root.querySelector<HTMLButtonElement>('button[aria-label="Zoom in"]')?.click());
        }
      }],
      checkpoints: [{ name: 'host reflow at 230', afterAction: 0, assert: (runtime) => {
        if (runtime.root.querySelector('[data-ui-kit-observation="grid-columns"]')?.textContent !== '5') throw new Error('AssetGrid zoom layout projection contract failed');
      } }],
    },
  },
  {
    id: 'asset-grid/waterfall-layout',
    componentId: 'asset-grid',
    name: 'Waterfall layout',
    description: 'Aspect-ratio-preserving waterfall layout leaves row geometry host-owned.',
    render: () => <VelvetAssetGrid columns={3} layout="waterfall" items={[{ id: 'a', label: 'A', aspectRatio: '4 / 3' }, { id: 'b', label: 'B', aspectRatio: '3 / 4' }, { id: 'c', label: 'C', aspectRatio: '16 / 9' }]} onSelect={() => undefined} />,
    scenario: {
      actions: [],
      checkpoints: [{ name: 'waterfall geometry', assert: (runtime) => {
        const grid = runtime.root.querySelector<HTMLElement>('[role="grid"]');
        if (grid?.classList.contains('velvet-asset-grid--waterfall') !== true || grid?.hasAttribute('aria-rowcount') || grid?.hasAttribute('aria-colcount')) throw new Error('AssetGrid waterfall geometry contract failed');
      } }],
    },
  },
];

function ZoomLayoutStory() {
  const [zoom, setZoom] = useState(180);
  const columns = zoom >= 230 ? 5 : 8;
  return (
    <div>
      <VelvetZoomControl ariaLabel="Zoom level" max={500} min={25} onChange={setZoom} value={zoom} />
      <output data-ui-kit-observation="grid-columns">{columns}</output>
      <VelvetAssetGrid
        columns={columns}
        items={Array.from({ length: 12 }, (_, index) => ({ id: `asset-${index}`, label: `Asset ${index}` }))}
        onSelect={() => undefined}
      />
    </div>
  );
}
