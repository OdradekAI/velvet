import assert from 'node:assert/strict';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of [
  'Document',
  'Element',
  'Event',
  'HTMLElement',
  'HTMLInputElement',
  'InputEvent',
  'KeyboardEvent',
  'MouseEvent',
  'MutationObserver',
  'Node',
  'PointerEvent',
]) {
  if (browser[name]) {
    globalThis[name] = browser[name];
  }
}
globalThis.Event = browser.Event;
globalThis.CustomEvent = browser.CustomEvent;
globalThis.window = browser;
globalThis.document = browser.document;
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: browser.navigator,
});
globalThis.getComputedStyle = browser.getComputedStyle.bind(browser);
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { PluginRecoveryOperationsPanel } = await import(
  '/tmp/velvet-plugin-tests/app/renderer/plugin-recovery-operations-panel.js',
);

let logs = [
  {
    extensionId: 'sample.reader',
    action: 'discard',
    status: 'succeeded',
    attempt: 1,
    message: 'orphan recovery directory removed',
    removedPaths: ['/tmp/.sample.reader.staging-old'],
    occurredAtMs: 200,
  },
  {
    extensionId: 'sample.reader',
    action: 'rollback',
    status: 'failed',
    attempt: 2,
    message: 'temporary blocker',
    removedPaths: [],
    occurredAtMs: 100,
  },
];
const listeners = new Set();
let exportDestination;
let clearCalls = 0;
const plugins = {
  queryCoreInstallRecoveryLog: async () => logs,
  clearCoreInstallRecoveryLog: async () => {
    clearCalls += 1;
    logs = [];
    for (const listener of listeners) listener();
  },
  subscribe: (listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};

const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);
await act(async () => {
  root.render(React.createElement(PluginRecoveryOperationsPanel, {
    exportLogs: async () => {
      exportDestination = '/tmp/recovery-export.json';
      return exportDestination;
    },
    plugins,
  }));
  await new Promise((resolve) => setTimeout(resolve, 10));
});

assert.equal(rootElement.querySelectorAll('[data-plugin-recovery-log-entry]').length, 2);
const search = rootElement.querySelector('[data-plugin-recovery-log-search]');
assert.ok(search);
await act(async () => {
  rootElement.querySelector('select[data-plugin-recovery-log-status]').value = 'failed';
  rootElement.querySelector('select[data-plugin-recovery-log-status]').dispatchEvent(
    new Event('change', { bubbles: true }),
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(rootElement.querySelectorAll('[data-plugin-recovery-log-entry]').length, 1);
assert.match(rootElement.textContent ?? '', /temporary blocker/);

await act(async () => {
  rootElement.querySelector('select[data-plugin-recovery-log-action]').value = 'rollback';
  rootElement.querySelector('select[data-plugin-recovery-log-action]').dispatchEvent(
    new Event('change', { bubbles: true }),
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(rootElement.querySelectorAll('[data-plugin-recovery-log-entry]').length, 1);

await act(async () => {
  rootElement.querySelector('select[data-plugin-recovery-log-status]').value = 'all';
  rootElement.querySelector('select[data-plugin-recovery-log-status]').dispatchEvent(
    new Event('change', { bubbles: true }),
  );
  rootElement.querySelector('select[data-plugin-recovery-log-action]').value = 'all';
  rootElement.querySelector('select[data-plugin-recovery-log-action]').dispatchEvent(
    new Event('change', { bubbles: true }),
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
});
await act(async () => {
  rootElement.querySelector('[data-plugin-recovery-log-export]').click();
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(exportDestination, '/tmp/recovery-export.json');
assert.match(rootElement.textContent ?? '', /exported to \/tmp\/recovery-export\.json/);

await act(async () => {
  rootElement.querySelector('[data-plugin-recovery-log-clear]').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.equal(clearCalls, 1);
assert.equal(rootElement.querySelectorAll('[data-plugin-recovery-log-entry]').length, 0);
assert.match(rootElement.textContent ?? '', /Recovery log cleared/);
assert.match(rootElement.textContent ?? '', /No plugin recovery log entries/);

await act(() => root.unmount());
