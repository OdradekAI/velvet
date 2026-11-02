#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const [latestPath, manifestDirectory, outputPath, expectedChannel] = process.argv.slice(2);
if (!latestPath || !manifestDirectory || !outputPath) {
  console.error('usage: inject-velvet-manifests.mjs <latest.json> <manifest-dir> <output.json> [stable|preview]');
  process.exit(2);
}

const latest = JSON.parse(fs.readFileSync(latestPath, 'utf8'));
if (!latest.platforms || typeof latest.platforms !== 'object') {
  throw new Error('latest.json.platforms must be an object');
}
const manifests = {};
for (const file of fs.readdirSync(manifestDirectory).filter((name) =>
  name.startsWith('velvet-manifest-') && name.endsWith('.json'))) {
  const target = file
    .replace(/^velvet-manifest-/, '')
    .replace(/\.json$/, '');
  const manifest = JSON.parse(fs.readFileSync(path.join(manifestDirectory, file), 'utf8'));
  if (expectedChannel && manifest.channel !== expectedChannel) {
    throw new Error(`${file} has channel ${manifest.channel}, expected ${expectedChannel}`);
  }
  if (manifest.version !== String(latest.version).replace(/^v/, '')) {
    throw new Error(`${file} version does not match latest.json`);
  }
  if (typeof manifest.signature !== 'string' || !/^[0-9a-f]{128}$/i.test(manifest.signature)) {
    throw new Error(`${file} does not contain a 64-byte hex manifest signature`);
  }
  manifests[target] = manifest;
}
if (Object.keys(manifests).length === 0) throw new Error('manifest directory is empty');
for (const target of Object.keys(manifests)) {
  if (!latest.platforms[target]) {
    throw new Error(`manifest target ${target} has no matching latest.json platform`);
  }
}
for (const platform of Object.keys(latest.platforms)) {
  if (!manifests[platform]) {
    throw new Error(`latest.json platform ${platform} has no Velvet release manifest`);
  }
}
latest.velvetManifests = manifests;
delete latest.velvetManifest;
fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(latest, null, 2)}\n`, 'utf8');
