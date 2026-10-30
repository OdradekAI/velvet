import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { symlinkSync } from 'node:fs';
import { Window } from 'happy-dom';

try {
  symlinkSync(
    fileURLToPath(new URL('../node_modules', import.meta.url)),
    '/tmp/velvet-plugin-tests/node_modules',
    'junction',
  );
} catch {
  // The compiled test tree may already point at the desktop dependencies.
}

const browser = new Window({ url: 'http://velvet.test/' });
const globals = [
  'Document',
  'DOMRect',
  'DOMRectReadOnly',
  'Element',
  'Event',
  'FocusEvent',
  'HTMLElement',
  'InputEvent',
  'KeyboardEvent',
  'MouseEvent',
  'MutationObserver',
  'Node',
  'PointerEvent',
  'CustomEvent',
  'ResizeObserver',
];
for (const name of globals) {
  if (browser[name]) {
    globalThis[name] = browser[name];
  }
}
globalThis.window = browser;
globalThis.document = browser.document;
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: browser.navigator,
});
globalThis.getComputedStyle = browser.getComputedStyle.bind(browser);
globalThis.requestAnimationFrame = browser.requestAnimationFrame.bind(browser);
globalThis.cancelAnimationFrame = browser.cancelAnimationFrame.bind(browser);
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { WorkbenchAppSurfaceHost } = await import('/tmp/velvet-plugin-tests/app/renderer/workbench-app-surface-host.js');
const { WorkbenchShell } = await import('/tmp/velvet-plugin-tests/app/renderer/workbench-shell.js');
const { PluginSurfaceHost } = await import('/tmp/velvet-plugin-tests/app/renderer/plugin-surface-host.js');
const { defaultLayoutState } = await import('/tmp/velvet-plugin-tests/workbench/layout/layout-state.js');
const { FocusService } = await import('/tmp/velvet-plugin-tests/workbench/focus/focus-service.js');
const { PluginUiRegistry } = await import('/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js');
const { InMemoryWorkspaceContext } = await import('/tmp/velvet-plugin-tests/workbench/services/workspace-context.js');
const { ViewRegistry } = await import('/tmp/velvet-plugin-tests/workbench/views/view-registry.js');
const { PluginClient } = await import('/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js');
const { WorkbenchAppRegistry } = await import('/tmp/velvet-plugin-tests/workbench/apps/workbench-app.js');

const layout = defaultLayoutState();
layout.containers['sample.global.tools'] = { visible: true, order: 0 };
layout.views['sample.global.first'] = { visible: true, order: 0, containerId: 'sample.global.tools' };
layout.views['sample.global.second'] = { visible: true, order: 1, containerId: 'sample.global.tools' };

const views = new ViewRegistry();
views.registerContainer({
  id: 'sample.global.tools',
  title: 'Global Tools',
  icon: 'tools',
  zone: 'contextual',
  preferredPart: 'auxiliary-sidebar',
  scope: { kind: 'workbench' },
  order: 0,
});
views.register({
  id: 'sample.global.first',
  containerId: 'sample.global.tools',
  title: 'First Tool',
  icon: 'one',
  zone: 'contextual',
  scope: { kind: 'workbench' },
  order: 0,
});
views.register({
  id: 'sample.global.second',
  containerId: 'sample.global.tools',
  title: 'Second Tool',
  icon: 'two',
  zone: 'contextual',
  scope: { kind: 'workbench' },
  order: 1,
});

const focus = new FocusService();
const commands = [];
const app = {
  id: 'asset-manager',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  surface: { trust: 'trusted', entrypoint: 'builtin://asset-manager' },
};
const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);

async function render(nextLayout) {
  await act(() => {
    root.render(React.createElement(WorkbenchShell, {
      apps: [app],
      focusService: focus,
      layout: nextLayout,
      onActivateApp: () => undefined,
      onLayoutCommand: (command) => commands.push(command),
      viewRegistry: views,
    }));
  });
}

function tick() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

async function settle(count = 2) {
  await act(async () => {
    for (let index = 0; index < count; index += 1) {
      await tick();
    }
  });
}

await render(layout);
await tick();
await tick();

const container = rootElement.querySelector('#velvet-view-container-sample-global-tools');
assert.ok(container);
assert.equal(container.getAttribute('aria-label'), 'Global Tools views');
assert.deepEqual(
  [...rootElement.querySelectorAll('.velvet-view')].map((element) => element.getAttribute('aria-label')),
  ['First Tool', 'Second Tool'],
);

await act(() => {
  focus.focus({ kind: 'part', part: 'auxiliary-sidebar' });
  focus.focus({ kind: 'view', viewId: 'sample.global.first' });
});
await tick();
assert.equal(
  document.activeElement?.dataset.velvetFocusTarget,
  'view:sample.global.first',
);

const resizeHandle = rootElement.querySelector('.velvet-resize-handle--sidebar-start');
assert.ok(resizeHandle);
await act(() => {
  resizeHandle.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowLeft' }));
});
assert.deepEqual(commands.at(-1), {
  id: 'workbench.resize-part',
  part: 'auxiliary-sidebar',
  size: 310,
});

const hiddenLayout = {
  ...layout,
  revision: layout.revision + 1,
  views: {
    ...layout.views,
    'sample.global.first': { ...layout.views['sample.global.first'], visible: false },
  },
};
await render(hiddenLayout);
await tick();
await tick();
assert.equal(
  document.activeElement?.dataset.velvetFocusTarget,
  'part:auxiliary-sidebar',
);

await render(layout);
await tick();
const restoredContainer = rootElement.querySelector('#velvet-view-container-sample-global-tools');
await act(() => {
  restoredContainer.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, button: 2 }));
});
await tick();
assert.ok(
  [...document.querySelectorAll('.velvet-context-menu__item')]
    .some((item) => item.textContent === 'Hide Global Tools'),
);

await act(() => root.unmount());
await tick();

const surfaceUi = new PluginUiRegistry();
surfaceUi.register({
  id: 'sample.reader.surface',
  extensionId: 'sample.extension',
  namespace: 'workbench',
  kind: 'view',
  entrypoint: 'https://plugin.example/reader',
  bridgeVersion: '1',
});
const surfaceContext = new InMemoryWorkspaceContext();
const surfaceRootElement = document.createElement('div');
document.body.append(surfaceRootElement);
const surfaceRoot = createRoot(surfaceRootElement);
await act(() => {
  surfaceRoot.render(React.createElement(PluginSurfaceHost, {
    context: surfaceContext,
    extensionId: 'sample.extension',
    surfaceId: 'sample.reader.surface',
    ui: surfaceUi,
  }));
});
await tick();
await tick();
assert.equal(surfaceRootElement.querySelector('iframe')?.src, 'https://plugin.example/reader');
assert.equal(surfaceRootElement.querySelector('[data-plugin-surface-state="active"]') !== null, true);
assert.equal(surfaceUi.getSession('sample.extension', 'sample.reader.surface')?.state, 'active');

await act(() => surfaceRoot.unmount());
await tick();
await tick();
assert.equal(surfaceUi.getSession('sample.extension', 'sample.reader.surface'), undefined);
assert.equal(surfaceRootElement.querySelector('iframe'), null);

const strictSurfaceUi = new PluginUiRegistry();
strictSurfaceUi.register({
  id: 'sample.strict.surface',
  extensionId: 'sample.strict-extension',
  namespace: 'workbench',
  kind: 'view',
  entrypoint: 'https://plugin.example/strict',
  bridgeVersion: '1',
});
const strictSurfaceRootElement = document.createElement('div');
document.body.append(strictSurfaceRootElement);
const strictSurfaceRoot = createRoot(strictSurfaceRootElement);
await act(() => {
  strictSurfaceRoot.render(React.createElement(
    React.StrictMode,
    null,
    React.createElement(PluginSurfaceHost, {
      context: new InMemoryWorkspaceContext(),
      extensionId: 'sample.strict-extension',
      surfaceId: 'sample.strict.surface',
      ui: strictSurfaceUi,
    }),
  ));
});
await settle(4);
assert.equal(
  strictSurfaceRootElement.querySelector('[data-plugin-surface-state="active"]') !== null,
  true,
);
assert.equal(strictSurfaceUi.getSession('sample.strict-extension', 'sample.strict.surface')?.state, 'active');
await act(() => strictSurfaceRoot.unmount());
await settle(2);
assert.equal(strictSurfaceUi.getSession('sample.strict-extension', 'sample.strict.surface'), undefined);

const failingSurfaceUi = new PluginUiRegistry();
failingSurfaceUi.register({
  id: 'sample.failure.surface',
  extensionId: 'sample.failure-extension',
  namespace: 'workbench',
  kind: 'view',
  entrypoint: 'https://plugin.example/failure',
  bridgeVersion: '1',
});
let failingSurfaceAttempts = 0;
failingSurfaceUi.bindAdapter('sample.failure-extension', 'sample.failure.surface', {
  mount: () => {
    failingSurfaceAttempts += 1;
    if (failingSurfaceAttempts === 1) {
      throw new Error('isolated surface boot failed');
    }
  },
  unmount: () => undefined,
});
const failingSurfaceRootElement = document.createElement('div');
document.body.append(failingSurfaceRootElement);
const failingSurfaceRoot = createRoot(failingSurfaceRootElement);
await act(() => {
  failingSurfaceRoot.render(React.createElement(PluginSurfaceHost, {
    context: new InMemoryWorkspaceContext(),
    extensionId: 'sample.failure-extension',
    surfaceId: 'sample.failure.surface',
    ui: failingSurfaceUi,
  }));
});
await settle(4);
assert.equal(failingSurfaceAttempts, 1);
assert.equal(
  failingSurfaceRootElement.querySelector('[data-plugin-surface-state="failed"]') !== null,
  true,
);
assert.match(
  failingSurfaceRootElement.querySelector('[role="alert"]')?.textContent ?? '',
  /isolated surface boot failed/,
);
await act(() => {
  failingSurfaceRootElement.querySelector('[data-plugin-surface-retry]').click();
});
await settle(4);
assert.equal(failingSurfaceAttempts, 2);
assert.equal(
  failingSurfaceRootElement.querySelector('[data-plugin-surface-state="active"]') !== null,
  true,
);
await act(() => failingSurfaceRoot.unmount());
await settle();

const appRegistry = new WorkbenchAppRegistry([{
  id: 'asset-manager',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  surface: { trust: 'trusted', entrypoint: 'builtin://asset-manager' },
}]);
let trustedAppMounts = 0;
let trustedAppDisposals = 0;
appRegistry.bindSurfaceFactory('asset-manager', () => ({
  mount: () => {
    trustedAppMounts += 1;
  },
  activate: () => undefined,
  deactivate: () => undefined,
  dispose: () => {
    trustedAppDisposals += 1;
  },
}));
const appUi = new PluginUiRegistry();
const appPlugins = new PluginClient(
  { request: async () => ({ kind: 'accepted' }), subscribe: () => () => {} },
  undefined,
  appRegistry,
  '1',
  appUi,
);
const appManifest = {
  id: 'sample.app-extension',
  version: '1.0.0',
  hostApi: '1',
  permissions: {},
  workbenchApp: {
    id: 'sample.app-extension.research',
    apiVersion: '1',
    title: 'Research App',
    activityBarIcon: 'search',
    surface: {
      entrypoint: 'https://plugin.example/research-app',
      bridgeVersion: '1',
    },
  },
  contributions: [],
};
appPlugins.register(appManifest);
appPlugins.activate(appManifest.id);
const appContext = new InMemoryWorkspaceContext();
const appRootElement = document.createElement('div');
document.body.append(appRootElement);
const appRoot = createRoot(appRootElement);
const renderAppSurface = (appId) => appRoot.render(React.createElement(WorkbenchAppSurfaceHost, {
  appId,
  apps: appRegistry,
  context: appContext,
  dispatch: () => undefined,
  fallback: React.createElement('div', { className: 'fallback-surface' }, 'Fallback'),
  plugins: appPlugins,
}));

await act(() => renderAppSurface('sample.app-extension.research'));
await settle();
assert.equal(appRootElement.querySelector('iframe')?.src, 'https://plugin.example/research-app');
assert.equal(
  appRegistry.getSurfaceState('sample.app-extension.research'),
  'active',
);

const failingAppRegistry = new WorkbenchAppRegistry([{
  id: 'asset-manager',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  surface: { trust: 'trusted', entrypoint: 'builtin://asset-manager' },
}]);
const failingAppUi = new PluginUiRegistry();
const failingAppPlugins = new PluginClient(
  { request: async () => ({ kind: 'accepted' }), subscribe: () => () => {} },
  undefined,
  failingAppRegistry,
  '1',
  failingAppUi,
);
const failingAppManifest = {
  id: 'sample.failing-app-extension',
  version: '1.0.0',
  hostApi: '1',
  permissions: {},
  workbenchApp: {
    id: 'sample.failing-app-extension.research',
    apiVersion: '1',
    title: 'Failing Research App',
    activityBarIcon: 'search',
    surface: {
      entrypoint: 'https://plugin.example/failing-app',
      bridgeVersion: '1',
    },
  },
  contributions: [],
};
failingAppPlugins.register(failingAppManifest);
failingAppPlugins.activate(failingAppManifest.id);
const failingAppBinding = failingAppPlugins.listAppSurfaceBindings()[0];
let failingAppAttempts = 0;
failingAppUi.bindAdapter(failingAppBinding.extensionId, failingAppBinding.surfaceId, {
  mount: () => {
    failingAppAttempts += 1;
    if (failingAppAttempts === 1) {
      throw new Error('app surface boot failed');
    }
  },
  unmount: () => undefined,
});
const failingAppRootElement = document.createElement('div');
document.body.append(failingAppRootElement);
const failingAppRoot = createRoot(failingAppRootElement);
const renderFailingAppSurface = () => failingAppRoot.render(React.createElement(WorkbenchAppSurfaceHost, {
  appId: failingAppManifest.workbenchApp.id,
  apps: failingAppRegistry,
  context: new InMemoryWorkspaceContext(),
  dispatch: () => undefined,
  fallback: React.createElement('div', { className: 'fallback-surface' }, 'Fallback'),
  plugins: failingAppPlugins,
}));
await act(renderFailingAppSurface);
await settle(5);
assert.equal(failingAppAttempts, 1);
assert.equal(
  failingAppRootElement.querySelector('.velvet-app-surface-host') !== null,
  true,
);
assert.equal(
  failingAppRootElement.querySelector('[data-surface-state="failed"]') !== null,
  true,
);
assert.match(
  failingAppRootElement.querySelector('[role="alert"]')?.textContent ?? '',
  /app surface boot failed/,
);
await act(() => {
  failingAppRootElement.querySelector('[data-app-surface-retry]').click();
});
await settle(5);
assert.equal(failingAppAttempts, 2);
assert.equal(
  failingAppRootElement.querySelector('[data-surface-state="active"]') !== null,
  true,
);
await act(() => failingAppRoot.unmount());
await settle();
await failingAppPlugins.deactivate(failingAppManifest.id);

await act(() => renderAppSurface('asset-manager'));
await settle();
assert.equal(appRootElement.querySelector('[data-trusted-app-surface="asset-manager"]') !== null, true);
assert.equal(appRootElement.querySelector('[data-surface-state="active"]') !== null, true);
assert.equal(trustedAppMounts, 1);
assert.equal(
  appRegistry.getSurfaceState('sample.app-extension.research'),
  'suspended',
);

await act(() => renderAppSurface('sample.app-extension.research'));
await settle();
assert.equal(appRootElement.querySelector('iframe')?.src, 'https://plugin.example/research-app');
assert.equal(
  appRegistry.getSurfaceState('sample.app-extension.research'),
  'active',
);
assert.equal(trustedAppDisposals, 0);

await act(async () => {
  await appPlugins.deactivate(appManifest.id);
  await tick();
  await tick();
});
assert.equal(appRegistry.get('sample.app-extension.research'), undefined);
assert.equal(appUi.getSession('sample.app-extension', 'sample.app-extension.research.surface'), undefined);
assert.equal(appRootElement.querySelector('iframe'), null);
await act(() => appRoot.unmount());

console.log('Workbench real DOM regression tests passed.');
