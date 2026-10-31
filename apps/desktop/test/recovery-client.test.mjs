import assert from 'node:assert/strict';
import { RecoveryClient } from '/tmp/velvet-plugin-tests/workbench/recovery/recovery-client.js';

const projection = {
  schemaVersion: 1,
  mode: 'safe',
  phase: 'preflight',
  attemptId: 2,
  consecutiveFailures: 2,
  lastFailure: null,
  lastGood: null,
  actions: ['retry', 'restore-snapshot'],
};
const listeners = new Set();
const core = {
  request: async (request) => {
    if (request.kind === 'query-recovery') {
      assert.deepEqual(request, { protocolVersion: 1, kind: 'query-recovery' });
      return { kind: 'recovery', projection };
    }
    assert.deepEqual(request, {
      protocolVersion: 1,
      kind: 'export-recovery-diagnostics',
      destination: '/tmp/velvet-recovery.json',
    });
    return { kind: 'accepted' };
  },
  subscribe: (listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};

const client = new RecoveryClient(core);
const received = [];
client.subscribe((value) => received.push(value));
assert.deepEqual(await client.load(), projection);
assert.deepEqual(client.snapshot(), projection);
await client.exportDiagnostics('/tmp/velvet-recovery.json');

for (const listener of listeners) {
  listener({ kind: 'recovery-changed', projection: { ...projection, mode: 'degraded' } });
}
assert.equal(client.snapshot().mode, 'degraded');
assert.equal(received.length, 2);
client.dispose();
assert.equal(listeners.size, 0);
console.log('Recovery client tests passed.');
