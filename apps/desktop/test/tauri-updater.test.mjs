import assert from 'node:assert/strict';
import { TauriUpdateCoordinator } from '../src/platform/tauri-updater.ts';

const calls = [];
const bridge = {
  async invoke(command, args) {
    calls.push({ command, args });
    return {
      schemaVersion: 1,
      state: command === 'velvet_update_check' ? 'available' : 'restart-required',
      channel: 'preview',
      currentVersion: '0.0.6',
      version: '0.0.6',
      target: 'macos-aarch64',
      releaseSequence: 5,
      transactionId: command === 'velvet_update_install' ? 'updater-1-1' : null,
      notes: null,
      message: null,
    };
  },
};

const coordinator = new TauriUpdateCoordinator(bridge);
assert.equal((await coordinator.check()).state, 'available');
assert.equal((await coordinator.install('0.0.6')).state, 'restart-required');
assert.deepEqual(calls, [
  { command: 'velvet_update_check', args: undefined },
  { command: 'velvet_update_install', args: { request: { expectedVersion: '0.0.6' } } },
]);
