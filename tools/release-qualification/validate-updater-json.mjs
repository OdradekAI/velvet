#!/usr/bin/env node

import fs from 'node:fs';

const [inputPath, expectedChannel] = process.argv.slice(2);
if (!inputPath) {
  console.error('usage: validate-updater-json.mjs <latest.json> [stable|preview]');
  process.exit(2);
}

const document = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
if (typeof document.version !== 'string' || !document.version) {
  throw new Error('latest.json.version must be a non-empty string');
}
if (!document.platforms || typeof document.platforms !== 'object') {
  throw new Error('latest.json.platforms must be an object');
}
for (const [target, platform] of Object.entries(document.platforms)) {
  if (!platform || typeof platform.url !== 'string' || !platform.url.startsWith('https://')) {
    throw new Error(`${target}.url must be an HTTPS URL`);
  }
  if (typeof platform.signature !== 'string' || !platform.signature.trim()) {
    throw new Error(`${target}.signature must be present`);
  }
}
const manifest = document.velvetManifest;
if (!manifest || typeof manifest !== 'object') {
  throw new Error('latest.json.velvetManifest is required for Velvet installation');
}
if (manifest.version !== document.version.replace(/^v/, '')) {
  throw new Error('velvetManifest.version must match latest.json.version');
}
if (expectedChannel && manifest.channel !== expectedChannel) {
  throw new Error(`velvetManifest.channel must be ${expectedChannel}`);
}
if (!Number.isInteger(manifest.releaseSequence) || manifest.releaseSequence < 1) {
  throw new Error('velvetManifest.releaseSequence must be a positive integer');
}
if (typeof manifest.signature !== 'string' || manifest.signature.length !== 128) {
  throw new Error('velvetManifest.signature must be a 64-byte hex signature');
}
