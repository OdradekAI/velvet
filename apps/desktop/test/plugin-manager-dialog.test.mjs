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
let installedState = 'active';
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
          ...(installed ? [summary('sample.installed', installedState)] : []),
        ],
      };
    }
    if (request.kind === 'install-plugin') {
      installed = true;
      installedState = 'registered';
      return { kind: 'extension', extension: summary('sample.installed', 'registered') };
    }
    if (request.kind === 'activate-plugin') {
      installedState = 'active';
      return { kind: 'extension', extension: summary(request.extensionId, 'active') };
    }
    if (request.kind === 'disable-plugin') {
      installedState = 'disabled';
      return { kind: 'extension', extension: summary(request.extensionId, 'disabled') };
    }
    if (request.kind === 'recover-plugin') {
      installedState = 'active';
      return { kind: 'extension', extension: summary(request.extensionId, 'active') };
    }
    if (request.kind === 'uninstall-plugin') {
      installed = false;
      return { kind: 'accepted' };
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
let confirmCalls = [];

await act(async () => {
  root.render(React.createElement(PluginManagerDialog, {
    onOpenChange: () => undefined,
    open: true,
    confirmUninstall: async (extensionId) => {
      confirmCalls.push(extensionId);
      return true;
    },
    pickDirectory: async () => '/tmp/native-plugin',
    plugins: client,
  }));
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.match(document.body.textContent ?? '', /sample\.reader/);
assert.match(document.body.textContent ?? '', /active/);

const input = document.querySelector('#velvet-plugin-source');
assert.ok(input);
const browseButton = [...document.querySelectorAll('button')]
  .find((button) => button.textContent === 'Browse…');
assert.ok(browseButton);
await act(async () => {
  browseButton.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(input?.value, '/tmp/native-plugin');

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

const disableButton = document.querySelector('[aria-label="Disable sample.installed"]');
assert.ok(disableButton);
await act(async () => {
  disableButton.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.match(document.body.textContent ?? '', /sample\.installed disabled/);
assert.ok(document.querySelector('[aria-label="Enable sample.installed"]'));

const enableButton = document.querySelector('[aria-label="Enable sample.installed"]');
assert.ok(enableButton);
await act(async () => {
  enableButton.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.match(document.body.textContent ?? '', /sample\.installed active/);

const uninstallButton = document.querySelector('[aria-label="Uninstall sample.installed"]');
assert.ok(uninstallButton);
await act(async () => {
  uninstallButton.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.deepEqual(confirmCalls, ['sample.installed']);
assert.equal(document.querySelector('[aria-label="Uninstall sample.installed"]'), null);
assert.equal(requests.at(-1)?.kind, 'query-extensions');
assert.equal(requests.find((request) => request.kind === 'install-plugin')?.sourceDir, '/tmp/native-plugin');

await act(() => root.unmount());
console.log('Plugin manager dialog tests passed.');

