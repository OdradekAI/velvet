#!/usr/bin/env node

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const args = parseArgs(process.argv.slice(2));
const artifactPath = selectArtifact(args);
const version = required(args, 'version');
const channel = required(args, 'channel');
const target = required(args, 'target');
const signingKeyId = required(args, 'signing-key-id');
const outputPath = required(args, 'output');
const releaseSequence = Number(required(args, 'release-sequence'));
if (!Number.isSafeInteger(releaseSequence) || releaseSequence < 1) {
  throw new Error('--release-sequence must be a positive integer');
}
if (!['stable', 'preview'].includes(channel)) {
  throw new Error('--channel must be stable or preview');
}

const now = Date.now();
const issuedAtMs = integerOption(args, 'issued-at-ms', now);
const expiresAtMs = integerOption(args, 'expires-at-ms', now + 180 * 24 * 60 * 60 * 1000);
const manifest = {
  schemaVersion: 1,
  channel,
  version: version.replace(/^v/, ''),
  target,
  artifactSha256: sha256(fs.readFileSync(artifactPath)),
  updaterSchema: boundedIntegerOption(args, 'updater-schema', 1, 0xffff),
  minDataSchema: boundedIntegerOption(args, 'min-data-schema', 1, 0xffff),
  maxDataSchema: boundedIntegerOption(args, 'max-data-schema', 1, 0xffff),
  minClientVersion: args['min-client-version'] ?? '0.0.1',
  releaseSequence,
  issuedAtMs,
  expiresAtMs,
  signingKeyId,
};
if (!Number.isSafeInteger(issuedAtMs) || !Number.isSafeInteger(expiresAtMs) || expiresAtMs <= issuedAtMs) {
  throw new Error('manifest validity timestamps are invalid');
}
if (manifest.minDataSchema > manifest.maxDataSchema) {
  throw new Error('--min-data-schema cannot exceed --max-data-schema');
}

const privateKey = loadPrivateKey();
const canonical = JSON.stringify(manifest);
manifest.signature = crypto.sign(null, Buffer.from(canonical), privateKey).toString('hex');
fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');

function selectArtifact(options) {
  if (options.artifact) {
    if (!fs.statSync(options.artifact).isFile()) throw new Error(`artifact is not a file: ${options.artifact}`);
    return options.artifact;
  }
  const raw = options['artifact-paths'] ?? process.env.VELVET_UPDATER_ARTIFACT_PATHS;
  if (!raw) throw new Error('--artifact or VELVET_UPDATER_ARTIFACT_PATHS is required');
  let paths;
  try {
    paths = JSON.parse(raw);
  } catch {
    paths = raw.split(/\r?\n/).filter(Boolean);
  }
  if (!Array.isArray(paths)) throw new Error('artifact paths must be a JSON array');
  let candidates = paths.filter((value) => typeof value === 'string' && !value.endsWith('.sig'));
  if (options['artifact-suffix']) {
    candidates = candidates.filter((value) => value.endsWith(options['artifact-suffix']));
  }
  if (candidates.length !== 1) {
    throw new Error(`expected exactly one updater artifact, found ${candidates.length}: ${candidates.join(', ')}`);
  }
  return candidates[0];
}

function loadPrivateKey() {
  let value = process.env.VELVET_RELEASE_MANIFEST_PRIVATE_KEY?.trim();
  if (!value) throw new Error('VELVET_RELEASE_MANIFEST_PRIVATE_KEY is required');
  if (value.startsWith('@')) value = fs.readFileSync(value.slice(1), 'utf8').trim();
  if (value.includes('BEGIN')) return crypto.createPrivateKey(value);
  let seed;
  if (/^[0-9a-fA-F]{64}$/.test(value)) seed = Buffer.from(value, 'hex');
  else seed = Buffer.from(value, 'base64');
  if (seed.length !== 32) throw new Error('manifest private key must be a PEM, 32-byte hex or base64 seed');
  return crypto.createPrivateKey({
    key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seed]),
    format: 'der',
    type: 'pkcs8',
  });
}

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function required(options, name) {
  const value = options[name];
  if (!value) throw new Error(`--${name} is required`);
  return value;
}

function integerOption(options, name, fallback) {
  const value = Number(options[name] ?? fallback);
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`--${name} must be a non-negative integer`);
  }
  return value;
}

function boundedIntegerOption(options, name, fallback, maximum) {
  const value = integerOption(options, name, fallback);
  if (value > maximum) throw new Error(`--${name} exceeds ${maximum}`);
  return value;
}

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith('--')) throw new Error(`unexpected argument: ${token}`);
    const name = token.slice(2);
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`missing value for --${name}`);
    options[name] = value;
    index += 1;
  }
  return options;
}
