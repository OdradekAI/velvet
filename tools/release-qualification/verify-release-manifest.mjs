#!/usr/bin/env node

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const args = parseArgs(process.argv.slice(2));
const inputPath = required(args, 'manifest');
const expectedChannel = args.channel;
const manifest = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
if (expectedChannel && manifest.channel !== expectedChannel) {
  throw new Error(`${path.basename(inputPath)} has channel ${manifest.channel}, expected ${expectedChannel}`);
}
if (typeof manifest.signature !== 'string' || !/^[0-9a-f]{128}$/i.test(manifest.signature)) {
  throw new Error(`${path.basename(inputPath)} does not contain a 64-byte hex signature`);
}
const unsigned = { ...manifest };
delete unsigned.signature;
const signature = Buffer.from(manifest.signature, 'hex');
const publicKey = loadPublicKey(args['public-key'] ?? process.env.VELVET_RELEASE_MANIFEST_PUBLIC_KEY);
if (!crypto.verify(null, Buffer.from(JSON.stringify(unsigned)), publicKey, signature)) {
  throw new Error(`${path.basename(inputPath)} has an invalid Ed25519 signature`);
}
console.log(`verified ${path.basename(inputPath)}`);

function loadPublicKey(value) {
  if (!value) throw new Error('--public-key or VELVET_RELEASE_MANIFEST_PUBLIC_KEY is required');
  if (value.startsWith('@')) value = fs.readFileSync(value.slice(1), 'utf8').trim();
  if (value.includes('BEGIN')) return crypto.createPublicKey(value);
  const bytes = /^[0-9a-fA-F]{64}$/.test(value)
    ? Buffer.from(value, 'hex')
    : Buffer.from(value, 'base64');
  if (bytes.length !== 32) throw new Error('manifest public key must be a PEM, 32-byte hex or base64 value');
  return crypto.createPublicKey({
    key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), bytes]),
    format: 'der',
    type: 'spki',
  });
}

function required(options, name) {
  const value = options[name];
  if (!value) throw new Error(`--${name} is required`);
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
