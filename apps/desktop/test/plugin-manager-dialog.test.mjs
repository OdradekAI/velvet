import assert from 'node:assert/strict';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of [
  'Document',
  'Element',
  'Event',
  'FocusEvent',
  'HTMLElement',
  'HTMLInputElement',
  'KeyboardEvent',
  'MouseEvent',
  'MutationObserver',
  'Node',
  'NodeFilter',
  'PointerEvent',
]) {
  if (browser[name]) {
    globalThis[name] = browser[name];
  }
}
globalThis.Event = browser.Event;
globalThis.CustomEvent = browser.CustomEvent;
globalThis.FocusEvent = browser.FocusEvent;
globalThis.KeyboardEvent = browser.KeyboardEvent;
globalThis.MouseEvent = browser.MouseEvent;
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
const { PluginManagerDialog } = await import('/tmp/velvet-plugin-tests/app/renderer/plugin-manager-dialog.js');
const { PluginClient } = await import('/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js');
const { PluginUiRegistry } = await import('/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js');
const { ViewRegistry } = await import('/tmp/velvet-plugin-tests/workbench/views/view-registry.js');
const { WorkbenchAppRegistry } = await import('/tmp/velvet-plugin-tests/workbench/apps/workbench-app.js');
const { CommandRegistry } = await import('/tmp/velvet-plugin-tests/workbench/commands/command-registry.js');
const { ContextKeyService } = await import('/tmp/velvet-plugin-tests/workbench/commands/context-key-service.js');
const { MenuRegistry } = await import('/tmp/velvet-plugin-tests/workbench/commands/menu-registry.js');
const { OverlayRegistry } = await import('/tmp/velvet-plugin-tests/workbench/overlays/overlay-registry.js');

const requests = [];
let installed = false;
const summary = (id, state) => ({
  id,
  version: id === 'sample.installed' ? '0.1.0' : '1.0.0',
  active: state === 'active',
  state,
});
const core = {
  request: async (request) => {
    requests.push(request);
    if (request.kind === 'query-extensions') {
      return {
        kind: 'extensions',
        extensions: [
          summary('sample.reader', 'active'),
          ...(installed ? [summary('sample.installed', 'active')] : []),
        ],
      };
    }
    if (request.kind === 'install-plugin') {
      installed = true;
      return { kind: 'extension', extension: summary('sample.installed', 'registered') };
    }
    if (request.kind === 'activate-plugin') {
      return { kind: 'extension', extension: summary(request.extensionId, 'active') };
    }
    if (request.kind === 'query-plugin-projection') {
      return {
        kind: 'plugin-projection',
        projection: {
          summary: summary(request.extensionId, 'active'),
          targetApp: null,
          contributions: [],
        },
      };
    }
    return { kind: 'accepted' };
  },
  subscribe: () => () => {},
};

const apps = new WorkbenchAppRegistry([]);
const client = new PluginClient(
  core,
  undefined,
  apps,
  '1',
  new PluginUiRegistry(),
  new ViewRegistry(),
  new CommandRegistry(new ContextKeyService()),
  undefined,
  new OverlayRegistry(new ContextKeyService()),
);
const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);

await act(async () => {
  root.render(React.createElement(PluginManagerDialog, {
    onOpenChange: () => undefined,
    open: true,
    plugins: client,
  }));
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.match(document.body.textContent ?? '', /sample\.reader/);
assert.match(document.body.textContent ?? '', /active/);

const input = document.querySelector('#velvet-plugin-source');
assert.ok(input);
const setInputValue = Object.getOwnPropertyDescriptor(
  browser.HTMLInputElement.prototype,
  'value',
).set;
setInputValue.call(input, '/tmp/sample-plugin');
await act(() => {
  input.dispatchEvent(new browser.Event('input', { bubbles: true }));
});
const installButton = [...document.querySelectorAll('button')]
  .find((button) => button.textContent === 'Install and activate');
assert.ok(installButton);
await act(async () => {
  installButton.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.match(document.body.textContent ?? '', /sample\.installed installed and activated/);
assert.match(document.body.textContent ?? '', /sample\.installed/);
assert.deepEqual(
  requests.map((request) => request.kind),
  ['query-extensions', 'install-plugin', 'query-plugin-projection', 'activate-plugin', 'query-plugin-projection', 'query-extensions'],
);

await act(() => root.unmount());
console.log('Plugin manager dialog tests passed.');

