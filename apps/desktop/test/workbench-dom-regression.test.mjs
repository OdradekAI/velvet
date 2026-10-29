import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { symlinkSync } from 'node:fs';
import { Window } from 'happy-dom';

try {
  symlinkSync(
    fileURLToPath(new URL('../node_modules', import.meta.url)),
    '/tmp/velvet-plugin-tests/node_modules',
    'junction',
  );
} catch {
  // The compiled test tree may already point at the desktop dependencies.
}

const browser = new Window({ url: 'http://velvet.test/' });
const globals = [
  'Document',
  'DOMRect',
  'DOMRectReadOnly',
  'Element',
  'Event',
  'FocusEvent',
  'HTMLElement',
  'InputEvent',
  'KeyboardEvent',
  'MouseEvent',
  'MutationObserver',
  'Node',
  'PointerEvent',
  'CustomEvent',
  'ResizeObserver',
];
for (const name of globals) {
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
globalThis.getComputedStyle = browser.getComputedStyle.bind(browser);
globalThis.requestAnimationFrame = browser.requestAnimationFrame.bind(browser);
globalThis.cancelAnimationFrame = browser.cancelAnimationFrame.bind(browser);
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { WorkbenchShell } = await import('/tmp/velvet-plugin-tests/app/renderer/workbench-shell.js');
const { defaultLayoutState } = await import('/tmp/velvet-plugin-tests/workbench/layout/layout-state.js');
const { FocusService } = await import('/tmp/velvet-plugin-tests/workbench/focus/focus-service.js');
const { ViewRegistry } = await import('/tmp/velvet-plugin-tests/workbench/views/view-registry.js');

const layout = defaultLayoutState();
layout.containers['sample.global.tools'] = { visible: true, order: 0 };
layout.views['sample.global.first'] = { visible: true, order: 0, containerId: 'sample.global.tools' };
layout.views['sample.global.second'] = { visible: true, order: 1, containerId: 'sample.global.tools' };

const views = new ViewRegistry();
views.registerContainer({
  id: 'sample.global.tools',
  title: 'Global Tools',
  icon: 'tools',
  zone: 'contextual',
  preferredPart: 'auxiliary-sidebar',
  scope: { kind: 'workbench' },
  order: 0,
});
views.register({
  id: 'sample.global.first',
  containerId: 'sample.global.tools',
  title: 'First Tool',
  icon: 'one',
  zone: 'contextual',
  scope: { kind: 'workbench' },
  order: 0,
});
views.register({
  id: 'sample.global.second',
  containerId: 'sample.global.tools',
  title: 'Second Tool',
  icon: 'two',
  zone: 'contextual',
  scope: { kind: 'workbench' },
  order: 1,
});

const focus = new FocusService();
const commands = [];
const app = {
  id: 'asset-manager',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  surface: { trust: 'trusted', entrypoint: 'builtin://asset-manager' },
};
const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);

async function render(nextLayout) {
  await act(() => {
    root.render(React.createElement(WorkbenchShell, {
      apps: [app],
      focusService: focus,
      layout: nextLayout,
      onActivateApp: () => undefined,
      onLayoutCommand: (command) => commands.push(command),
      viewRegistry: views,
    }));
  });
}

function tick() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

await render(layout);
await tick();
await tick();

const container = rootElement.querySelector('#velvet-view-container-sample-global-tools');
assert.ok(container);
assert.equal(container.getAttribute('aria-label'), 'Global Tools views');
assert.deepEqual(
  [...rootElement.querySelectorAll('.velvet-view')].map((element) => element.getAttribute('aria-label')),
  ['First Tool', 'Second Tool'],
);

await act(() => {
  focus.focus({ kind: 'part', part: 'auxiliary-sidebar' });
  focus.focus({ kind: 'view', viewId: 'sample.global.first' });
});
await tick();
assert.equal(
  document.activeElement?.dataset.velvetFocusTarget,
  'view:sample.global.first',
);

const resizeHandle = rootElement.querySelector('.velvet-resize-handle--sidebar-start');
assert.ok(resizeHandle);
await act(() => {
  resizeHandle.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowLeft' }));
});
assert.deepEqual(commands.at(-1), {
  id: 'workbench.resize-part',
  part: 'auxiliary-sidebar',
  size: 310,
});

const hiddenLayout = {
  ...layout,
  revision: layout.revision + 1,
  views: {
    ...layout.views,
    'sample.global.first': { ...layout.views['sample.global.first'], visible: false },
  },
};
await render(hiddenLayout);
await tick();
await tick();
assert.equal(
  document.activeElement?.dataset.velvetFocusTarget,
  'part:auxiliary-sidebar',
);

await render(layout);
await tick();
const restoredContainer = rootElement.querySelector('#velvet-view-container-sample-global-tools');
await act(() => {
  restoredContainer.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, button: 2 }));
});
await tick();
assert.ok(
  [...document.querySelectorAll('.velvet-context-menu__item')]
    .some((item) => item.textContent === 'Hide Global Tools'),
);

await act(() => root.unmount());
await tick();
console.log('Workbench real DOM regression tests passed.');
