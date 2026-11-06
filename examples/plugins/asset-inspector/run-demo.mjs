import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

import {
  activate,
  validateManifest,
} from './src/asset-inspector.mjs';
import { createAssetInspectorFixtureHost } from './src/fixture-host.mjs';

const packageRoot = fileURLToPath(new URL('./', import.meta.url));
const manifest = JSON.parse(await readFile(new URL('./manifest.json', import.meta.url), 'utf8'));
validateManifest(manifest);

const { host, state } = createAssetInspectorFixtureHost();
const plugin = await activate(host);
const result = await plugin.analyzeSelection({
  revision: 1,
  selection: [{ id: 'asset-1', kind: 'asset' }],
});
const output = {
  extensionId: manifest.id,
  status: result.status,
  registeredContributions: state.registrations.length,
  facets: [...state.facets.keys()],
  uiComponents: [...state.uiRegistrations.values()].map((surface) => surface.components),
  packageRoot,
};
plugin.dispose();
process.stdout.write(JSON.stringify(output) + '\n');
