import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../../examples/plugins/asset-inspector/', import.meta.url));
const manifest = JSON.parse(await readFile(`${root}/manifest.json`, 'utf8'));
const { activate, EXTENSION_ID, FACET_NAMESPACE, JOB_KIND } = await import(
  `${root}/src/asset-inspector.mjs`
);

assert.equal(manifest.id, EXTENSION_ID);
assert.equal(manifest.runtime, 'built-in');
assert.equal(manifest.targetApp.appId, 'asset-manager');
assert.ok(manifest.contributions.some((contribution) => contribution.id === JOB_KIND));
assert.ok(manifest.requiredCapabilities.includes('asset.facet.write'));
assert.ok(manifest.activationEvents.some((event) => event.kind === 'onJob'));

const assets = new Map([
  ['asset-1', { id: 'asset-1', contentKind: 'image', fingerprint: 'sha256:asset-1:v1' }],
  ['asset-2', { id: 'asset-2', contentKind: 'text/plain', fingerprint: 'sha256:asset-2:v1' }],
]);
const facets = new Map();
const registrations = [];
const logs = [];
const notifications = [];
let nextJobId = 1;
let staleNextJob = false;
let cancelledNextJob = false;
let unknownNextJob = false;
let migrated;
const jobRequests = new Map();

const host = {
  commands: {
    register: (id, handler) => {
      registrations.push({ kind: 'command', id, handler });
      return () => registrations.splice(registrations.findIndex((entry) => entry.id === id), 1);
    },
  },
  menus: {
    register: (id) => {
      registrations.push({ kind: 'menu', id });
      return () => registrations.splice(registrations.findIndex((entry) => entry.id === id), 1);
    },
  },
  keybindings: {
    register: (id) => {
      registrations.push({ kind: 'keybinding', id });
      return () => registrations.splice(registrations.findIndex((entry) => entry.id === id), 1);
    },
  },
  views: {
    register: (id) => {
      registrations.push({ kind: 'view', id });
      return () => registrations.splice(registrations.findIndex((entry) => entry.id === id), 1);
    },
  },
  preview: {
    registerAugmentation: (id, handler) => {
      registrations.push({ kind: 'preview', id, handler });
      return () => registrations.splice(registrations.findIndex((entry) => entry.id === id), 1);
    },
  },
  assets: {
    readProjection: async (id) => ({ ...assets.get(id) }),
  },
  configuration: {
    version: async () => 'demo-settings-v1',
  },
  jobs: {
    create: async (request) => {
      const id = `job-${nextJobId++}`;
      jobRequests.set(id, request);
      return { id, status: 'queued', request };
    },
    wait: async (id) => {
      if (cancelledNextJob) {
        cancelledNextJob = false;
        return { id, status: 'cancelled', result: null };
      }
      if (unknownNextJob) {
        unknownNextJob = false;
        return { id, status: 'unknown', result: null };
      }
      const request = jobRequests.get(id);
      if (staleNextJob) {
        staleNextJob = false;
        assets.get('asset-1').fingerprint = 'sha256:asset-1:v2';
      }
      return {
        id,
        status: 'completed',
        result: {
          findings: [
            {
              assetId: 'asset-1',
              inputFingerprint: request.input.assets[0].fingerprint,
              summary: 'Readable image',
              score: 0.98,
            },
          ],
        },
      };
    },
  },
  facets: {
    write: async (record) => {
      const current = assets.get(record.assetId);
      if (current.fingerprint !== record.expectedFingerprint) {
        const error = new Error('asset fingerprint is stale');
        error.code = 'stale-fingerprint';
        throw error;
      }
      facets.set(`${record.namespace}:${record.assetId}`, record);
    },
    read: async ({ namespace, assetId }) => facets.get(`${namespace}:${assetId}`),
  },
  storage: {
    migrate: async (from, to) => {
      migrated = { from, to };
      return migrated;
    },
  },
  diagnostics: { log: (record) => logs.push(record) },
  notifications: { show: (record) => notifications.push(record) },
};

const plugin = await activate(host);
assert.equal(registrations.length, 7);

await plugin.analyzeSelection({
  revision: 4,
  selection: [{ id: 'asset-1', kind: 'asset' }, { id: 'asset-2', kind: 'asset' }],
});
assert.ok(facets.has(`${FACET_NAMESPACE}:asset-1`));
assert.equal(facets.get(`${FACET_NAMESPACE}:asset-1`).schemaVersion, 1);
assert.equal(notifications.at(-1).severity, 'success');
assert.ok(!JSON.stringify(logs).includes('sourcePath'));

const previewHandler = registrations.find((entry) => entry.kind === 'preview').handler;
assert.deepEqual(
  await previewHandler({ assetId: 'asset-1' }),
  { slot: 'metadata', summary: 'Readable image' },
);

staleNextJob = true;
await assert.rejects(
  () => plugin.analyzeSelection({ revision: 5, selection: [{ id: 'asset-1', kind: 'asset' }] }),
  /stale/,
);
assert.equal(logs.at(-1).code, 'asset-inspector.stale-fingerprint');

cancelledNextJob = true;
const cancelled = await plugin.analyzeSelection({
  revision: 6,
  selection: [{ id: 'asset-2', kind: 'asset' }],
});
assert.equal(cancelled.status, 'cancelled');

unknownNextJob = true;
await assert.rejects(
  () => plugin.analyzeSelection({ revision: 7, selection: [{ id: 'asset-2', kind: 'asset' }] }),
  /unknown/,
);
assert.equal(logs.at(-1).code, 'asset-inspector.analysis-failed');

await plugin.migrateStorage(0);
assert.deepEqual(migrated, { from: 0, to: 1 });
plugin.dispose();
assert.equal(registrations.length, 0);
assert.ok(facets.has(`${FACET_NAMESPACE}:asset-1`));

console.log('Asset Inspector demo contract tests passed.');
