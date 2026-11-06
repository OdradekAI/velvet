import assert from 'node:assert/strict';
import { createPluginSurfaceAdapter } from '/tmp/velvet-plugin-tests/workbench/plugins/surface-adapters.js';

const windowListeners = new Map();
const postedMessages = [];
const receivedMessages = [];
let iframe;

globalThis.window = {
  addEventListener: (type, listener) => windowListeners.set(type, listener),
  removeEventListener: (type) => windowListeners.delete(type),
};
globalThis.document = {
  createElement: (type) => {
    assert.equal(type, 'iframe');
    iframe = {
      contentWindow: {
        postMessage: (message, origin) => postedMessages.push({ message, origin }),
      },
      className: '',
      title: '',
      src: '',
      style: {},
      setAttribute: () => undefined,
      remove: () => {
        iframe.removed = true;
      },
    };
    return iframe;
  },
};

const host = {
  replaceChildren: (child) => {
    host.child = child;
  },
  getBoundingClientRect: () => ({ left: 10, top: 20, width: 640, height: 480 }),
};
const hostListeners = new Set();
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
);
adapter.mount(endpoint, host);
assert.equal(host.child, iframe);
assert.equal(iframe.src, 'https://plugin.example/research');
assert.equal(windowListeners.has('message'), true);

for (const listener of hostListeners) {
  listener({ type: 'host.ready', bridgeVersion: '1', surfaceId: 'research.surface', theme: { scheme: 'dark', revision: 0, tokens: {} }, size: { width: 640, height: 480 }, context: { revision: 0, activeAppId: 'sample.extension.research', dataScope: null, selection: [], query: '', filters: {}, sort: [], handoff: null } });
}
assert.equal(postedMessages[0].message.type, 'velvet.ui.host');

windowListeners.get('message')({
  source: iframe.contentWindow,
  origin: 'https://evil.example',
  data: { type: 'velvet.ui.surface', message: { type: 'surface.error', message: 'spoofed' } },
});
assert.deepEqual(receivedMessages, []);

windowListeners.get('message')({
  source: iframe.contentWindow,
  origin: 'https://plugin.example',
  data: { type: 'velvet.ui.surface', message: { type: 'surface.ready', bridgeVersion: '1' } },
});
assert.deepEqual(receivedMessages, [{ type: 'surface.ready', bridgeVersion: '1' }]);
assert.equal(postedMessages[0].origin, 'https://plugin.example');

windowListeners.get('message')({
  source: iframe.contentWindow,
  origin: 'https://plugin.example',
  data: {
    type: 'velvet.ui.surface',
    message: {
      type: 'surface.command',
      requestId: 'request-1',
      commandId: 'plugin.command',
      args: { value: 'ok' },
      unknown: true,
    },
  },
});
assert.deepEqual(receivedMessages.at(-1), {
  type: 'surface.error',
  message: 'Invalid UI Bridge message',
});

windowListeners.get('message')({
  source: iframe.contentWindow,
  origin: 'https://plugin.example',
  data: {
    type: 'velvet.ui.surface',
    message: {
      type: 'surface.command',
      requestId: 'request-2',
      commandId: 'plugin.command',
      args: { value: 'x'.repeat(70_000) },
    },
  },
});
assert.deepEqual(receivedMessages.at(-1), {
  type: 'surface.error',
  message: 'Invalid UI Bridge message',
});

const nextHost = {
  replaceChildren: (child) => {
    nextHost.child = child;
  },
  getBoundingClientRect: () => ({ left: 12, top: 24, width: 700, height: 500 }),
};
await adapter.attach(nextHost);
assert.equal(nextHost.child, iframe);

await adapter.unmount('unmount');
assert.equal(iframe.removed, true);
assert.equal(windowListeners.has('message'), false);

console.log('Plugin iframe surface adapter tests passed.');
