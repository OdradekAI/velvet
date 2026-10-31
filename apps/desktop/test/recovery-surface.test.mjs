import assert from 'node:assert/strict';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of [
  'Document',
  'Element',
  'Event',
  'HTMLElement',
  'KeyboardEvent',
  'MouseEvent',
  'Node',
]) {
  if (browser[name]) {
    globalThis[name] = browser[name];
  }
}
globalThis.window = browser;
globalThis.document = browser.document;
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: browser.navigator,
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { RecoveryBoundary, RecoverySurface } = await import(
  '/tmp/velvet-plugin-tests/app/renderer/recovery-surface.js',
);

const projection = {
  schemaVersion: 1,
  mode: 'safe',
  phase: 'preflight',
  attemptId: 2,
  consecutiveFailures: 2,
  lastFailure: {
    class: 'core-migration',
    phase: 'preflight',
    message: 'layout migration failed',
    atMs: 42,
  },
  lastGood: null,
  actions: ['retry', 'restore-snapshot', 'export-diagnostics'],
};

const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);
let retries = 0;
let exports = 0;

await act(() => {
  root.render(React.createElement(RecoverySurface, {
    onExportDiagnostics: () => { exports += 1; },
    onRetry: () => { retries += 1; },
    projection,
  }));
});

assert.equal(rootElement.querySelector('[data-recovery-mode="safe"]') !== null, true);
assert.equal(rootElement.querySelector('#velvet-recovery-title').textContent, 'Velvet could not mount the workbench');
assert.equal(rootElement.querySelector('[data-recovery-action="retry"]').disabled, false);
assert.equal(rootElement.querySelector('[data-recovery-action="restore-snapshot"]').disabled, true);
assert.equal(rootElement.querySelector('[data-recovery-action="export-diagnostics"]').disabled, false);
assert.match(rootElement.textContent, /layout migration failed/);

await act(() => {
  rootElement.querySelector('[data-recovery-action="retry"]').click();
});
assert.equal(retries, 1);
await act(() => {
  rootElement.querySelector('[data-recovery-action="export-diagnostics"]').click();
});
assert.equal(exports, 1);

function BrokenWorkbench() {
  throw new Error('renderer mount failed');
}

const previousConsoleError = console.error;
console.error = () => undefined;
await act(() => {
  root.render(React.createElement(
    RecoveryBoundary,
    { onRetry: () => { retries += 1; }, projection },
    React.createElement(BrokenWorkbench),
  ));
});
console.error = previousConsoleError;

assert.ok(rootElement.querySelector('.velvet-recovery-surface'));
assert.match(rootElement.textContent, /renderer mount failed/);
await act(() => root.unmount());

console.log('Recovery surface tests passed.');
