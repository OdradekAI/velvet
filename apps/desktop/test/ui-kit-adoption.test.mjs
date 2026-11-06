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
const { RecoverySurface } = await import('/tmp/velvet-plugin-tests/app/renderer/recovery-surface.js');
const { JobOperationsPanel } = await import('/tmp/velvet-plugin-tests/app/renderer/job-operations-panel.js');

const host = document.createElement('div');
document.body.append(host);
const root = createRoot(host);

await act(async () => {
  root.render(React.createElement(RecoverySurface, { onRetry: () => {} }));
});
assert.ok(host.querySelector('[data-ui-kit-component="button"]'));
assert.ok(host.querySelector('[data-ui-kit-component="state"]') === null);

const listeners = new Set();
const jobs = {
  list: () => [],
  subscribe: (listener) => { listeners.add(listener); return () => listeners.delete(listener); },
  query: async () => undefined,
  create: async () => undefined,
  cancel: async () => undefined,
  retry: async () => undefined,
};
await act(async () => {
  root.render(React.createElement(JobOperationsPanel, { jobs }));
});
assert.equal(host.querySelectorAll('[data-ui-kit-component="button"]').length, 3);
assert.ok(host.querySelector('[data-ui-kit-component="state"]'));

await act(async () => root.unmount());
console.log('UI Kit adoption fixtures passed.');
