import assert from 'node:assert/strict';
import {
  createPluginSurfaceAdapter,
} from '/tmp/velvet-plugin-tests/workbench/plugins/surface-adapters.js';
import {
  UI_BRIDGE_HOST_EVENT,
  UI_BRIDGE_SURFACE_EVENT,
} from '/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js';

globalThis.isTauri = true;

const hostListeners = new Set();
const receivedMessages = [];
const emittedMessages = [];
const positions = [];
const sizes = [];
const lifecycle = [];
let surfaceListener;
let removedSurfaceListener = false;
let created;

const child = {
  async emit(event, payload) {
    emittedMessages.push({ event, payload });
  },
  async listen(event, listener) {
    assert.equal(event, UI_BRIDGE_SURFACE_EVENT);
    surfaceListener = listener;
    return () => {
      removedSurfaceListener = true;
      surfaceListener = undefined;
    };
  },
  once(event, listener) {
    if (event === 'tauri://created') {
      queueMicrotask(() => listener({ payload: undefined }));
    }
    return undefined;
  },
  async show() {
    lifecycle.push('show');
  },
  async hide() {
    lifecycle.push('hide');
  },
  async setFocus() {
    lifecycle.push('focus');
  },
  async setPosition(position) {
    positions.push(position);
  },
  async setSize(size) {
    sizes.push(size);
  },
  async close() {
    lifecycle.push('close');
  },
};

class LogicalPosition {
  constructor(x, y) {
    this.x = x;
    this.y = y;
  }
}

class LogicalSize {
  constructor(width, height) {
    this.width = width;
    this.height = height;
  }
}

const runtime = {
  getCurrentWebview: () => ({ window: 'main-window' }),
  createWebview: (window, label, options) => {
    created = { window, label, options };
    return child;
  },
  LogicalPosition,
  LogicalSize,
};

const endpoint = {
  subscribeHost: (listener) => {
    hostListeners.add(listener);
    return () => hostListeners.delete(listener);
  },
  subscribe: () => () => {},
  receive: (message) => receivedMessages.push(message),
  send: () => undefined,
  close: () => undefined,
};

const host = {
  getBoundingClientRect: () => ({ left: 10, top: 20, width: 640, height: 480 }),
};
const adapter = createPluginSurfaceAdapter(
  {
    id: 'research.surface',
    extensionId: 'sample.extension',
    namespace: 'workbench.app.sample.extension.research',
    kind: 'app-surface',
    entrypoint: 'https://plugin.example/research',
    bridgeVersion: '1',
    appId: 'sample.extension.research',
  },
  host,
  { tauriRuntime: runtime },
);

await adapter.mount(endpoint);
assert.deepEqual(created, {
  window: 'main-window',
  label: 'velvet-surface-sample_extension-research_surface',
  options: {
    url: 'https://plugin.example/research',
    x: 10,
    y: 20,
    width: 640,
    height: 480,
    focus: false,
    dragDropEnabled: false,
  },
});
assert.deepEqual({ x: positions.at(-1).x, y: positions.at(-1).y }, { x: 10, y: 20 });
assert.deepEqual(
  { width: sizes.at(-1).width, height: sizes.at(-1).height },
  { width: 640, height: 480 },
);

for (const listener of hostListeners) {
  listener({
    type: 'host.resize',
    size: { width: 640, height: 480 },
  });
  listener({ type: 'host.focus', focused: true });
  listener({ type: 'host.focus', focused: false });
}
await new Promise((resolve) => setImmediate(resolve));
assert.deepEqual(lifecycle, ['show', 'hide']);
lifecycle.length = 0;
for (const listener of hostListeners) {
  listener({ type: 'host.focus', focused: true });
}
await new Promise((resolve) => setImmediate(resolve));
assert.deepEqual(lifecycle, ['show', 'focus']);
for (const listener of hostListeners) {
  listener({ type: 'host.focus', focused: false });
}
await new Promise((resolve) => setImmediate(resolve));
assert.deepEqual(lifecycle, ['show', 'focus', 'hide']);
assert.equal(emittedMessages.at(-1).event, UI_BRIDGE_HOST_EVENT);
assert.equal(emittedMessages.at(-1).payload.type, 'host.focus');

surfaceListener({
  payload: {
    type: 'velvet.ui.surface',
    message: { type: 'surface.ready', bridgeVersion: '1' },
  },
});
assert.deepEqual(receivedMessages, [{ type: 'surface.ready', bridgeVersion: '1' }]);

const nextHost = {
  getBoundingClientRect: () => ({ left: 12, top: 24, width: 700, height: 500 }),
};
adapter.attach(nextHost);
await new Promise((resolve) => setImmediate(resolve));
assert.deepEqual({ x: positions.at(-1).x, y: positions.at(-1).y }, { x: 12, y: 24 });
assert.deepEqual(
  { width: sizes.at(-1).width, height: sizes.at(-1).height },
  { width: 700, height: 500 },
);

await adapter.unmount('unmount');
assert.equal(lifecycle.at(-1), 'close');
assert.equal(removedSurfaceListener, true);
assert.equal(hostListeners.size, 0);

console.log('Tauri native surface adapter tests passed.');
