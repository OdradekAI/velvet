import { lstat, readdir, readFile, realpath } from 'node:fs/promises';
import path from 'node:path';

const docsRoot = path.resolve(import.meta.dirname, '..');
const repositoryRoot = path.resolve(docsRoot, '../..');
const englishDocsRoot = path.join(docsRoot, 'docs/en');
const chineseDocsRoot = path.join(docsRoot, 'docs/zh-Hans');
const englishReferenceRoot = path.join(docsRoot, 'reference/en');
const chineseReferenceRoot = path.join(docsRoot, 'reference/zh-Hans');
const canonicalReferenceRoot = path.join(repositoryRoot, 'docs/engineering');

async function markdownFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await markdownFiles(entryPath));
    } else if (entry.isFile() && entry.name.endsWith('.md')) {
      files.push(entryPath);
    }
  }

  return files.sort();
}

async function canonicalReferenceFiles() {
  const canonicalTopLevelFiles = [
    'plugin-contract-matrix.md',
    'plugin-developer-guide.md',
    'plugin-ecosystem-qualification.md',
    'plugin-manifest-v1.md',
    'plugin-registry-v1.md',
    'ui-kit-component-guide.md',
    'ui-kit-contract-v1.md',
  ].map((file) => path.join(canonicalReferenceRoot, file));

  return [
    ...canonicalTopLevelFiles,
    ...await markdownFiles(path.join(canonicalReferenceRoot, 'ui-kit')),
  ].sort();
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
  let relativeRoute = route.replace(/^\//u, '').split(/[?#]/u, 1)[0];
  if (locale === 'zh-Hans') {
    relativeRoute = relativeRoute.replace(/^zh-Hans\/?/u, '');
  }

  const referenceRoot = locale === 'zh-Hans' ? chineseReferenceRoot : canonicalReferenceRoot;
  const docsRootForLocale = locale === 'zh-Hans' ? chineseDocsRoot : englishDocsRoot;

  if (relativeRoute === 'reference/ui-kit/components') {
    return [path.join(referenceRoot, 'ui-kit/components/README.md')];
  }

  if (relativeRoute.startsWith('reference/')) {
    return [path.join(referenceRoot, `${relativeRoute.slice('reference/'.length)}.md`)];
  }

  if (relativeRoute === '') {
    return [path.join(docsRootForLocale, 'index.md')];
  }

  return [
    path.join(docsRootForLocale, `${relativeRoute}.md`),
    path.join(docsRootForLocale, relativeRoute, 'index.md'),
  ];
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
  const routes = [...llms.matchAll(/^- (\/[^\s]+)$/gmu)].map((match) => match[1]);
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

  return routes.length;
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

console.log(
  `Docs checks passed: ${mainPages} main pages, `
  + `${referencePages} Reference pages mounted from docs/engineering, `
  + `${englishPages} English pages, `
  + `${llmsRoutes} llms routes, `
  + `${linkCount} internal links.`,
);
