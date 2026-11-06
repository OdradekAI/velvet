import assert from 'node:assert/strict';
import { CommandRegistry } from '/tmp/velvet-plugin-tests/workbench/commands/command-registry.js';
import { ContextKeyService } from '/tmp/velvet-plugin-tests/workbench/commands/context-key-service.js';
import { KeybindingRegistry } from '/tmp/velvet-plugin-tests/workbench/commands/keybinding-registry.js';
import { WorkbenchShortcutAdapter } from '/tmp/velvet-plugin-tests/workbench/commands/shortcut-adapter.js';

const contextKeys = new ContextKeyService();
const commands = new CommandRegistry(contextKeys);
const keybindings = new KeybindingRegistry(commands, contextKeys);
const executed = [];
const errors = [];

commands.register({
  id: 'open',
  title: 'Open',
  scope: 'workbench',
  namespace: 'workbench',
  execute: (_args, context) => executed.push(context),
});
keybindings.register({
  id: 'open',
  commandId: 'open',
  keybinding: 'CTRL+O',
  scope: 'workbench',
  namespace: 'workbench',
});

let listener;
let starts = 0;
let disposes = 0;
const source = {
  async start(next) {
    starts += 1;
    listener = next;
  },
  async dispose() {
    disposes += 1;
    listener = undefined;
  },
};
const context = { activeAppId: 'asset-manager', keys: contextKeys.snapshot() };
const adapter = new WorkbenchShortcutAdapter(keybindings, source, {
  context: () => context,
  onError: (error) => errors.push(error),
});

await adapter.start();
await adapter.start();
assert.equal(starts, 1);
listener({ keybinding: 'CTRL+O', state: 'released' });
listener({ keybinding: 'CTRL+O', state: 'pressed' });
await new Promise((resolve) => setImmediate(resolve));
assert.equal(executed.length, 1);
assert.equal(executed[0].activeAppId, 'asset-manager');

listener({ keybinding: 'CTRL+UNKNOWN', state: 'pressed' });
await new Promise((resolve) => setImmediate(resolve));
assert.equal(errors.length, 1);
assert.match(errors[0].message, /Keybinding is not visible/);

const contextErrors = [];
let contextListener;
const contextSource = {
  async start(next) {
    contextListener = next;
  },
  async dispose() {
    contextListener = undefined;
  },
};
const contextFailureAdapter = new WorkbenchShortcutAdapter(keybindings, contextSource, {
  context: () => {
    throw new Error('context unavailable');
  },
  onError: (error) => contextErrors.push(error),
});
await contextFailureAdapter.start();
contextListener({ keybinding: 'CTRL+O', state: 'pressed' });
await new Promise((resolve) => setImmediate(resolve));
assert.equal(contextErrors.length, 1);
assert.match(contextErrors[0].message, /context unavailable/);
await contextFailureAdapter.dispose();

await adapter.dispose();
await adapter.dispose();
assert.equal(disposes, 1);
console.log('shortcut adapter tests passed');
