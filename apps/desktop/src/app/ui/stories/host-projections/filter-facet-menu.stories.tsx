import { useState } from 'react';
import type { UiKitStory } from '../../story-registry.js';
import { VelvetFilterFacetMenu, type VelvetFilterFolderOption, type VelvetFilterFolderRule } from '../../host-projections/index.js';

const folderOptions: readonly VelvetFilterFolderOption[] = [
  { id: 'reference', label: 'Reference', count: 12, depth: 0, state: 'included' },
  { id: 'illustration', label: 'Illustration', count: 8, depth: 1 },
  { id: 'archive', label: 'Archive', count: 3, depth: 0, state: 'excluded' },
];

const colorSwatches = [
  { id: 'black', color: '#111111', label: 'Black' },
  { id: 'white', color: '#FFFFFF', label: 'White' },
  { id: 'blue', color: '#2B76E7', label: 'Blue', selected: true },
  { id: 'purple', color: '#6D50ED', label: 'Purple' },
] as const;

let folderAction = '';
let colorAction = '';
let folderRule: VelvetFilterFolderRule = 'OR';
let changeColorAccuracy: ((value: number) => void) | undefined;

export const filterFacetMenuStories: readonly UiKitStory[] = [
  {
    id: 'filter-facet-menu/folder-default',
    componentId: 'filter-facet-menu',
    name: 'Folder default',
    description: 'Observed folder menu geometry with include/exclude and rule actions.',
    render: () => <FolderStory />,
    scenario: {
      covers: ['query', 'folder-rule', 'folder-include', 'folder-exclude', 'clear', 'close'],
      setup: () => { folderAction = ''; folderRule = 'OR'; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLInputElement>('input[type="search"]')?.dispatchEvent(new Event('input', { bubbles: true })); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-filter-rule="AND"]')?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-filter-option="reference"]')?.click(); }),
        (runtime) => runtime.act(async () => { const option = runtime.root.querySelector<HTMLButtonElement>('[data-filter-option="archive"]'); option?.dispatchEvent(new Event('contextmenu', { bubbles: true, cancelable: true })); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-filter-action="clear"]')?.click(); }),
      ],
      checkpoints: [
        { name: 'menu geometry', assert: (runtime) => { if (runtime.root.querySelector('[data-filter-facet-kind="folder"]') === null || runtime.root.querySelector('[data-filter-rule="OR"]') === null) throw new Error('Folder facet menu geometry contract failed'); } },
        { name: 'rule and actions', afterAction: 3, assert: () => { if (folderRule !== 'AND' || folderAction !== 'archive:exclude') throw new Error('Folder facet menu action contract failed'); } },
        { name: 'clear', afterAction: 4, assert: () => { if (folderAction !== 'clear') throw new Error('Folder facet menu clear contract failed'); } },
      ],
    },
  },
  {
    id: 'filter-facet-menu/color-default',
    componentId: 'filter-facet-menu',
    name: 'Color default',
    description: 'Observed color palette, hex input and accuracy range.',
    render: () => <ColorStory />,
    scenario: {
      covers: ['color-select', 'color-hex', 'color-accuracy'],
      setup: () => { colorAction = ''; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-filter-color="purple"]')?.click(); }),
        (runtime) => runtime.act(async () => { const input = runtime.root.querySelector<HTMLInputElement>('[aria-label="Hex color"]'); if (input) { input.value = '#FF2727'; input.dispatchEvent(new Event('input', { bubbles: true })); } }),
        (runtime) => runtime.act(async () => { changeColorAccuracy?.(30); }),
      ],
      checkpoints: [
        { name: 'color geometry', assert: (runtime) => { if (runtime.root.querySelectorAll('[data-filter-color]').length !== 6 || runtime.root.querySelector('[aria-label="Hex color"]') === null) throw new Error('Color facet menu geometry contract failed'); } },
        { name: 'color actions', afterAction: 2, assert: () => { if (colorAction !== 'accuracy:30') throw new Error('Color facet menu action contract failed'); } },
      ],
    },
  },
  {
    id: 'filter-facet-menu/closed',
    componentId: 'filter-facet-menu',
    name: 'Closed',
    description: 'Closed facet menu does not render or own filter state.',
    render: () => <VelvetFilterFacetMenu kind="folder" open={false} options={[]} />,
    scenario: { actions: [], checkpoints: [{ name: 'closed', assert: (runtime) => { if (runtime.root.querySelector('[data-ui-kit-component="filter-facet-menu"]') !== null) throw new Error('Closed facet menu contract failed'); } }] },
  },
];

function FolderStory() {
  const [query, setQuery] = useState('');
  const [options, setOptions] = useState(folderOptions);
  return <VelvetFilterFacetMenu
    kind="folder"
    onClear={() => { folderAction = 'clear'; setOptions(folderOptions.map((option) => ({ ...option, state: 'idle' }))); }}
    onClose={() => { folderAction = 'close'; }}
    onOptionAction={(id, action) => { folderAction = `${id}:${action}`; }}
    onQueryChange={setQuery}
    onRuleChange={(rule) => { folderRule = rule; }}
    open
    options={options}
    query={query}
    rule={folderRule}
    title="Folders"
  />;
}

function ColorStory() {
  const [hex, setHex] = useState('#FFFFFF');
  changeColorAccuracy = (value) => { colorAction = `accuracy:${value}`; };
  return <VelvetFilterFacetMenu
    accuracy={20}
    graySelected={false}
    hexValue={hex}
    kind="color"
    onAccuracyChange={changeColorAccuracy}
    onHexChange={(value) => { colorAction = `hex:${value}`; setHex(value); }}
    onPresetChange={(value) => { colorAction = `preset:${value ?? 'none'}`; }}
    open
    selectedColor="#2B76E7"
    swatches={colorSwatches}
    title="Color"
  />;
}
