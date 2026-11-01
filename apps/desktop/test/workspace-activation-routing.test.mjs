import assert from 'node:assert/strict';
import {
  routeActivationRequest,
  workspaceRoutingIntentKey,
} from '/tmp/velvet-plugin-tests/workbench/activation-routing.js';
import { Workbench } from '/tmp/velvet-plugin-tests/workbench/workbench.js';

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
const workbench = new Workbench({
  subscribe() {
    return () => undefined;
  },
  subscribeStatus(listener) {
    statusListeners.add(listener);
    listener({ state: 'connected' });
    return () => statusListeners.delete(listener);
  },
  async request() {
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
workbench.acceptActivationRequest({ ...request, requestId: 'request-2' });
assert.equal(routed.length, 2);
assert.equal(workbench.workspaceRoutingIntents().length, 2);
assert.equal(workbench.layout.snapshot(), initialSnapshot);
workbench.dispose();

console.log('Workspace activation routing tests passed.');
