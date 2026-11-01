import assert from 'node:assert/strict';
import {
  routeActivationRequest,
  workspaceRoutingIntentKey,
} from '/tmp/velvet-plugin-tests/workbench/activation-routing.js';
import { Workbench } from '/tmp/velvet-plugin-tests/workbench/workbench.js';
import { defaultLayoutState } from '/tmp/velvet-plugin-tests/workbench/layout/layout-state.js';

const intents = routeActivationRequest({
  schemaVersion: 1,
  requestId: 'activation-1',
  targets: [
    { kind: 'file-path', value: '/tmp/asset.png' },
    { kind: 'deep-link', value: 'velvet://asset/asset%2Fone' },
    { kind: 'deep-link', value: 'velvet://unknown/item' },
  ],
});

assert.deepEqual(intents, [
  {
    kind: 'open-file',
    workspaceId: 'asset-manager',
    requestId: 'activation-1',
    targetIndex: 0,
    path: '/tmp/asset.png',
  },
  {
    kind: 'open-asset',
    workspaceId: 'asset-manager',
    requestId: 'activation-1',
    targetIndex: 1,
    assetId: 'asset/one',
  },
  {
    kind: 'unrouted-deep-link',
    requestId: 'activation-1',
    targetIndex: 2,
    value: 'velvet://unknown/item',
    reason: 'unsupported-route',
  },
]);

assert.equal(
  workspaceRoutingIntentKey(intents[0]),
  'activation-1:0:open-file:asset-manager:/tmp/asset.png',
);
assert.equal(
  workspaceRoutingIntentKey(intents[1]),
  'activation-1:1:open-asset:asset-manager:asset/one',
);

assert.equal(
  routeActivationRequest({
    schemaVersion: 1,
    requestId: 'invalid',
    targets: [{ kind: 'deep-link', value: 'velvet://asset/' }],
  })[0].kind,
  'unrouted-deep-link',
);

const statusListeners = new Set();
const registerCalls = [];
let layoutState = defaultLayoutState();
const workbench = new Workbench({
  subscribe() {
    return () => undefined;
  },
  subscribeStatus(listener) {
    statusListeners.add(listener);
    listener({ state: 'connected' });
    return () => statusListeners.delete(listener);
  },
  async request(request) {
    if (request.kind === 'layout-command') {
      if (request.command.id === 'workbench.upsert-view-container') {
        layoutState = {
          ...layoutState,
          containers: {
            ...layoutState.containers,
            [request.command.containerId]: {
              visible: request.command.visible,
              order: request.command.order,
            },
          },
        };
      } else if (request.command.id === 'workbench.upsert-view') {
        layoutState = {
          ...layoutState,
          views: {
            ...layoutState.views,
            [request.command.viewId]: {
              visible: request.command.visible,
              order: request.command.order,
              containerId: request.command.containerId,
            },
          },
        };
      }
      layoutState = {
        ...layoutState,
        revision: layoutState.revision + 1,
        activeApp:
          request.command.id === 'workbench.activate-app'
            ? request.command.appId
            : layoutState.activeApp,
      };
      return { kind: 'layout', state: layoutState };
    }
    if (request.kind === 'register-preview-asset') {
      registerCalls.push(request);
    }
    return { kind: 'accepted' };
  },
});
const routed = [];
workbench.subscribeWorkspaceRouting(() => {
  throw new Error('one routing consumer failed');
});
workbench.subscribeWorkspaceRouting((intent) => routed.push(intent));
const request = {
  schemaVersion: 1,
  requestId: 'request-1',
  targets: [{ kind: 'file-path', value: '/tmp/asset.png' }],
};
const initialSnapshot = workbench.layout.snapshot();
workbench.acceptActivationRequest(request);
workbench.acceptActivationRequest(request);
workbench.acceptActivationRequest({
  schemaVersion: 1,
  requestId: 'request-2',
  targets: [{ kind: 'deep-link', value: 'velvet://asset/asset-1' }],
});
assert.equal(routed.length, 2);
assert.equal(workbench.workspaceRoutingIntents().length, 2);
assert.equal(workbench.layout.snapshot(), initialSnapshot);
workbench.layout.acceptSnapshot(layoutState);
const pendingIntent = workbench.workspaceRoutingIntents().find((intent) => intent.kind === 'open-asset');
assert.ok(pendingIntent);
const consumed = [];
assert.equal(
  await workbench.consumeWorkspaceRoutingIntent(pendingIntent, (intent) => {
    consumed.push(intent);
    if (intent.kind === 'open-asset') {
      workbench.context.apply({
        kind: 'set-selection',
        selection: [{ kind: 'asset', id: intent.assetId }],
      });
    }
  }),
  true,
);
assert.equal(consumed.length, 1);
assert.equal(workbench.context.snapshot().activeAppId, 'asset-manager');
assert.deepEqual(workbench.context.snapshot().selection, [
  { kind: 'asset', id: 'asset-1' },
]);
assert.equal(workbench.workspaceRoutingIntents().length, 1);
const fileIntent = workbench.workspaceRoutingIntents().find((intent) => intent.kind === 'open-file');
assert.ok(fileIntent);
assert.equal(
  await workbench.consumeWorkspaceRoutingIntent(fileIntent, async (intent) => {
    const assetId = await workbench.registerFileActivation(intent);
    workbench.context.apply({
      kind: 'set-selection',
      selection: [{ kind: 'asset', id: assetId }],
    });
  }),
  true,
);
assert.deepEqual(registerCalls, [{
  protocolVersion: 1,
  kind: 'register-preview-asset',
  assetId: 'activation-request-1-0',
  sourcePath: '/tmp/asset.png',
  contentKind: 'application/octet-stream',
}]);
assert.deepEqual(workbench.context.snapshot().selection, [
  { kind: 'asset', id: 'activation-request-1-0' },
]);
assert.equal(workbench.workspaceRoutingIntents().length, 0);
workbench.acceptActivationRequest({
  schemaVersion: 1,
  requestId: 'request-3',
  targets: [{ kind: 'file-path', value: '/tmp/failure.png' }],
});
const failedIntent = workbench.workspaceRoutingIntents()[0];
assert.equal(
  await workbench.consumeWorkspaceRoutingIntent(failedIntent, () => {
    throw new Error('workspace failed');
  }),
  false,
);
assert.equal(workbench.workspaceRoutingIntents().length, 1);
workbench.dispose();

console.log('Workspace activation routing tests passed.');
