import assert from 'node:assert/strict';
import { PluginClient } from '/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js';
import { PluginUiRegistry } from '/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js';
import { ViewRegistry } from '/tmp/velvet-plugin-tests/workbench/views/view-registry.js';
import { WorkbenchAppRegistry } from '/tmp/velvet-plugin-tests/workbench/apps/workbench-app.js';

const requests = [];
let runtimeState = 'registered';
const projection = (state, id = 'sample.reader') => ({
  summary: {
    id,
    version: '1.0.0',
    active: state === 'active',
    state,
  },
  targetApp: { appId: 'asset-manager', api: '1' },
  contributions: id === 'sample.reader'
    ? [
        {
          id: 'library',
          kind: 'view-container',
          scope: { kind: 'workbench-app', appId: 'asset-manager' },
          namespace: 'workbench.app.asset-manager',
          container: 'primary-sidebar',
          zone: 'navigation',
          title: 'Reader',
          titleKey: null,
          icon: 'book',
          order: 2,
          defaultVisible: true,
          viewContainerId: null,
          activityBar: { title: 'Reader', icon: 'book', order: 2 },
          ui: null,
        },
        {
          id: 'reader-view',
          kind: 'view',
          scope: { kind: 'workbench-app', appId: 'asset-manager' },
          namespace: 'workbench.app.asset-manager',
          container: 'primary-sidebar',
          zone: 'navigation',
          title: 'Reader view',
          titleKey: null,
          icon: 'book-open',
          order: 1,
          defaultVisible: true,
          viewContainerId: 'library',
          activityBar: null,
          ui: { kind: 'view', entrypoint: 'wasm://sample/reader', bridgeVersion: '1' },
        },
      ]
    : [],
});

const core = {
  request: async (request) => {
    requests.push(request);
    if (request.kind === 'query-extensions') {
      return {
        kind: 'extensions',
        extensions: [projection(runtimeState).summary],
      };
    }
    if (request.kind === 'query-plugin-projection') {
      return { kind: 'plugin-projection', projection: projection(runtimeState, request.extensionId) };
    }

    const extensionId = request.kind === 'register-plugin'
      ? request.manifest.id
      : request.extensionId;
    if (request.kind === 'activate-plugin' || request.kind === 'recover-plugin') {
      runtimeState = 'active';
    } else if (request.kind === 'disable-plugin') {
      runtimeState = 'disabled';
    } else if (request.kind === 'check-plugin-health') {
      runtimeState = 'active';
    } else if (request.kind === 'register-plugin') {
      runtimeState = 'registered';
    }
    return {
      kind: 'extension',
      extension: {
        id: extensionId,
        version: '1.0.0',
        active: runtimeState === 'active',
        state: runtimeState,
      },
    };
  },
  subscribe: () => () => {},
};

const views = new ViewRegistry();
const ui = new PluginUiRegistry();
const apps = new WorkbenchAppRegistry([{
  id: 'asset-manager',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  apiVersion: '1',
  surface: { trust: 'trusted', entrypoint: 'builtin://resource-manager' },
}]);
const client = new PluginClient(core, undefined, apps, '1', ui, views);
const manifest = {
  id: 'sample.reader',
  version: '1.0.0',
  hostApi: '^1',
  runtime: 'built-in',
  launch: null,
  targetApp: null,
  contributions: [],
  permissionSets: [],
};

const registered = await client.registerInCore(manifest);
assert.equal(registered.state, 'registered');
assert.equal((await client.activateInCore('sample.reader')).state, 'active');
assert.ok(views.getContainer('sample.reader.library'));
assert.ok(views.get('sample.reader.reader-view'));
assert.equal(ui.getDescriptor('sample.reader', 'reader-view')?.namespace, 'workbench.app.asset-manager');
assert.equal((await client.checkCoreHealth('sample.reader')).state, 'active');
assert.equal((await client.disableInCore('sample.reader')).state, 'disabled');
assert.equal(views.get('sample.reader.reader-view'), undefined);
assert.equal(ui.getDescriptor('sample.reader', 'reader-view'), undefined);
assert.equal((await client.recoverInCore('sample.reader')).state, 'active');
assert.ok(views.get('sample.reader.reader-view'));
assert.equal((await client.queryCoreExtensions())[0].id, 'sample.reader');
assert.equal(client.getCoreSummary('sample.reader')?.state, 'active');

await assert.rejects(
  () => client.applyCoreProjection({
    ...projection('active', 'foreign.reader'),
    contributions: [{ ...projection('active').contributions[0], namespace: 'workbench.app.foreign' }],
  }),
  /invalid namespace/,
);

assert.deepEqual(
  requests.map((request) => request.kind),
  [
    'register-plugin',
    'query-plugin-projection',
    'activate-plugin',
    'query-plugin-projection',
    'check-plugin-health',
    'query-plugin-projection',
    'disable-plugin',
    'recover-plugin',
    'query-plugin-projection',
    'query-extensions',
  ],
);
assert.equal(requests[0].manifest.runtime, 'built-in');
assert.equal(requests[6].reason, 'user');

console.log('Plugin core lifecycle bridge tests passed.');
