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
const manifests = document.velvetManifests;
if (!manifests || typeof manifests !== 'object' || Object.keys(manifests).length === 0) {
  throw new Error('latest.json.velvetManifests is required for Velvet installation');
}
for (const [target, manifest] of Object.entries(manifests)) {
  if (!manifest || typeof manifest !== 'object') {
    throw new Error(`velvetManifests.${target} must be an object`);
  }
  if (!document.platforms[target] &&
      !Object.keys(document.platforms).some((platform) => platform.startsWith(`${target}-`))) {
    throw new Error(`velvetManifests.${target} has no matching updater platform`);
  }
  if (manifest.version !== document.version.replace(/^v/, '')) {
    throw new Error(`velvetManifests.${target}.version must match latest.json.version`);
  }
  if (typeof manifest.target !== 'string' || !manifest.target ||
      typeof manifest.signingKeyId !== 'string' || !manifest.signingKeyId ||
      typeof manifest.minClientVersion !== 'string' || !manifest.minClientVersion) {
    throw new Error(`velvetManifests.${target} must contain target and signingKeyId`);
  }
  if (typeof manifest.artifactSha256 !== 'string' || !/^[0-9a-f]{64}$/i.test(manifest.artifactSha256)) {
    throw new Error(`velvetManifests.${target}.artifactSha256 must be a SHA-256 hex digest`);
  }
  if (!Number.isInteger(manifest.schemaVersion) || manifest.schemaVersion < 1 ||
      !Number.isInteger(manifest.updaterSchema) || manifest.updaterSchema < 1 ||
      !Number.isInteger(manifest.minDataSchema) || !Number.isInteger(manifest.maxDataSchema) ||
      manifest.minDataSchema > manifest.maxDataSchema ||
      !Number.isInteger(manifest.issuedAtMs) || !Number.isInteger(manifest.expiresAtMs) ||
      manifest.expiresAtMs <= manifest.issuedAtMs) {
    throw new Error(`velvetManifests.${target} has invalid schema or validity fields`);
  }
  if (expectedChannel && manifest.channel !== expectedChannel) {
    throw new Error(`velvetManifests.${target}.channel must be ${expectedChannel}`);
  }
  if (!Number.isInteger(manifest.releaseSequence) || manifest.releaseSequence < 1) {
    throw new Error(`velvetManifests.${target}.releaseSequence must be a positive integer`);
  }
  if (typeof manifest.signature !== 'string' || !/^[0-9a-f]{128}$/i.test(manifest.signature)) {
    throw new Error(`velvetManifests.${target}.signature must be a 64-byte hex signature`);
  }
}
for (const platform of Object.keys(document.platforms)) {
  if (!Object.keys(manifests).some((target) =>
    platform === target || platform.startsWith(`${target}-`))) {
    throw new Error(`latest.json platform ${platform} has no Velvet release manifest`);
  }
}
