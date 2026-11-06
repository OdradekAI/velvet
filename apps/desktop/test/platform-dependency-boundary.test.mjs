import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const shortcutSource = await readFile(
  new URL('../src/platform/tauri-shortcut-adapter.ts', import.meta.url),
  'utf8',
);
assert.doesNotMatch(shortcutSource, /(?:\.\.\/)+workbench/);
assert.doesNotMatch(shortcutSource, /workbench\/commands/);
console.log('platform dependency boundary tests passed');
