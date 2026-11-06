import { writeFile } from 'node:fs/promises';

const { UI_KIT_CATALOG } = await import('/tmp/velvet-plugin-tests/app/ui/component-meta.js');
await writeFile(
  new URL('../src/app/ui/ui-kit-catalog.json', import.meta.url),
  `${JSON.stringify(UI_KIT_CATALOG, null, 2)}\n`,
  'utf8',
);
console.log(`Generated ${UI_KIT_CATALOG.components.length} Velvet UI Kit catalog entries.`);
