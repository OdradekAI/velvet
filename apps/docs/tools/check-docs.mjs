import { lstat, readdir, readFile, realpath } from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { assertManifestFields, assertReviewedTranslation, assertTranslationContractParity, englishPages as loadEnglishPages, markdownFiles, pageRoute, translatedPath } from './docs-content.mjs';

const docsRoot = path.resolve(import.meta.dirname, '..');
const repositoryRoot = path.resolve(docsRoot, '../..');
const englishDocsRoot = path.join(docsRoot, 'docs/en');
const chineseDocsRoot = path.join(docsRoot, 'docs/zh-Hans');
const englishReferenceRoot = path.join(docsRoot, 'reference/en');
const chineseReferenceRoot = path.join(docsRoot, 'reference/zh-Hans');
const canonicalReferenceRoot = path.join(repositoryRoot, 'docs/engineering');
const publishedPages = await loadEnglishPages();
const publishedRoutes = new Map(publishedPages.map((page) => [page.route, page]));
if (publishedRoutes.size !== publishedPages.length) throw new Error('Duplicate published routes');

async function canonicalReferenceFiles() {
  return publishedPages.filter((page) => page.key.startsWith('reference/')).map((page) => page.file).sort();
}

function relativeMarkdownFiles(files, root) {
  return new Set(files.map((file) => path.relative(root, file)));
}

function formatList(values) {
  return values.map((value) => `  - ${value}`).join('\n');
}

function compareSets(left, right) {
  return {
    missing: [...left].filter((file) => !right.has(file)),
    extra: [...right].filter((file) => !left.has(file)),
  };
}

async function assertEnglishReferenceLink() {
  let stats;
  try {
    stats = await lstat(englishReferenceRoot);
  } catch {
    throw new Error(`English Reference link is missing: ${path.relative(repositoryRoot, englishReferenceRoot)}`);
  }

  if (!stats.isSymbolicLink()) {
    throw new Error(`English Reference must be a symbolic link: ${path.relative(repositoryRoot, englishReferenceRoot)}`);
  }

  const [actualTarget, expectedTarget] = await Promise.all([
    realpath(englishReferenceRoot),
    realpath(canonicalReferenceRoot),
  ]);
  if (actualTarget !== expectedTarget) {
    throw new Error(
      `English Reference link target mismatch\n`
      + `Expected: ${path.relative(repositoryRoot, canonicalReferenceRoot)}\n`
      + `Actual: ${path.relative(repositoryRoot, actualTarget)}`,
    );
  }
  const tracked = execFileSync('git', ['ls-files', '--stage', 'apps/docs/reference/en'], { cwd: repositoryRoot, encoding: 'utf8' });
  if (!tracked.startsWith('120000 ')) throw new Error('English Reference must remain a tracked symlink');
}

async function assertMainLanguageParity() {
  const englishFiles = await markdownFiles(englishDocsRoot);
  const chineseFiles = await markdownFiles(chineseDocsRoot);
  const englishSet = relativeMarkdownFiles(englishFiles, englishDocsRoot);
  const chineseSet = relativeMarkdownFiles(chineseFiles, chineseDocsRoot);
  const { missing, extra } = compareSets(englishSet, chineseSet);

  if (missing.length || extra.length) {
    const details = [
      missing.length ? `Missing Chinese pages\n${formatList(missing)}` : '',
      extra.length ? `Extra Chinese pages\n${formatList(extra)}` : '',
    ].filter(Boolean).join('\n');
    throw new Error(`Main language parity failed\n${details}`);
  }

  return englishFiles.length;
}

async function assertReferenceLanguageParity() {
  const englishFiles = await canonicalReferenceFiles();
  const chineseFiles = await markdownFiles(chineseReferenceRoot);
  const englishSet = relativeMarkdownFiles(englishFiles, canonicalReferenceRoot);
  const chineseSet = relativeMarkdownFiles(chineseFiles, chineseReferenceRoot);
  const { missing, extra } = compareSets(englishSet, chineseSet);

  if (missing.length || extra.length) {
    const details = [
      missing.length ? `Missing Chinese Reference pages\n${formatList(missing)}` : '',
      extra.length ? `Extra Chinese Reference pages\n${formatList(extra)}` : '',
    ].filter(Boolean).join('\n');
    throw new Error(`Reference language parity failed\n${details}`);
  }

  return englishFiles.length;
}

async function assertEnglishHasNoHanCharacters() {
  const englishFiles = [
    ...await markdownFiles(englishDocsRoot),
    ...await canonicalReferenceFiles(),
  ];
  const filesWithHan = [];

  for (const file of englishFiles) {
    const content = await readFile(file, 'utf8');
    if (/[\u3400-\u9fff]/u.test(content)) {
      filesWithHan.push(path.relative(repositoryRoot, file));
    }
  }

  if (filesWithHan.length) {
    throw new Error(`English documentation contains Han characters\n${formatList(filesWithHan)}`);
  }

  return englishFiles.length;
}

async function assertChineseLinksStayLocalized() {
  const chineseFiles = [
    ...await markdownFiles(chineseDocsRoot),
    ...await markdownFiles(chineseReferenceRoot),
  ];
  const filesWithDefaultLocaleLinks = [];

  for (const file of chineseFiles) {
    const content = await readFile(file, 'utf8');
    const links = [...content.matchAll(/\]\((\/[^)]+)\)/gu)].map((match) => match[1]);
    const explicitEnglishLinks = links.filter((link) => link.startsWith('/en/'));
    if (explicitEnglishLinks.length) {
      filesWithDefaultLocaleLinks.push(`${path.relative(docsRoot, file)}: ${explicitEnglishLinks.join(', ')}`);
    }
  }

  if (filesWithDefaultLocaleLinks.length) {
    throw new Error(`Chinese documentation contains explicit English-locale links\n${formatList(filesWithDefaultLocaleLinks)}`);
  }
}

function routeToSourcePaths(route, locale = 'en') {
  const normalized = route.split(/[?#]/u, 1)[0].replace(/^\/zh-Hans(?=\/|$)/, '').replace(/\/$/, '') || '/';
  const page = publishedRoutes.get(normalized);
  return page ? [locale === 'zh-Hans' ? translatedPath(page.key) : page.file] : [];
}

function isExternalLink(target) {
  return /^(?:[a-z][a-z\d+.-]*:|\/\/)/iu.test(target);
}

function linkTargetPath(target) {
  const trimmedTarget = target.trim();
  const angleTarget = trimmedTarget.match(/^<([^>]+)>/u);
  const unwrappedTarget = angleTarget ? angleTarget[1] : trimmedTarget;
  return unwrappedTarget.split(/[?#\s]/u, 1)[0];
}

function relativeLinkCandidates(sourceFile, target) {
  const targetPath = linkTargetPath(target);
  const absolutePath = path.resolve(path.dirname(sourceFile), targetPath);
  return [
    absolutePath,
    `${absolutePath}.md`,
    path.join(absolutePath, 'index.md'),
  ];
}

async function pathExists(filePath) {
  try {
    await readFile(filePath, 'utf8');
    return true;
  } catch {
    return false;
  }
}

async function assertMarkdownLinks() {
  const linkSources = [
    { locale: 'en', root: englishDocsRoot, files: await markdownFiles(englishDocsRoot) },
    { locale: 'zh-Hans', root: chineseDocsRoot, files: await markdownFiles(chineseDocsRoot) },
    { locale: 'en', root: canonicalReferenceRoot, files: await canonicalReferenceFiles() },
    { locale: 'zh-Hans', root: chineseReferenceRoot, files: await markdownFiles(chineseReferenceRoot) },
  ];
  const missingLinks = [];
  let checkedLinks = 0;

  for (const source of linkSources) {
    for (const file of source.files) {
      const content = await readFile(file, 'utf8');
      const links = [...content.matchAll(/\[[^\]]*\]\(([^)\n]+)\)/gu)];

      for (const [, rawTarget] of links) {
        const target = linkTargetPath(rawTarget);
        if (!target || target.startsWith('#') || isExternalLink(target)) {
          continue;
        }

        // The search theme owns this route; the production build validates it.
        if (target === '/search') {
          checkedLinks += 1;
          continue;
        }

        const candidates = target.startsWith('/')
          ? routeToSourcePaths(target, source.locale)
          : relativeLinkCandidates(file, target);
        const exists = await Promise.any(
          candidates.map(async (candidate) => {
            if (await pathExists(candidate)) {
              return true;
            }
            throw new Error('missing link target');
          }),
        ).catch(() => false);

        checkedLinks += 1;
        if (!exists) {
          missingLinks.push(`${path.relative(repositoryRoot, file)} -> ${rawTarget}`);
        }
      }
    }
  }

  if (missingLinks.length) {
    throw new Error(`Markdown link check failed\n${formatList(missingLinks)}`);
  }

  return checkedLinks;
}

async function assertLlmsRoutes() {
  const llmsPath = path.join(docsRoot, 'static/llms.txt');
  const llms = await readFile(llmsPath, 'utf8');
  const routes = [...llms.matchAll(/^- (\/[^\s]*)$/gmu)].map((match) => match[1]);
  const missingRoutes = [];

  for (const route of routes) {
    const candidates = routeToSourcePaths(route);
    const existing = await Promise.all(candidates.map(async (candidate) => {
      return pathExists(candidate);
    }));

    if (!existing.some(Boolean)) {
      missingRoutes.push(route);
    }
  }

  if (missingRoutes.length) {
    throw new Error(`llms.txt contains missing routes\n${formatList(missingRoutes)}`);
  }
  const coverage = compareSets(new Set(publishedRoutes.keys()), new Set(routes));
  if (coverage.missing.length || coverage.extra.length || new Set(routes).size !== routes.length) {
    throw new Error('llms.txt must list every published route once. Run npm run generate:index.');
  }

  return routes.length;
}

function firstCodeCells(markdown) {
  return [...markdown.matchAll(/^\| `([^`]+)` \|/gm)].map(([, cell]) => cell).sort();
}

async function assertContentContracts() {
  const records = JSON.parse(await readFile(path.join(docsRoot, 'translation-status.json'), 'utf8'));
  const recordKeys = compareSets(new Set(publishedPages.map((page) => page.key)), new Set(Object.keys(records)));
  if (recordKeys.missing.length || recordKeys.extra.length) throw new Error('Translation records must match published pages');
  const rust = await readFile(path.join(repositoryRoot, 'crates/plugin-api/src/lib.rs'), 'utf8');
  const bridge = await readFile(path.join(repositoryRoot, 'apps/desktop/src/workbench/plugins/ui-bridge.ts'), 'utf8');
  const bridgeKinds = [...bridge.slice(bridge.indexOf('export type HostToSurfaceMessage'), bridge.indexOf('export interface UiBridgeEndpoint')).matchAll(/readonly type: '([^']+)'/g)].map(([, kind]) => kind).sort();
  const protocol = await readFile(path.join(repositoryRoot, 'crates/workbench-protocol/src/protocol.rs'), 'utf8');
  const requestBody = protocol.match(/pub enum PluginRequest \{([\s\S]*?)\n\}/)?.[1] ?? '';
  const requestKinds = [...requestBody.matchAll(/^    (\w+) \{/gm)].map(([, kind]) => kind.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase()).sort();
  for (const page of publishedPages) {
    const translated = await readFile(translatedPath(page.key), 'utf8');
    assertReviewedTranslation(page.key, page.content, translated, records[page.key], process.argv.includes('--require-reviewed'));
    if (records[page.key].status === 'reviewed') assertTranslationContractParity(page.key, page.content, translated);
    if (pageRoute(page.key, translated) !== page.route) throw new Error(`Locale route mismatch: ${page.key}`);
    if (page.key === 'reference/plugin-manifest-v1.md') {
      assertManifestFields(rust, page.content);
      assertManifestFields(rust, translated);
    }
    if (page.key === 'reference/ui-bridge-protocol-v1.md') {
      for (const content of [page.content, translated]) {
        const table = content.match(/\{\/\* bridge-messages:start \*\/\}([\s\S]*?)\{\/\* bridge-messages:end \*\/\}/)?.[1] ?? '';
        if (JSON.stringify(firstCodeCells(table)) !== JSON.stringify(bridgeKinds)) throw new Error('Bridge message catalog differs from source');
      }
    }
    if (page.key === 'reference/plugin-api-index.md') {
      for (const content of [page.content, translated]) {
        if (JSON.stringify(firstCodeCells(content)) !== JSON.stringify(requestKinds)) throw new Error('Host request catalog differs from source');
      }
    }
    if (page.key.startsWith('docs/ui-kit/components/') && JSON.stringify(firstCodeCells(page.content)) !== JSON.stringify(firstCodeCells(translated))) {
      throw new Error(`Component prop tables differ between languages: ${page.key}`);
    }
  }
  const config = JSON.parse(await readFile(path.join(docsRoot, 'tsconfig.json'), 'utf8'));
  if (!config.compilerOptions.strict || !config.compilerOptions.isolatedModules) throw new Error('Keep docs TypeScript strict and isolatedModules enabled');
}

async function assertDemoImports(directory = path.join(docsRoot, 'src')) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) await assertDemoImports(file);
    else if (/\.tsx?$/.test(entry.name)) {
      const content = await readFile(file, 'utf8');
      for (const [, target] of content.matchAll(/(?:from\s+|import\s*(?:\(\s*)?)["']([^"']+)["']/g)) {
        if ((target.startsWith('@velvet-') && target !== '@velvet-ui') || (target.startsWith('.') && !path.resolve(path.dirname(file), target).startsWith(docsRoot + path.sep))) {
          throw new Error(`Docs demo import crosses its UI entrypoint: ${file} -> ${target}`);
        }
      }
    }
  }
}

const linksOnly = process.argv.includes('--links-only');
const sourceLinkOnly = process.argv.includes('--source-link-only');
await assertEnglishReferenceLink();

if (sourceLinkOnly) {
  console.log('English Reference source link check passed.');
  process.exit(0);
}

const linkCount = await assertMarkdownLinks();

if (linksOnly) {
  console.log(`Markdown link checks passed: ${linkCount} internal links.`);
  process.exit(0);
}

const [mainPages, referencePages, englishPages, llmsRoutes] = await Promise.all([
  assertMainLanguageParity(),
  assertReferenceLanguageParity(),
  assertEnglishHasNoHanCharacters(),
  assertLlmsRoutes(),
]);
await assertChineseLinksStayLocalized();
await assertContentContracts();
await assertDemoImports();

console.log(
  `Docs checks passed: ${mainPages} main pages, `
  + `${referencePages} Reference pages mounted from docs/engineering, `
  + `${englishPages} English pages, `
  + `${llmsRoutes} llms routes, `
  + `${linkCount} internal links.`,
);
