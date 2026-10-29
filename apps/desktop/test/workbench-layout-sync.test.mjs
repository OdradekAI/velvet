import assert from 'node:assert/strict';
import { defaultLayoutState } from '/tmp/velvet-plugin-tests/workbench/layout/layout-state.js';
import { Workbench } from '/tmp/velvet-plugin-tests/workbench/workbench.js';

const initial = defaultLayoutState();
initial.containers['asset-manager.navigation'] = { visible: false, order: 7 };
initial.views['asset-manager.asset-library'] = {
  visible: false,
  order: 4,
  containerId: 'asset-manager.navigation',
};

let snapshot = initial;
const commands = [];
const listeners = new Set();
const core = {
  subscribe(listener) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
  async request(request) {
    if (request.kind === 'query-layout') {
      return { kind: 'layout', state: snapshot };
    }

    const command = request.command;
    commands.push(command);
    if (command.id === 'workbench.upsert-view-container') {
      snapshot = {
        ...snapshot,
        revision: snapshot.revision + 1,
        containers: {
          ...snapshot.containers,
          [command.containerId]: snapshot.containers[command.containerId] ?? {
            visible: command.visible,
            order: command.order,
          },
        },
      };
    }
    if (command.id === 'workbench.upsert-view') {
      snapshot = {
        ...snapshot,
        revision: snapshot.revision + 1,
        views: {
          ...snapshot.views,
          [command.viewId]: snapshot.views[command.viewId] ?? {
            visible: command.visible,
            order: command.order,
            containerId: command.containerId,
          },
        },
      };
    }
    for (const listener of listeners) {
      listener({ kind: 'layout-changed', revision: snapshot.revision, state: snapshot });
    }
    return { kind: 'layout', state: snapshot };
  },
};

const workbench = new Workbench(core);
await workbench.layout.load();
for (let index = 0; index < 4; index += 1) {
  await new Promise((resolve) => setImmediate(resolve));
}

assert.equal(snapshot.containers['asset-manager.navigation'].visible, false);
assert.equal(snapshot.containers['asset-manager.navigation'].order, 7);
assert.equal(snapshot.views['asset-manager.asset-library'].visible, false);
assert.equal(snapshot.views['asset-manager.asset-library'].order, 4);
assert.ok(snapshot.containers['feed-workbench.navigation']);
assert.ok(snapshot.views['feed-workbench.feed-sources']);
assert.equal(
  commands.some((command) => command.id === 'workbench.upsert-view-container' && command.containerId === 'asset-manager.navigation'),
  false,
);
assert.equal(
  commands.some((command) => command.id === 'workbench.upsert-view' && command.viewId === 'asset-manager.asset-library'),
  false,
);

workbench.plugins.register({
  id: 'sample.layout-extension',
  version: '1.0.0',
  hostApi: '1',
  targetApp: { appId: 'asset-manager', apiRange: '1' },
  permissions: {},
  contributions: [
    {
      id: 'research-tools',
      scope: 'workbench-app',
      appId: 'asset-manager',
      kind: 'view-container',
      container: 'primary-sidebar',
      zone: 'navigation',
      title: 'Research Tools',
      icon: 'search',
    },
    {
      id: 'research-list',
      scope: 'workbench-app',
      appId: 'asset-manager',
      kind: 'view',
      container: 'primary-sidebar',
      zone: 'navigation',
      viewContainerId: 'research-tools',
      title: 'Research List',
      icon: 'list',
      ui: {
        kind: 'view',
        entrypoint: 'wasm://sample/research-list',
        bridgeVersion: '1',
      },
    },
  ],
});
workbench.plugins.activate('sample.layout-extension');
for (let index = 0; index < 4; index += 1) {
  await new Promise((resolve) => setImmediate(resolve));
}

assert.ok(snapshot.containers['sample.layout-extension.research-tools']);
assert.ok(snapshot.views['sample.layout-extension.research-list']);
assert.deepEqual(
  commands
    .filter((command) => command.id === 'workbench.upsert-view')
    .at(-1),
  {
    id: 'workbench.upsert-view',
    viewId: 'sample.layout-extension.research-list',
    containerId: 'sample.layout-extension.research-tools',
    visible: true,
    order: 0,
  },
);

console.log('Workbench layout contribution sync tests passed.');
