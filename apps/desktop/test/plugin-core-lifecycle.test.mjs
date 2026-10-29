import assert from 'node:assert/strict';
import { PluginClient } from '/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js';

const requests = [];
const core = {
  request: async (request) => {
    requests.push(request);
    if (request.kind === 'query-extensions') {
      return {
        kind: 'extensions',
        extensions: [{ id: 'sample.reader', version: '1.0.0', active: false, state: 'registered' }],
      };
    }

    const extensionId = request.kind === 'register-plugin'
      ? request.manifest.id
      : request.extensionId;
    const state = request.kind === 'activate-plugin'
      ? 'active'
      : request.kind === 'disable-plugin'
        ? 'disabled'
        : 'registered';
    return {
      kind: 'extension',
      extension: {
        id: extensionId,
        version: '1.0.0',
        active: state === 'active',
        state,
      },
    };
  },
  subscribe: () => () => {},
};

const client = new PluginClient(core);
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
assert.equal((await client.checkCoreHealth('sample.reader')).state, 'registered');
assert.equal((await client.disableInCore('sample.reader')).state, 'disabled');
assert.equal((await client.recoverInCore('sample.reader')).state, 'registered');
assert.equal((await client.queryCoreExtensions())[0].id, 'sample.reader');
assert.equal(client.getCoreSummary('sample.reader')?.state, 'registered');

assert.deepEqual(
  requests.map((request) => request.kind),
  [
    'register-plugin',
    'activate-plugin',
    'check-plugin-health',
    'disable-plugin',
    'recover-plugin',
    'query-extensions',
  ],
);
assert.equal(requests[0].manifest.runtime, 'built-in');
assert.equal(requests[3].reason, 'user');

console.log('Plugin core lifecycle bridge tests passed.');
