import assert from 'node:assert/strict';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of [
  'Document',
  'Element',
  'Event',
  'HTMLElement',
  'HTMLInputElement',
  'InputEvent',
  'KeyboardEvent',
  'MouseEvent',
  'MutationObserver',
  'Node',
  'NodeFilter',
  'PointerEvent',
]) {
  if (browser[name]) {
    globalThis[name] = browser[name];
  }
}
globalThis.Event = browser.Event;
globalThis.CustomEvent = browser.CustomEvent;
globalThis.window = browser;
globalThis.document = browser.document;
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: browser.navigator,
});
globalThis.getComputedStyle = browser.getComputedStyle.bind(browser);
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { SourceRootDialog } = await import(
  '/tmp/velvet-plugin-tests/app/renderer/source-root-dialog.js',
);
const { SourceRootService } = await import(
  '/tmp/velvet-plugin-tests/workbench/services/source-root-service.js',
);
const { createNativeSourceRootPicker } = await import(
  '/tmp/velvet-plugin-tests/platform/tauri-source-root-picker.js',
);

let roots = ['/tmp/assets'];
let pickedRoot = '/tmp/chosen';
let failNextAdd = false;
const requests = [];
const service = new SourceRootService({
  async request(request) {
    requests.push(request);
    if (request.kind === 'query-approved-source-roots') {
      return { kind: 'approved-source-roots', roots };
    }
    if (request.kind === 'add-approved-source-root') {
      if (failNextAdd) {
        failNextAdd = false;
        throw new Error('source policy is temporarily unavailable');
      }
      roots = [...roots, request.root];
      return { kind: 'accepted' };
    }
    if (request.kind === 'remove-approved-source-root') {
      roots = roots.filter((root) => root !== request.root);
      return { kind: 'accepted' };
    }
    throw new Error(`unexpected request ${request.kind}`);
  },
});

const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);
await act(async () => {
  root.render(React.createElement(SourceRootDialog, {
    confirmRevoke: async () => true,
    onOpenChange: () => undefined,
    open: true,
    pickDirectory: async () => pickedRoot,
    sourceRoots: service,
  }));
  await new Promise((resolve) => setTimeout(resolve, 10));
});

assert.equal(document.body.querySelectorAll('[data-source-root]').length, 1);
await act(async () => {
  document.body.querySelector('.velvet-source-roots__input-row button').click();
  await new Promise((resolve) => setTimeout(resolve, 0));
});
await act(async () => {
  document.body.querySelector('.velvet-source-roots__form button[type="submit"]').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.equal(document.body.querySelectorAll('[data-source-root]').length, 2);
assert.match(document.body.textContent ?? '', /Approved source folder/);

await act(async () => {
  document.body.querySelector('[data-source-root="/tmp/assets"] button').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.equal(document.body.querySelectorAll('[data-source-root]').length, 1);
assert.equal(document.body.querySelector('[data-source-root="/tmp/assets"]'), null);
assert.deepEqual(
  requests.filter((request) => request.kind !== 'query-approved-source-roots').map((request) => request.kind),
  ['add-approved-source-root', 'remove-approved-source-root'],
);

pickedRoot = '/tmp/failing';
failNextAdd = true;
await act(async () => {
  document.body.querySelector('.velvet-source-roots__input-row button').click();
  await new Promise((resolve) => setTimeout(resolve, 0));
});
await act(async () => {
  document.body.querySelector('.velvet-source-roots__form button[type="submit"]').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.match(document.body.querySelector('[role="alert"]')?.textContent ?? '', /temporarily unavailable/);
assert.equal(document.body.querySelectorAll('[data-source-root]').length, 1);

let pickerOptions;
const picker = createNativeSourceRootPicker(async (options) => {
  pickerOptions = options;
  return '/tmp/picked';
});
assert.equal(await picker.pick(), '/tmp/picked');
assert.deepEqual(pickerOptions, {
  directory: true,
  multiple: false,
  title: 'Choose an approved Velvet source folder',
});

await act(() => root.unmount());
console.log('Source-root dialog tests passed.');
