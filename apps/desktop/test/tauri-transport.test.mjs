import assert from 'node:assert/strict';
import {
  CoreTransportError,
  TauriCoreTransport,
} from '/tmp/velvet-plugin-tests/platform/tauri-transport.js';

const calls = [];
let eventListener;
let resynced;
const bridge = {
  async invoke(command, args) {
    calls.push({ command, args });
    if (command === 'velvet_core_handshake') {
      return {
        protocolMajor: 1,
        protocolMinor: 0,
        selectedFeatures: ['session-v1'],
        schemaVersions: { profile: 1 },
        sessionId: `session-${calls.length}`,
      };
    }
    assert.equal(command, 'velvet_core_request');
    assert.match(args.envelope.sessionId, /^session-/);
    if (args.envelope.request.kind === 'query-snapshot') {
      return {
        protocolVersion: 1,
        requestId: args.envelope.requestId,
        response: {
          kind: 'snapshot',
          snapshot: { schemaVersion: 1, layout: {}, profile: {}, recovery: {}, jobs: [], extensions: [] },
        },
      };
    }
    if (args.envelope.request.kind === 'query-request-outcome') {
      return {
        protocolVersion: 1,
        requestId: args.envelope.requestId,
        response: {
          kind: 'request-outcome',
          outcome: { requestId: args.envelope.request.requestId, status: 'unknown' },
        },
      };
    }
    return {
      protocolVersion: 1,
      requestId: args.envelope.requestId,
      response: {
        kind: 'profile',
        state: { schemaVersion: 1, revision: 0, locale: 'en' },
      },
    };
  },
  async listen(_event, listener) {
    eventListener = listener;
    return () => undefined;
  },
};

const transport = new TauriCoreTransport(bridge, {
  onSequenceGap: (expected, received) => {
    assert.equal(expected, 2);
    assert.equal(received, 3);
  },
  onResync: (snapshot) => {
    resynced = snapshot;
  },
});
const response = await transport.request({ protocolVersion: 1, kind: 'query-profile' });
assert.equal(response.kind, 'profile');
assert.equal(calls.filter(({ command }) => command === 'velvet_core_handshake').length, 1);
assert.equal(calls.filter(({ command }) => command === 'velvet_core_request').length, 1);

const unsubscribe = transport.subscribe(() => undefined);
await new Promise((resolve) => setImmediate(resolve));
eventListener({ payload: { protocolVersion: 1, sequence: 1, event: { kind: 'profile-changed', revision: 0, state: {} } } });
eventListener({ payload: { protocolVersion: 1, sequence: 3, event: { kind: 'profile-changed', revision: 1, state: {} } } });
await new Promise((resolve) => setImmediate(resolve));
await new Promise((resolve) => setImmediate(resolve));
assert.equal(resynced.schemaVersion, 1);
assert.equal(calls.filter(({ command }) => command === 'velvet_core_handshake').length, 2);
assert.equal(calls.filter(({ command }) => command === 'velvet_core_request').length, 2);
unsubscribe();

const outcome = await transport.queryRequestOutcome(11);
assert.deepEqual(outcome, { requestId: 11, status: 'unknown' });

const firstSession = await transport.connect();
await transport.reconnect();
const secondSession = await transport.connect();
assert.notEqual(firstSession.sessionId, secondSession.sessionId);
assert.equal(calls.filter(({ command }) => command === 'velvet_core_handshake').length, 3);
const handshakeCalls = calls.filter(({ command }) => command === 'velvet_core_handshake');
assert.match(handshakeCalls[0].args.request.clientId, /^desktop-/);
assert.equal(handshakeCalls[0].args.request.clientId, handshakeCalls.at(-1).args.request.clientId);

const incompatible = new TauriCoreTransport({
  ...bridge,
  async invoke(command) {
    if (command === 'velvet_core_handshake') {
      return {
        protocolMajor: 2,
        protocolMinor: 0,
        selectedFeatures: [],
        schemaVersions: {},
        sessionId: 'invalid',
      };
    }
    throw new Error('request should not run after an incompatible handshake');
  },
});
await assert.rejects(
  incompatible.connect(),
  (error) => error instanceof CoreTransportError && error.details.code === 'protocol-major-mismatch',
);

console.log('Tauri transport handshake tests passed.');
