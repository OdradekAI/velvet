#!/usr/bin/env node

import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, '../..');
const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'velvet-updater-'));
const artifactPath = path.join(temporaryDirectory, 'Velvet_0.0.5_aarch64.app.tar.gz');
const manifestPath = path.join(temporaryDirectory, 'velvet-manifest-darwin-aarch64.json');
const latestPath = path.join(temporaryDirectory, 'latest.json');
const mergedPath = path.join(temporaryDirectory, 'latest.merged.json');
const privateSeed = '09'.repeat(32);

fs.writeFileSync(artifactPath, Buffer.from('signed-artifact-fixture'));
run('generate-release-manifest.mjs', [
  '--artifact-paths', JSON.stringify([artifactPath, `${artifactPath}.sig`]),
  '--artifact-suffix', '.app.tar.gz',
  '--version', '0.0.5',
  '--channel', 'preview',
  '--target', 'macos-aarch64',
  '--signing-key-id', 'velvet-preview-root-1',
  '--release-sequence', '5',
  '--issued-at-ms', '1793491200000',
  '--expires-at-ms', '1796083200000',
  '--output', manifestPath,
], { VELVET_RELEASE_MANIFEST_PRIVATE_KEY: privateSeed });

const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const unsigned = { ...manifest };
delete unsigned.signature;
const privateKey = crypto.createPrivateKey({
  key: Buffer.concat([
    Buffer.from('302e020100300506032b657004220420', 'hex'),
    Buffer.from(privateSeed, 'hex'),
  ]),
  format: 'der',
  type: 'pkcs8',
});
const publicKey = crypto.createPublicKey(privateKey);
assert.equal(publicKey.asymmetricKeyType, 'ed25519');
const publicKeyHex = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32).toString('hex');
assert.equal(
  crypto.verify(null, Buffer.from(JSON.stringify(unsigned)), publicKey, Buffer.from(manifest.signature, 'hex')),
  true,
  'manifest signature must verify against the canonical unsigned payload',
);
run('verify-release-manifest.mjs', [
  '--manifest', manifestPath,
  '--channel', 'preview',
], { VELVET_RELEASE_MANIFEST_PUBLIC_KEY: publicKeyHex });

fs.copyFileSync(path.join(scriptDirectory, 'fixtures/tauri-latest-preview.json'), latestPath);
run('inject-velvet-manifests.mjs', [latestPath, temporaryDirectory, mergedPath, 'preview']);
run('validate-updater-json.mjs', [mergedPath, 'preview']);

const merged = JSON.parse(fs.readFileSync(mergedPath, 'utf8'));
assert.deepEqual(merged.velvetManifests['darwin-aarch64'], manifest);
assert.equal(merged.velvetManifest, undefined);

const invalidManifestPath = path.join(temporaryDirectory, 'invalid-latest.json');
const invalid = { ...merged, velvetManifests: { ...merged.velvetManifests,
  'darwin-aarch64': { ...manifest, signature: 'not-a-signature' },
} };
fs.writeFileSync(invalidManifestPath, JSON.stringify(invalid));
const invalidResult = spawnSync(process.execPath, [
  path.join(scriptDirectory, 'validate-updater-json.mjs'), invalidManifestPath, 'preview',
], { encoding: 'utf8' });
assert.notEqual(invalidResult.status, 0, 'invalid manifest signature must fail closed');

const configPath = path.join(temporaryDirectory, 'tauri.release.generated.json');
run('write-tauri-updater-config.mjs', [configPath], {
  TAURI_UPDATER_PUBLIC_KEY: 'test-public-key',
  TAURI_UPDATER_ENDPOINT: 'https://github.com/OdradekAI/velvet/releases/download/preview/latest.json',
  TAURI_RELEASE_CHANNEL: 'preview',
});
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
assert.deepEqual(config.plugins.updater.endpoints, [
  'https://github.com/OdradekAI/velvet/releases/download/preview/latest.json',
]);
assert.equal(config.plugins.updater.requireSignedVersion, true);

console.log('updater release qualification fixture passed');

function run(scriptName, arguments_, environment = {}) {
  const result = spawnSync(process.execPath, [path.join(scriptDirectory, scriptName), ...arguments_], {
    cwd: repositoryRoot,
    env: { ...process.env, ...environment },
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    process.stderr.write(result.stderr || result.stdout || `${scriptName} failed\n`);
    process.exit(result.status ?? 1);
  }
}
