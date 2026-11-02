#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const outputPath = process.argv[2];
if (!outputPath) {
  console.error('usage: write-tauri-updater-config.mjs <output-path>');
  process.exit(2);
}

const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY?.trim();
if (!publicKey) {
  console.error('TAURI_UPDATER_PUBLIC_KEY is required for release updater artifacts');
  process.exit(2);
}

const channel = process.env.TAURI_RELEASE_CHANNEL?.trim();
if (!channel || !['stable', 'preview'].includes(channel)) {
  console.error('TAURI_RELEASE_CHANNEL must be stable or preview');
  process.exit(2);
}

const endpoint = process.env.TAURI_UPDATER_ENDPOINT?.trim();
if (!endpoint || !endpoint.startsWith('https://')) {
  console.error(
    'TAURI_UPDATER_ENDPOINT must be an explicit HTTPS endpoint for the selected release channel',
  );
  process.exit(2);
}
const config = {
  bundle: { createUpdaterArtifacts: true },
  plugins: {
    updater: {
      pubkey: publicKey,
      endpoints: [endpoint],
      requireSignedVersion: true,
    },
  },
};

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
