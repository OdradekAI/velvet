import assert from 'node:assert/strict';

const { createNativePluginPackagePicker } = await import(
  '/tmp/velvet-plugin-tests/platform/tauri-plugin-package-picker.js',
);

const calls = [];
const picker = createNativePluginPackagePicker(async (options) => {
  calls.push(options);
  return options.directory ? '/tmp/native-plugin' : '/tmp/native-plugin.zip';
});

assert.equal(await picker.pick('directory'), '/tmp/native-plugin');
assert.deepEqual(calls[0], {
  directory: true,
  multiple: false,
  title: 'Select a Velvet plugin directory package',
});

assert.equal(await picker.pick('archive'), '/tmp/native-plugin.zip');
assert.deepEqual(calls[1], {
  directory: false,
  multiple: false,
  filters: [{ name: 'Velvet plugin package', extensions: ['zip'] }],
  title: 'Select a Velvet plugin ZIP package',
});

const cancelled = createNativePluginPackagePicker(async () => null);
assert.equal(await cancelled.pick('archive'), undefined);

const unexpectedMultipleSelection = createNativePluginPackagePicker(async () => [
  '/tmp/first.zip',
]);
assert.equal(await unexpectedMultipleSelection.pick('archive'), undefined);

const failed = createNativePluginPackagePicker(async () => {
  throw new Error('dialog unavailable');
});
await assert.rejects(() => failed.pick('directory'), /dialog unavailable/);

console.log('Plugin package picker tests passed.');
