import assert from 'node:assert/strict';
import { PreviewHost } from '/tmp/velvet-plugin-tests/workbench/preview/preview-host.js';
import { PreviewRegistry } from '/tmp/velvet-plugin-tests/workbench/preview/preview-registry.js';

let shouldFail = false;
const core = {
  request: async () => {
    if (shouldFail) {
      throw new Error('preview provider unavailable');
    }
    return {
      kind: 'preview',
      preview: {
        candidate: {
          providerId: 'image.provider',
          contentKinds: ['image'],
          intents: ['quick'],
          surfaces: ['workspace'],
          priority: 10,
        },
        session: {
          providerId: 'image.provider',
          context: {
            assetId: 'asset-1',
            contentKind: 'image',
            intent: 'quick',
            surface: 'workspace',
          },
        },
      },
    };
  },
  subscribe: () => () => {},
};

const host = new PreviewHost(core);
const context = {
  assetId: 'asset-1',
  contentKind: 'image',
  intent: 'quick',
  surface: 'workspace',
};
const resolved = await host.open(context);
assert.equal(resolved.session.providerId, 'image.provider');
assert.equal(host.snapshot.state, 'ready');

shouldFail = true;
const fallback = await host.open(context);
assert.equal(fallback.session.providerId, 'builtin.generic-fallback');
assert.equal(host.snapshot.state, 'failed');
assert.match(host.snapshot.error, /provider unavailable/);

host.close();
assert.equal(host.snapshot.state, 'idle');
host.dispose();
assert.equal(host.snapshot.state, 'disposed');
await assert.rejects(() => host.open(context), /disposed/);

const registry = new PreviewRegistry();
const removeProvider = registry.registerProvider({
  id: 'sample.image',
  extensionId: 'sample.extension',
  contentKinds: ['image'],
  intents: ['quick'],
  surfaces: ['workspace'],
  priority: 10,
});
registry.registerAugmentation({
  id: 'sample.metadata',
  extensionId: 'sample.extension',
  contentKinds: ['image'],
  slot: 'metadata',
});
assert.equal(registry.listProviders()[0].id, 'sample.image');
assert.equal(registry.listAugmentations('image')[0].slot, 'metadata');
registry.unregisterExtension('sample.extension');
assert.equal(registry.listProviders().length, 0);
assert.equal(registry.listAugmentations('image').length, 0);
removeProvider();

console.log('Preview host and registry tests passed.');
