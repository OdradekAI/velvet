import assert from 'node:assert/strict';
import { PreviewRendererRegistry } from '/tmp/velvet-plugin-tests/workbench/preview/preview-renderer-registry.js';

const events = [];
const registry = new PreviewRendererRegistry(() => ({
  mount: (endpoint) => {
    events.push('mount');
    endpoint.receive({ type: 'surface.ready', bridgeVersion: '1' });
  },
  unmount: (reason) => {
    events.push(`unmount:${reason}`);
  },
}));

const preview = {
  candidate: {
    providerId: 'sample.preview.image',
    contentKinds: ['image'],
    intents: ['quick'],
    surfaces: ['workspace'],
    priority: 20,
  },
  session: {
    providerId: 'sample.preview.image',
    context: {
      assetId: 'asset-1',
      contentKind: 'image',
      intent: 'quick',
      surface: 'workspace',
    },
    sessionId: 'sample.preview.image:asset-1',
    renderer: {
      kind: 'ui-bridge',
      entrypoint: 'data:text/html,%3Cstrong%3Epreview%3C%2Fstrong%3E',
      bridgeVersion: '1',
    },
    commands: ['preview.zoom'],
    slots: ['metadata'],
  },
  diagnostic: null,
};

const session = registry.createSession(preview, {});
assert.ok(session);
await session.mount({
  context: {
    revision: 0,
    activeAppId: 'asset-manager',
    dataScope: null,
    selection: [],
    query: '',
    filters: {},
    sort: [],
    handoff: null,
  },
  theme: { scheme: 'dark', revision: 0, tokens: {} },
  size: { width: 320, height: 240 },
}, {});
assert.equal(session.state, 'mounted');
session.activate();
assert.equal(session.state, 'active');
await session.dispose('unmount');
assert.deepEqual(events, ['mount', 'unmount:unmount']);

assert.equal(
  registry.createSession({
    ...preview,
    session: { ...preview.session, renderer: { kind: 'generic' } },
  }, {}),
  undefined,
);

console.log('Preview renderer registry tests passed.');
