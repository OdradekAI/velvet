import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { docsRoot, englishPages, pageRoute, sourceHash, translatedPath } from './docs-content.mjs';

const translations = JSON.parse(await readFile(path.join(docsRoot, 'translation-status.json'), 'utf8'));
const pages = await englishPages();
const metadata = {};
const search = [];
for (const page of pages) {
  const translated = await readFile(translatedPath(page.key), 'utf8');
  if (pageRoute(page.key, translated) !== page.route) throw new Error(`Locale routes differ: ${page.key}`);
  const review = translations[page.key];
  metadata[page.route] = {
    audience: page.audience,
    translationStatus: review?.status ?? 'unreviewed',
    stale: review?.sourceHash !== sourceHash(page.content) || review?.translationHash !== sourceHash(translated),
  };
  for (const [locale, content] of [['en', page.content], ['zh-Hans', translated]]) {
    const title = content.match(/^# (.+)$/m)?.[1] ?? page.key;
    // Only published sources enter the index. Results render as text, never HTML.
    const text = content.replace(/^---\n[\s\S]*?\n---\n/, '').replace(/^import .+$/gm, '').replace(/\{\/\*[\s\S]*?\*\/\}/g, '').replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
    search.push({ locale, route: page.route, title, text, audience: page.audience });
  }
}
await mkdir(path.join(docsRoot, 'src/generated'), { recursive: true });
await writeFile(path.join(docsRoot, 'src/generated/page-metadata.json'), JSON.stringify(metadata, null, 2) + '\n');
await writeFile(path.join(docsRoot, 'static/search-index.json'), JSON.stringify(search) + '\n');
const llms = '# Velvet Docs\n\nIn-repository Alpha contracts; no published external SDK. Host React UI is for trusted host code.\nChinese routes use /zh-Hans/. See each page for audience and translation status.\n\n## Published English routes\n\n' + pages.map(({ route }) => `- ${route}`).sort().join('\n') + '\n';
await writeFile(path.join(docsRoot, 'static/llms.txt'), llms);
console.log(`Generated local search and page metadata for ${pages.length} pages in both locales.`);
