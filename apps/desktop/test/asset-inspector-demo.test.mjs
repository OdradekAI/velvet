import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../../examples/plugins/asset-inspector/', import.meta.url));
const manifest = JSON.parse(await readFile(`${root}/manifest.json`, 'utf8'));
const {
  activate,
  AssetInspectorError,
  EXTENSION_ID,
  FACET_NAMESPACE,
  FACET_SCHEMA_VERSION,
  HOST_API_RANGE,
  JOB_KIND,
  UI_COMPONENTS,
  UI_KIT_API_VERSION,
  validateManifest,
} = await import(`${root}/src/asset-inspector.mjs`);
const { createAssetInspectorFixtureHost } = await import(
  `${root}/src/fixture-host.mjs`
);

validateManifest(manifest);
assert.equal(manifest.id, EXTENSION_ID);
assert.equal(manifest.runtime, 'built-in');
assert.equal(manifest.hostApi, HOST_API_RANGE);
assert.equal(manifest.bridgeApi, '^1');
assert.equal(manifest.targetApp.appId, 'asset-manager');
assert.equal(manifest.storage.schemaVersion, FACET_SCHEMA_VERSION);
assert.ok(manifest.contributions.some((contribution) => contribution.id.endsWith('command.analyze-selection')));
assert.ok(manifest.contributions.some((contribution) => contribution.id.endsWith('menu.asset.analyze')));
assert.ok(manifest.contributions.some((contribution) => contribution.id.endsWith('keybinding.analyze-selection')));
assert.ok(manifest.contributions.some((contribution) => contribution.id.endsWith('view.inspector')));
for (const capability of [
  'asset.read',
  'asset.facet.write',
  'config.read',
  'jobs.create',
  'jobs.read',
  'storage.read',
  'storage.write',
  'preview.render',
  'notifications.show',
  'logs.write',
  'command.register',
  'menu.register',
  'view.register',
]) {
  assert.ok(manifest.requiredCapabilities.includes(capability), `manifest must request ${capability}`);
}
assert.ok(manifest.activationEvents.some((event) => event.kind === 'onJob'));
for (const highRisk of ['filesystem', 'process', 'network', 'secret', 'core-metadata-write']) {
  assert.ok(!manifest.requiredCapabilities.includes(highRisk), `demo must not request ${highRisk}`);
}
assert.ok(!manifest.contributions.some((contribution) => contribution.ui?.entrypoint.includes('<body>')));

const { host, state } = createAssetInspectorFixtureHost();
const plugin = await activate(host);
assert.equal(state.registrations.length, 7);
assert.equal(state.uiRegistrations.size, 1);
assert.deepEqual(state.uiRegistrations.values().next().value.components, UI_COMPONENTS);

await plugin.saveSettings({ qualityThreshold: 0.9 });
assert.deepEqual(await plugin.readSettings(), { qualityThreshold: 0.9 });

const beforeAsset = { ...state.assets.get('asset-1') };
const completed = await plugin.analyzeSelection({
  revision: 4,
  selection: [{ id: 'asset-1', kind: 'asset' }, { id: 'asset-2', kind: 'asset' }],
});
assert.equal(completed.status, 'completed');
assert.ok(state.contextReads >= 1);
assert.ok(state.coreAssetReads >= 2);
assert.ok(state.facets.has(`${FACET_NAMESPACE}:asset-1`));
assert.equal(state.facets.get(`${FACET_NAMESPACE}:asset-1`).schemaVersion, 1);
assert.equal(state.notifications.at(-1).severity, 'success');
assert.ok(state.uiEvents.some((event) => event.component === 'progress'));
assert.ok(state.uiEvents.some((event) => event.component === 'table'));
assert.deepEqual(state.assets.get('asset-1'), beforeAsset);
assert.ok(!JSON.stringify(state.logs).includes('sourcePath'));

const previewHandler = state.registrations.find((entry) => entry.kind === 'preview').handler;
assert.deepEqual(
  await previewHandler({ assetId: 'asset-1' }),
  { slot: 'metadata', summary: 'Readable asset' },
);

state.nextJobMode = 'stale';
await assert.rejects(
  () => plugin.analyzeSelection({ revision: 5, selection: [{ id: 'asset-1', kind: 'asset' }] }),
  (error) => error instanceof AssetInspectorError && error.code === 'stale-fingerprint',
);
assert.equal(state.logs.at(-1).code, 'asset-inspector.stale-fingerprint');
state.assets.get('asset-1').fingerprint = 'sha256:asset-1:v1';

state.nextJobMode = 'cancelled';
const cancelled = await plugin.analyzeSelection({
  revision: 6,
  selection: [{ id: 'asset-2', kind: 'asset' }],
});
assert.equal(cancelled.status, 'cancelled');

state.nextJobMode = 'blocked';
const pending = plugin.analyzeSelection({
  revision: 7,
  selection: [{ id: 'asset-2', kind: 'asset' }],
});
await new Promise((resolve) => setImmediate(resolve));
assert.ok(state.lastJobId);
await plugin.cancelAnalysis(state.lastJobId);
assert.equal((await pending).status, 'cancelled');

state.nextJobMode = 'unknown';
await assert.rejects(
  () => plugin.analyzeSelection({ revision: 8, selection: [{ id: 'asset-2', kind: 'asset' }] }),
  (error) => error instanceof AssetInspectorError && error.code === 'job-failed',
);
assert.equal(state.logs.at(-1).code, 'asset-inspector.analysis-failed');

state.nextJobMode = 'crash';
await assert.rejects(
  () => plugin.analyzeSelection({ revision: 9, selection: [{ id: 'asset-2', kind: 'asset' }] }),
  (error) => error instanceof AssetInspectorError && error.code === 'runtime-crash',
);
assert.equal(state.crashCount, 1);
assert.ok(state.facets.has(`${FACET_NAMESPACE}:asset-1`));

const denied = createAssetInspectorFixtureHost({ deniedCapabilities: ['asset.facet.write'] });
const deniedPlugin = await activate(denied.host);
await assert.rejects(
  () => deniedPlugin.analyzeSelection({ revision: 10, selection: [{ id: 'asset-1', kind: 'asset' }] }),
  (error) => error instanceof AssetInspectorError && error.code === 'permission-denied',
);
assert.equal(denied.state.facets.size, 0);
assert.equal(denied.state.logs.at(-1).code, 'asset-inspector.permission-denied');
deniedPlugin.dispose();
assert.equal(denied.state.registrations.length, 0);
assert.equal(denied.state.uiRegistrations.size, 0);

const failingSurfaces = createAssetInspectorFixtureHost({
  nextJobMode: 'unknown',
  failLogs: true,
  failNotifications: true,
});
const failingPlugin = await activate(failingSurfaces.host);
await assert.rejects(() => failingPlugin.analyzeSelection({
  revision: 11,
  selection: [{ id: 'asset-1', kind: 'asset' }],
}));
failingPlugin.dispose();

const incompatible = createAssetInspectorFixtureHost({ apiVersion: '2' });
await assert.rejects(
  () => activate(incompatible.host),
  (error) => error instanceof AssetInspectorError && error.code === 'host-api-incompatible',
);
const incompatibleUi = createAssetInspectorFixtureHost({ uiApiVersion: '2.0' });
await assert.rejects(
  () => activate(incompatibleUi.host),
  (error) => error instanceof AssetInspectorError && error.code === 'ui-kit-incompatible',
);
assert.throws(
  () => validateManifest({ ...manifest, hostApi: '^2' }),
  (error) => error instanceof AssetInspectorError && error.code === 'manifest-incompatible',
);

await plugin.migrateStorage(0);
assert.deepEqual(state.migrated, { from: 0, to: 1 });
plugin.dispose();
assert.equal(state.registrations.length, 0);
assert.equal(state.uiRegistrations.size, 0);
assert.ok(state.facets.has(`${FACET_NAMESPACE}:asset-1`));

const reenabled = await activate(host);
assert.equal(state.registrations.length, 7);
assert.equal(state.uiRegistrations.size, 1);
reenabled.dispose();
assert.equal(state.registrations.length, 0);
assert.equal(state.uiRegistrations.size, 0);
assert.equal(UI_KIT_API_VERSION, '1.0');
assert.equal(JOB_KIND, 'com.acme.asset-inspector.job.asset-quality-analysis');

console.log('Asset Inspector demo contract tests passed.');
