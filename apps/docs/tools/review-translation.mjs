import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { docsRoot, englishPages, sourceHash, translatedPath } from './docs-content.mjs';

const [key, status] = process.argv.slice(2);
if (!key || !['reviewed', 'summary', 'unreviewed'].includes(status)) {
  throw new Error('Usage: node tools/review-translation.mjs docs/platform/quick-start.md reviewed|summary|unreviewed');
}
const page = (await englishPages()).find((candidate) => candidate.key === key);
if (!page) throw new Error(`Unknown published page: ${key}`);
const file = path.join(docsRoot, 'translation-status.json');
const records = JSON.parse(await readFile(file, 'utf8'));
records[key] = {
  status,
  sourceHash: sourceHash(page.content),
  translationHash: sourceHash(await readFile(translatedPath(key), 'utf8')),
};
await writeFile(file, JSON.stringify(Object.fromEntries(Object.entries(records).sort()), null, 2) + '\n');
console.log(`Recorded ${status} translation review for ${key}. This records human/agent review, not machine proof of semantic equivalence.`);
