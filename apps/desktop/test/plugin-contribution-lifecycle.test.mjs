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
const client = new PluginClient(
  { request: async () => ({ kind: 'accepted' }), subscribe: () => () => {} },
  new ContributionRegistry(),
  new WorkbenchAppRegistry(),
  '1',
  ui,
  new ViewRegistry(),
  commands,
  menus,
  overlays,
);

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
