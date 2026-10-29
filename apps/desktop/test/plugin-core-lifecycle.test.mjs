import assert from 'node:assert/strict';
import { CommandRegistry } from '/tmp/velvet-plugin-tests/workbench/commands/command-registry.js';
import { ContextKeyService } from '/tmp/velvet-plugin-tests/workbench/commands/context-key-service.js';
import { MenuRegistry } from '/tmp/velvet-plugin-tests/workbench/commands/menu-registry.js';
import { OverlayRegistry } from '/tmp/velvet-plugin-tests/workbench/overlays/overlay-registry.js';
import { PluginClient } from '/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js';
import { PluginUiRegistry } from '/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js';
import { ViewRegistry } from '/tmp/velvet-plugin-tests/workbench/views/view-registry.js';
import { WorkbenchAppRegistry } from '/tmp/velvet-plugin-tests/workbench/apps/workbench-app.js';

const requests = [];
let runtimeState = 'registered';
let installed = false;
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
        {
          id: 'open-reader',
          kind: 'command',
          scope: { kind: 'workbench-app', appId: 'asset-manager' },
          namespace: 'workbench.app.asset-manager',
          container: null,
          zone: null,
          title: 'Open Reader',
          titleKey: null,
          icon: null,
          order: null,
          defaultVisible: null,
          viewContainerId: null,
          activityBar: null,
          label: null,
          labelKey: null,
          commandId: null,
          group: null,
          when: null,
          anchor: null,
          ui: null,
        },
        {
          id: 'reader-menu',
          kind: 'context-menu',
          scope: { kind: 'workbench-app', appId: 'asset-manager' },
          namespace: 'workbench.app.asset-manager',
          container: 'context-menu',
          zone: 'navigation',
          title: 'Open Reader',
          titleKey: null,
          icon: null,
          order: 1,
          defaultVisible: null,
          viewContainerId: null,
          activityBar: null,
          label: 'Open Reader',
          labelKey: null,
          commandId: 'open-reader',
          group: 'reader',
          when: null,
          anchor: 'asset',
          ui: null,
        },
        {
          id: 'reader-dialog',
          kind: 'dialog',
          scope: { kind: 'workbench-app', appId: 'asset-manager' },
          namespace: 'workbench.app.asset-manager',
          container: 'overlay-layer',
          zone: null,
          title: 'Reader dialog',
          titleKey: null,
          icon: null,
          order: null,
          defaultVisible: null,
          viewContainerId: null,
          activityBar: null,
          label: null,
          labelKey: null,
          commandId: null,
          group: null,
          when: null,
          anchor: null,
          ui: { kind: 'dialog', entrypoint: 'wasm://sample/dialog', bridgeVersion: '1' },
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
        extensions: [
          projection(runtimeState).summary,
          ...(installed ? [
            {
              id: 'sample.installed',
              version: '0.1.0',
              active: runtimeState === 'active',
              state: runtimeState,
            },
          ] : []),
        ],
      };
    }
    if (request.kind === 'query-plugin-projection') {
      return { kind: 'plugin-projection', projection: projection(runtimeState, request.extensionId) };
    }
    if (request.kind === 'install-plugin') {
      installed = true;
      runtimeState = 'registered';
      return {
        kind: 'extension',
        extension: {
          id: 'sample.installed',
          version: '0.1.0',
          active: false,
          state: 'registered',
        },
      };
    }
    if (request.kind === 'invoke-plugin-command') {
      return { kind: 'accepted' };
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
const contextKeys = new ContextKeyService();
const commands = new CommandRegistry(contextKeys);
const menus = new MenuRegistry(commands, contextKeys);
const overlays = new OverlayRegistry(contextKeys);
const apps = new WorkbenchAppRegistry([{
  id: 'asset-manager',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  apiVersion: '1',
  surface: { trust: 'trusted', entrypoint: 'builtin://resource-manager' },
}]);
const client = new PluginClient(core, undefined, apps, '1', ui, views, commands, menus, overlays);
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
assert.ok(commands.get('workbench.app.asset-manager', 'open-reader'));
await commands.execute(
  'open-reader',
  { source: 'test' },
  { activeAppId: 'asset-manager', keys: { resourceKind: 'asset' } },
);
assert.equal(requests[4].kind, 'invoke-plugin-command');
assert.equal(requests[4].commandId, 'open-reader');
assert.deepEqual(requests[4].context, {
  activeAppId: 'asset-manager',
  keys: { resourceKind: 'asset' },
});
assert.equal(
  menus.list('context-menu', { activeAppId: 'asset-manager', anchor: 'asset' }).length,
  1,
);
assert.ok(overlays.get('workbench.app.asset-manager', 'reader-dialog'));
assert.equal(ui.getDescriptor('sample.reader', 'reader-dialog')?.namespace, 'workbench.app.asset-manager');
assert.equal((await client.checkCoreHealth('sample.reader')).state, 'active');
assert.equal((await client.disableInCore('sample.reader')).state, 'disabled');
assert.equal(views.get('sample.reader.reader-view'), undefined);
assert.equal(ui.getDescriptor('sample.reader', 'reader-view'), undefined);
assert.equal(commands.get('workbench.app.asset-manager', 'open-reader'), undefined);
assert.equal(
  menus.list('context-menu', { activeAppId: 'asset-manager', anchor: 'asset' }).length,
  0,
);
assert.equal(overlays.get('workbench.app.asset-manager', 'reader-dialog'), undefined);
assert.equal(ui.getDescriptor('sample.reader', 'reader-dialog'), undefined);
assert.equal((await client.recoverInCore('sample.reader')).state, 'active');
assert.ok(views.get('sample.reader.reader-view'));
assert.ok(commands.get('workbench.app.asset-manager', 'open-reader'));
assert.equal((await client.queryCoreExtensions())[0].id, 'sample.reader');
assert.equal(client.getCoreSummary('sample.reader')?.state, 'active');

const installedSummary = await client.installInCore('/tmp/sample-plugin');
assert.equal(installedSummary.id, 'sample.installed');
assert.equal(installedSummary.state, 'active');
assert.deepEqual(
  requests.slice(-4).map((request) => request.kind),
  ['install-plugin', 'query-plugin-projection', 'activate-plugin', 'query-plugin-projection'],
);
assert.ok((await client.queryCoreExtensions()).some((summary) => summary.id === 'sample.installed'));

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
    'invoke-plugin-command',
    'check-plugin-health',
    'query-plugin-projection',
    'disable-plugin',
    'recover-plugin',
    'query-plugin-projection',
    'query-extensions',
    'install-plugin',
    'query-plugin-projection',
    'activate-plugin',
    'query-plugin-projection',
    'query-extensions',
  ],
);
assert.equal(requests[0].manifest.runtime, 'built-in');
assert.equal(requests[7].reason, 'user');

const failedInstallRequests = [];
let failedInstallState = 'registered';
const duplicateCommand = projection('active').contributions.find(
  (contribution) => contribution.kind === 'command',
);
const failedInstallProjection = {
  summary: {
    id: 'sample.failed-install',
    version: '0.1.0',
    active: failedInstallState === 'active',
    state: failedInstallState,
  },
  targetApp: { appId: 'asset-manager', api: '1' },
  contributions: [
    { ...duplicateCommand, id: 'duplicate-command' },
    { ...duplicateCommand, id: 'duplicate-command' },
  ],
};
const failedInstallCore = {
  request: async (request) => {
    failedInstallRequests.push(request);
    if (request.kind === 'install-plugin') {
      failedInstallState = 'registered';
      return {
        kind: 'extension',
        extension: {
          id: 'sample.failed-install',
          version: '0.1.0',
          active: false,
          state: 'registered',
        },
      };
    }
    if (request.kind === 'query-plugin-projection') {
      return {
        kind: 'plugin-projection',
        projection: {
          ...failedInstallProjection,
          summary: {
            ...failedInstallProjection.summary,
            active: failedInstallState === 'active',
            state: failedInstallState,
          },
        },
      };
    }
    if (request.kind === 'activate-plugin') {
      failedInstallState = 'active';
      return {
        kind: 'extension',
        extension: {
          id: 'sample.failed-install',
          version: '0.1.0',
          active: true,
          state: 'active',
        },
      };
    }
    if (request.kind === 'disable-plugin') {
      failedInstallState = 'disabled';
      return {
        kind: 'extension',
        extension: {
          id: 'sample.failed-install',
          version: '0.1.0',
          active: false,
          state: 'disabled',
        },
      };
    }
    throw new Error(`Unexpected failed-install request: ${request.kind}`);
  },
  subscribe: () => () => {},
};
const failedInstallCommands = new CommandRegistry(new ContextKeyService());
const failedInstallClient = new PluginClient(
  failedInstallCore,
  undefined,
  new WorkbenchAppRegistry([{
    id: 'asset-manager',
    title: 'Resource Manager',
    activityBarIcon: 'library',
    apiVersion: '1',
    surface: { trust: 'trusted', entrypoint: 'builtin://resource-manager' },
  }]),
  '1',
  new PluginUiRegistry(),
  new ViewRegistry(),
  failedInstallCommands,
  new MenuRegistry(failedInstallCommands, new ContextKeyService()),
  new OverlayRegistry(new ContextKeyService()),
);
await assert.rejects(
  () => failedInstallClient.installInCore('/tmp/failing-plugin'),
  /Extension activation failed: sample\.failed-install/,
);
assert.equal(failedInstallState, 'disabled');
assert.equal(failedInstallClient.getCoreSummary('sample.failed-install')?.state, 'disabled');
assert.equal(
  failedInstallCommands.get('workbench.app.asset-manager', 'duplicate-command'),
  undefined,
);
assert.deepEqual(
  failedInstallRequests.map((request) => request.kind),
  [
    'install-plugin',
    'query-plugin-projection',
    'activate-plugin',
    'query-plugin-projection',
    'disable-plugin',
  ],
);

console.log('Plugin core lifecycle bridge tests passed.');
