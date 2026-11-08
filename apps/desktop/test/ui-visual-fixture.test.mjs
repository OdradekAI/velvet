import assert from 'node:assert/strict';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/?fixture=visual-baseline' });
for (const name of ['Document', 'Element', 'Event', 'HTMLElement', 'HTMLImageElement', 'KeyboardEvent', 'Node']) {
  if (browser[name]) globalThis[name] = browser[name];
}
globalThis.window = browser;
globalThis.document = browser.document;
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { VisualQualificationFixture } = await import('/tmp/velvet-plugin-tests/app/renderer/visual-qualification-fixture.js');

const host = document.createElement('div');
document.body.append(host);
const root = createRoot(host);

await act(async () => root.render(React.createElement(VisualQualificationFixture)));

assert.equal(host.querySelector('[data-ui-kit-fixture="visual-baseline"]')?.getAttribute('data-ui-kit-viewport'), '1280x800');
for (const probe of ['title-bar', 'navigation', 'toolbar', 'facets', 'workspace', 'grid', 'inspector', 'selection-preview', 'operations', 'zoom-control']) {
  assert.ok(host.querySelector(`[data-ui-kit-probe="${probe}"]`), `missing visual probe: ${probe}`);
}
assert.equal(host.querySelectorAll('[data-ui-kit-component="button"]').length >= 10, true);
assert.equal(host.querySelectorAll('[data-ui-kit-component="badge"]').length >= 5, true);
assert.equal(host.querySelectorAll('[role="gridcell"]').length, 9);
assert.ok(host.querySelector('[role="gridcell"][aria-selected="true"]'));
assert.ok(host.querySelector('input[aria-label="Search assets"]'));
assert.ok(host.querySelector('select[aria-label="Sort assets"]'));
assert.equal(host.querySelector('input[aria-label="Zoom level"]')?.getAttribute('step'), '5');
assert.equal(host.querySelector('input[aria-label="Zoom level"]')?.getAttribute('value'), '180');
assert.ok(host.querySelector('[data-ui-kit-probe="dialog-trigger"]'));
assert.ok(host.querySelector('[data-ui-kit-probe="context-trigger"]'));
assert.deepEqual(
  [...host.querySelectorAll('[data-ui-kit-probe="facets"] .velvet-visual-fixture__facet')].map((node) => node.textContent?.trim()),
  ['Color', 'Tags', 'Folders', 'Shapes', 'Rating', 'Format'],
);
assert.deepEqual(
  [...host.querySelectorAll('[data-ui-kit-probe="facets"] .velvet-visual-fixture__facet [data-icon-name]')].map((node) => node.getAttribute('data-icon-name')),
  ['color', 'tag', 'folder', 'shape', 'rating', 'format'],
);
assert.equal(host.querySelector('.velvet-visual-fixture__sidebar-filter')?.textContent?.trim(), 'Filter');
assert.equal(host.querySelector('.velvet-visual-fixture__sidebar-filter [data-icon-name="filter"]') !== null, true);
assert.equal(host.querySelector('[role="dialog"]'), null);
assert.equal(host.querySelector('.velvet-context-menu'), null);

await act(async () => root.unmount());
console.log('UI visual qualification fixture passed.');
