import assert from 'node:assert/strict';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of ['Document', 'Element', 'Event', 'HTMLElement', 'KeyboardEvent', 'Node']) {
  if (browser[name]) globalThis[name] = browser[name];
}
globalThis.window = browser;
globalThis.document = browser.document;
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const {
  VelvetAssetGrid,
  VelvetCommandPalette,
  VelvetDataTable,
  VelvetState,
  VelvetTabs,
} = await import('/tmp/velvet-plugin-tests/app/ui/velvet-ui.js');
const { applyUiThemeSnapshot, UiThemeService } = await import('/tmp/velvet-plugin-tests/app/ui/theme.js');

applyUiThemeSnapshot({
  schema: 'velvet.ui.theme',
  schemaVersion: 1,
  scheme: 'light',
  density: 'compact',
  contrast: 'high',
  reducedMotion: true,
  revision: 4,
  tokens: { 'surface.canvas': '#fff', 'focus.default': '#000' },
});
assert.equal(document.documentElement.dataset.velvetScheme, 'light');
assert.equal(document.documentElement.dataset.velvetDensity, 'compact');
assert.equal(document.documentElement.style.getPropertyValue('--velvet-surface-app'), '#fff');
const themeService = new UiThemeService();
assert.equal(themeService.toggleScheme().scheme, 'light');
assert.equal(document.documentElement.dataset.velvetScheme, 'light');

const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);
let selectedTab = 'one';
let selectedAsset;
let openedAsset;
let selectedCommand;

await act(async () => {
  root.render(React.createElement(
    React.Fragment,
    null,
    React.createElement(VelvetTabs, {
      tabs: [{ id: 'one', label: 'One' }, { id: 'two', label: 'Two' }],
      selectedId: selectedTab,
      onSelect: (id) => { selectedTab = id; },
    }),
    React.createElement(VelvetState, { kind: 'error', title: 'Failed', onRetry: () => {} }),
    React.createElement(VelvetAssetGrid, {
      items: [{ id: 'asset-1', label: 'Asset 1', selected: true, metadata: '2 tags' }, { id: 'asset-2', label: 'Asset 2' }],
      onSelect: (id) => { selectedAsset = id; },
      onOpen: (id) => { openedAsset = id; },
    }),
    React.createElement(VelvetDataTable, {
      rows: [{ id: 'row-1', name: 'Row 1' }],
      columns: [{ id: 'name', label: 'Name', render: (row) => row.name, sortable: true }],
      rowId: (row) => row.id,
      onSelect: (id) => { selectedAsset = id; },
    }),
    React.createElement(VelvetCommandPalette, {
      open: true,
      commands: [{ id: 'command.open', label: 'Open', shortcutHint: '⌘O' }],
      onSelect: (id) => { selectedCommand = id; },
      onClose: () => {},
    }),
  ));
});

assert.equal(rootElement.querySelector('[role="alert"]')?.textContent?.startsWith('Failed'), true);
assert.equal(rootElement.querySelectorAll('[role="gridcell"]').length, 2);
assert.equal(rootElement.querySelector('.velvet-asset-grid__metadata')?.textContent, '2 tags');
assert.equal(rootElement.querySelectorAll('tbody tr').length, 1);
assert.equal(rootElement.querySelector('[role="option"]')?.textContent?.includes('Open'), true);

await act(async () => {
  rootElement.querySelector('[role="tab"]')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  rootElement.querySelector('[role="gridcell"]')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  rootElement.querySelector('[role="option"]')?.click();
});
assert.equal(selectedTab, 'two');
assert.equal(openedAsset, 'asset-1');
assert.equal(selectedCommand, 'command.open');
assert.equal(selectedAsset, undefined);

await act(async () => root.unmount());
console.log('UI Kit component contract tests passed.');
