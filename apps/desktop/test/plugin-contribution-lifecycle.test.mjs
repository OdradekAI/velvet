import assert from 'node:assert/strict';
import { CommandRegistry } from '/tmp/velvet-plugin-tests/workbench/commands/command-registry.js';
import { ContextKeyService } from '/tmp/velvet-plugin-tests/workbench/commands/context-key-service.js';
import { MenuRegistry } from '/tmp/velvet-plugin-tests/workbench/commands/menu-registry.js';
import { ContributionRegistry } from '/tmp/velvet-plugin-tests/workbench/contributions/contribution-registry.js';
import { OverlayRegistry } from '/tmp/velvet-plugin-tests/workbench/overlays/overlay-registry.js';
import { PluginClient } from '/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js';
import { PluginUiRegistry } from '/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js';
import { ViewRegistry } from '/tmp/velvet-plugin-tests/workbench/views/view-registry.js';
import { WorkbenchAppRegistry } from '/tmp/velvet-plugin-tests/workbench/apps/workbench-app.js';

const contextKeys = new ContextKeyService();
const commands = new CommandRegistry(contextKeys);
const menus = new MenuRegistry(commands, contextKeys);
const overlays = new OverlayRegistry(contextKeys);
const ui = new PluginUiRegistry();
const apps = new WorkbenchAppRegistry();
const client = new PluginClient(
  { request: async () => ({ kind: 'accepted' }), subscribe: () => () => {} },
  new ContributionRegistry(),
  apps,
  '1',
  ui,
  new ViewRegistry(),
  commands,
  menus,
  overlays,
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
    activityBarOrder: 20,
    surface: {
      entrypoint: 'wasm://sample/research-app',
      bridgeVersion: '1',
    },
  },
  contributions: [
    {
      id: 'open-tool',
      scope: 'workbench-app',
      appId: 'sample.app-extension.research',
      kind: 'command',
      title: 'Open tool',
    },
  ],
};

assert.throws(
  () => client.register({
    ...appManifest,
    id: 'sample.foreign-app',
    workbenchApp: { ...appManifest.workbenchApp, id: 'other.extension.research' },
  }),
  /must be owned by extension/,
);

client.register(appManifest);
client.activate(appManifest.id);
assert.equal(apps.get(appManifest.workbenchApp.id)?.surface.trust, 'isolated');
assert.deepEqual(
  apps.listActivityBarEntries().map((entry) => entry.appId),
  [appManifest.workbenchApp.id],
);
assert.equal(client.listAppSurfaceBindings().length, 1);
assert.equal(
  client.listAppSurfaceBindings()[0].surface.namespace,
  `workbench.app.${appManifest.workbenchApp.id}`,
);

let appMounted = false;
let appUnmounted = false;
const appSurfaceId = client.listAppSurfaceBindings()[0].surfaceId;
ui.bindAdapter(appManifest.id, appSurfaceId, {
  mount: () => {
    appMounted = true;
  },
  unmount: () => {
    appUnmounted = true;
  },
});
const appActivatedResult = await apps.activateSurface(appManifest.workbenchApp.id, {
  appId: appManifest.workbenchApp.id,
  context: {
    snapshot: () => ({
      revision: 0,
      activeAppId: appManifest.workbenchApp.id,
      dataScope: null,
      selection: [],
      query: '',
      filters: {},
      sort: [],
      handoff: null,
    }),
    subscribe: () => () => {},
  },
  dispatch: () => {},
});
assert.equal(appActivatedResult, true);
assert.equal(appMounted, true);
assert.equal(apps.getSurfaceState(appManifest.workbenchApp.id), 'active');
assert.equal(
  commands.get(`workbench.app.${appManifest.workbenchApp.id}`, 'open-tool')?.appId,
  appManifest.workbenchApp.id,
);
await apps.deactivateSurface(appManifest.workbenchApp.id);
assert.equal(apps.getSurfaceState(appManifest.workbenchApp.id), 'suspended');
await apps.activateSurface(appManifest.workbenchApp.id, {
  appId: appManifest.workbenchApp.id,
  context: {
    snapshot: () => ({
      revision: 0,
      activeAppId: appManifest.workbenchApp.id,
      dataScope: null,
      selection: [],
      query: '',
      filters: {},
      sort: [],
      handoff: null,
    }),
    subscribe: () => () => {},
  },
  dispatch: () => {},
});
assert.equal(apps.getSurfaceState(appManifest.workbenchApp.id), 'active');

await client.deactivate(appManifest.id);
assert.equal(appUnmounted, true);
assert.equal(apps.get(appManifest.workbenchApp.id), undefined);
assert.equal(apps.listActivityBarEntries().length, 0);
assert.equal(client.listAppSurfaceBindings().length, 0);
assert.equal(ui.getDescriptor(appManifest.id, appSurfaceId), undefined);

const contributionAppManifest = {
  id: 'sample.surface-extension',
  version: '1.0.0',
  hostApi: '1',
  permissions: {},
  contributions: [
    {
      id: 'reader-surface',
      scope: 'workbench-app',
      appId: 'sample.surface-extension.reader',
      kind: 'app-surface',
      container: 'workspace-area',
      title: 'Reader',
      icon: 'book',
      ui: { kind: 'app-surface', entrypoint: 'wasm://sample/reader', bridgeVersion: '1' },
    },
  ],
};
client.register(contributionAppManifest);
client.activate(contributionAppManifest.id);
assert.equal(apps.get('sample.surface-extension.reader')?.title, 'Reader');
assert.equal(client.listAppSurfaceBindings()[0].surfaceId, 'reader-surface');
await client.deactivate(contributionAppManifest.id);
assert.equal(apps.get('sample.surface-extension.reader'), undefined);

const manifest = {
  id: 'sample.extension',
  version: '1.0.0',
  hostApi: '1',
  permissions: {},
  contributions: [
    {
      id: 'open-dialog',
      scope: 'workbench',
      kind: 'command',
      title: 'Open dialog',
    },
    {
      id: 'asset-menu',
      scope: 'workbench',
      kind: 'context-menu',
      container: 'context-menu',
      anchor: 'asset',
      commandId: 'open-dialog',
      label: 'Open extension dialog',
    },
    {
      id: 'dialog',
      scope: 'workbench',
      kind: 'dialog',
      container: 'overlay-layer',
      title: 'Extension dialog',
      ui: { kind: 'dialog', entrypoint: 'wasm://sample/dialog', bridgeVersion: '1' },
    },
  ],
};

assert.throws(
  () => client.register({
    ...manifest,
    id: 'sample.invalid',
    contributions: [{
      ...manifest.contributions[1],
      namespace: 'workbench.app.asset-manager',
    }],
  }),
  /invalid namespace/,
);

client.register(manifest);
client.activate(manifest.id);

let executedCommand;
const releaseCommandHandler = client.bindCommandHandler(
  manifest.id,
  (commandId) => {
    executedCommand = commandId;
  },
);
await commands.execute('open-dialog', undefined, {
  activeAppId: undefined,
  keys: {},
});
assert.equal(executedCommand, 'open-dialog');

assert.equal(menus.list('context-menu', { anchor: 'asset' }).length, 1);
const instance = overlays.open('dialog');
assert.equal(instance.namespace, 'workbench');
assert.equal(overlays.active().length, 1);
assert.ok(ui.getDescriptor(manifest.id, 'dialog'));

let unmounted = false;
ui.bindAdapter(manifest.id, 'dialog', {
  mount: () => undefined,
  unmount: () => {
    unmounted = true;
  },
});
const session = ui.createSession(manifest.id, 'dialog');
await session.mount({
  context: {
    revision: 0,
    activeAppId: 'asset-manager',
    dataScope: null,
    selection: [],
    query: '',
    filters: {},
    sort: [],
    handoff: null,
  },
  theme: { scheme: 'dark', revision: 0, tokens: {} },
  size: { width: 400, height: 240 },
});
session.activate();

await client.deactivate(manifest.id);
releaseCommandHandler();
assert.equal(menus.list('context-menu', { anchor: 'asset' }).length, 0);
assert.equal(overlays.active().length, 0);
assert.equal(ui.getDescriptor(manifest.id, 'dialog'), undefined);
assert.equal(unmounted, true);

console.log('Plugin menu, overlay namespace and lifecycle tests passed.');
