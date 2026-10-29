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
    if (request.kind === 'query-plugin-install-recoveries') {
      return { kind: 'plugin-install-recoveries', recoveries: [] };
    }
    if (request.kind === 'install-plugin') {
      installed = true;
      installedState = 'registered';
      return { kind: 'extension', extension: summary('sample.installed', 'registered') };
    }
    if (request.kind === 'inspect-plugin') {
      return {
        kind: 'plugin-install-preview',
        preview: {
          manifest: {
            id: 'sample.installed',
            version: '0.1.0',
            hostApi: '1',
            runtime: 'sidecar',
            launch: { program: 'sample-sidecar', args: [] },
            targetApp: null,
            contributions: [],
            permissionSets: [{ zone: 'navigation', permissions: ['library-read', 'asset-read'] }],
          },
          existing: null,
          operation: 'install',
          compatible: true,
          canInstall: true,
          issues: [],
        },
      };
    }
    if (request.kind === 'commit-plugin-install') {
      return { kind: 'accepted' };
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
assert.ok(document.querySelector('[data-plugin-install-preview]'));
assert.match(document.body.textContent ?? '', /library read, asset read/);

const installButton = [...document.querySelectorAll('button')]
  .find((button) => button.textContent === 'Review manifest');
assert.ok(installButton);
await act(async () => {
  installButton.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.match(document.body.textContent ?? '', /sample\.installed installed and activated/);
assert.match(document.body.textContent ?? '', /sample\.installed/);
assert.deepEqual(
  requests.slice(0, 10).map((request) => request.kind),
  [
    'query-extensions',
    'query-plugin-install-recoveries',
    'inspect-plugin',
    'install-plugin',
    'query-plugin-projection',
    'activate-plugin',
    'query-plugin-projection',
    'commit-plugin-install',
    'query-extensions',
    'query-plugin-install-recoveries',
  ],
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
assert.equal(requests.at(-1)?.kind, 'query-plugin-install-recoveries');
assert.equal(requests.find((request) => request.kind === 'install-plugin')?.sourceDir, '/tmp/native-plugin');

await act(() => root.unmount());

const recoveryRequests = [];
let recoveryVisible = true;
let recoveryState = 'registered';
const recoverySummary = () => ({
  id: 'sample.recovery',
  version: recoveryState === 'active' ? '0.1.0' : '0.2.0',
  active: recoveryState === 'active',
  state: recoveryState,
});
const recoveryCore = {
  request: async (request) => {
    recoveryRequests.push(request);
    if (request.kind === 'query-extensions') {
      return { kind: 'extensions', extensions: [recoverySummary()] };
    }
    if (request.kind === 'query-plugin-install-recoveries') {
      return {
        kind: 'plugin-install-recoveries',
        recoveries: recoveryVisible ? [{
          extensionId: 'sample.recovery',
          kind: 'rollback',
          operation: 'upgrade',
          candidateVersion: '0.2.0',
          current: recoverySummary(),
          rollbackVersion: '0.1.0',
          canResume: false,
          canCommit: true,
          canRollback: true,
          canDiscard: true,
          issues: [],
        }] : [],
      };
    }
    if (request.kind === 'resolve-plugin-install-recovery') {
      recoveryVisible = false;
      recoveryState = 'registered';
      return { kind: 'accepted' };
    }
    if (request.kind === 'activate-plugin') {
      recoveryState = 'active';
      return { kind: 'extension', extension: recoverySummary() };
    }
    if (request.kind === 'query-plugin-projection') {
      return {
        kind: 'plugin-projection',
        projection: {
          summary: recoverySummary(),
          targetApp: null,
          contributions: [],
        },
      };
    }
    return { kind: 'accepted' };
  },
  subscribe: () => () => {},
};
const recoveryClient = new PluginClient(recoveryCore);
const recoveryRootElement = document.createElement('div');
document.body.append(recoveryRootElement);
const recoveryRoot = createRoot(recoveryRootElement);
await act(async () => {
  recoveryRoot.render(React.createElement(PluginManagerDialog, {
    onOpenChange: () => undefined,
    open: true,
    plugins: recoveryClient,
  }));
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.ok(document.querySelector('[data-plugin-install-recoveries]'));
assert.match(document.body.textContent ?? '', /sample\.recovery/);
const restoreButton = [...document.querySelectorAll('button')]
  .find((button) => button.textContent === 'Restore previous');
assert.ok(restoreButton);
await act(async () => {
  restoreButton.click();
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.match(document.body.textContent ?? '', /recovery resolved and activated/);
assert.equal(document.querySelector('[data-plugin-install-recoveries]'), null);
assert.ok(recoveryRequests.some((request) => request.kind === 'resolve-plugin-install-recovery'));
await act(() => recoveryRoot.unmount());
console.log('Plugin manager dialog tests passed.');

