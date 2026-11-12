import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { docsRoot, englishPages } from './docs-content.mjs';

const pages = await englishPages();
const normalizeRoute = (route) => route.replace(/\/$/, '') || '/';

for (const locale of ['en', 'zh-Hans']) {
  const output = path.join(docsRoot, 'build', locale === 'en' ? '' : locale);
  const home = await readFile(path.join(output, 'index.html'), 'utf8');
  const canonicalTag = [...home.matchAll(/<link\b[^>]*>/g)]
    .find(([tag]) => /\brel=(?:"canonical"|'canonical'|canonical)(?:\s|>)/.test(tag))?.[0];
  const href = canonicalTag?.match(/\bhref=(?:"([^"]+)"|'([^']+)'|([^\s>]+))/);
  const canonical = href?.slice(1).find((value) => value !== undefined);
  assert.ok(canonical, `${locale}: home page has no canonical URL`);
  const baseUrl = new URL(canonical).pathname;
  assert.ok(baseUrl.endsWith('/'), `${locale}: expected a trailing slash in baseUrl`);
  const expectedRoutes = new Map(pages.map(({ route }) => [
    normalizeRoute(baseUrl + route.slice(1)), route,
  ]));
  const files = await readdir(output);
  const indexFiles = files.filter((file) => /^search-index-[\w-]+\.json$/.test(file));
  assert.equal(indexFiles.length, 1, `${locale}: expected one versioned search index`);
  assert.ok(!files.includes('search-index.json'), `${locale}: obsolete bespoke index is still published`);

  const indexes = JSON.parse(await readFile(path.join(output, indexFiles[0]), 'utf8'));
  const titles = indexes[0].documents;
  assert.equal(titles.length, pages.length, `${locale}: search title coverage differs from published pages`);
  assert.deepEqual(new Set(titles.map(({ u }) => normalizeRoute(u))), new Set(expectedRoutes.keys()),
    `${locale}: index contains missing, unpublished or foreign-locale pages`);

  const htmlByRoute = new Map();
  for (const [url, route] of expectedRoutes) {
    const stem = path.join(output, route.slice(1));
    const candidates = route === '/' ? [path.join(output, 'index.html')] : [stem + '.html', path.join(stem, 'index.html')];
    const html = await Promise.any(candidates.map((file) => readFile(file, 'utf8')));
    htmlByRoute.set(url, new Set([...html.matchAll(/\bid=(?:"([^"]+)"|'([^']+)'|([^\s>]+))/g)]
      .map((match) => match.slice(1).find((value) => value !== undefined))));
  }

  let sections = 0;
  for (const index of indexes) {
    assert.ok(Array.isArray(index.documents), `${locale}: unsupported search index format`);
    for (const record of index.documents) {
      const route = normalizeRoute(record.u);
      assert.ok(expectedRoutes.has(route), `${locale}: unexpected indexed route ${record.u}`);
      if (record.h) {
        const id = decodeURIComponent(record.h.replace(/^#/, ''));
        assert.ok(htmlByRoute.get(route).has(id), `${locale}: missing indexed anchor ${record.u}${record.h}`);
        sections += 1;
      }
    }
  }
  await readFile(path.join(output, 'search/index.html'), 'utf8');
  console.log(`${locale}: ${titles.length} published pages and ${sections} section links verified in the local search index.`);
}
