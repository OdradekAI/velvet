import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';

export const docsRoot = path.resolve(import.meta.dirname, '..');
export const repositoryRoot = path.resolve(docsRoot, '../..');

export async function markdownFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = await Promise.all(entries.map((entry) => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? markdownFiles(file) : entry.isFile() && file.endsWith('.md') ? [file] : [];
  }));
  return files.flat().sort();
}

export function sourceHash(content) {
  return createHash('sha256').update(content.replace(/\r\n/g, '\n')).digest('hex');
}

export function pageRoute(key, content) {
  const frontMatter = content.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1] ?? '';
  const prefix = key.startsWith('reference/') ? '/reference' : '';
  const relative = key.replace(/^(?:docs|reference)\//, '').replace(/\.md$/, '');
  const slug = frontMatter.match(/^slug:\s*(.+)$/m)?.[1]?.replace(/^['"]|['"]$/g, '');
  // Published pages use absolute slugs; reject new shapes instead of inventing routes.
  if (slug && !slug.startsWith('/')) throw new Error(`Use an absolute slug in ${key}`);
  return (prefix + (slug ?? `/${relative.replace(/(?:^|\/)(?:index|README)$/, '')}`)).replace(/\/$/, '') || '/';
}

export function audienceFor(key) {
  if (key.startsWith('docs/maintenance/') || /reference\/(plugin-registry-v1|plugin-contract-matrix|plugin-ecosystem-qualification|ui-kit\/action-coverage)\.md$/.test(key)) return 'maintainer';
  if (key === 'reference/plugin-developer-guide.md') return 'fixture';
  if (key.startsWith('docs/ui-kit/') || key.startsWith('reference/ui-kit/components/') || key === 'reference/ui-kit-component-guide.md') return 'host-ui';
  return 'extension';
}

export async function englishPages() {
  const guideRoot = path.join(docsRoot, 'docs/en');
  const references = JSON.parse(await readFile(path.join(docsRoot, 'reference-pages.json'), 'utf8'));
  if (!Array.isArray(references) || new Set(references).size !== references.length || references.some((file) => typeof file !== 'string' || !file.endsWith('.md') || file.includes('..') || file.startsWith('/'))) {
    throw new Error('reference-pages.json must be an explicit, unique list of relative Markdown files');
  }
  const entries = [
    ...(await markdownFiles(guideRoot)).map((file) => ({ key: `docs/${path.relative(guideRoot, file)}`, file })),
    ...references.map((file) => ({ key: `reference/${file}`, file: path.join(repositoryRoot, 'docs/engineering', file) })),
  ];
  return Promise.all(entries.map(async ({ key, file }) => {
    const content = await readFile(file, 'utf8');
    return { key, file, content, route: pageRoute(key, content), audience: audienceFor(key) };
  }));
}

export function translatedPath(key) {
  return path.join(docsRoot, key.replace(/^(docs|reference)\//, '$1/zh-Hans/'));
}

export function assertReviewedTranslation(key, source, translation, record, requireReviewed = false) {
  if (!record || !['reviewed', 'summary', 'unreviewed'].includes(record.status)) {
    throw new Error(`Missing translation status: ${key}`);
  }
  if (requireReviewed && record.status !== 'reviewed') {
    throw new Error(`Published translation is not fully reviewed: ${key}`);
  }
  if (record.status === 'reviewed' && (record.sourceHash !== sourceHash(source) || record.translationHash !== sourceHash(translation))) {
    throw new Error(`Translation review is stale: ${key}. Review both files, then record their hashes.`);
  }
}

function codeBlocks(markdown) {
  const blocks = [];
  const prose = markdown.replace(/^(`{3,}|~{3,})([^\n]*)\n([\s\S]*?)^\1[ \t]*$/gm, (_, fence, info, body) => {
    const language = info.trim().split(/\s+/)[0].toLowerCase();
    if (/^(?:sh|bash|shell|json|jsonc|js|javascript|ts|typescript|tsx|jsx|rust|toml|yaml|yml)$/.test(language)) {
      blocks.push(body.replace(/\r\n/g, '\n').trimEnd());
    }
    return '';
  });
  return { blocks, prose };
}

function apiTableRows(markdown) {
  const result = [];
  let columns = 0;
  for (const line of markdown.split('\n')) {
    if (!line.startsWith('|')) { columns = 0; continue; }
    const cells = line.split(/(?<!\\)\|/).slice(1, -1).map((cell) => cell.trim());
    if (/^(?:Type|类型)$/i.test(cells[1] ?? '')) {
      columns = /^(?:Default|默认|默认值)$/i.test(cells[2] ?? '') ? 3 : 2;
    } else if (columns && cells[0]?.startsWith('`')) {
      result.push(cells.slice(0, columns).join(' | '));
    }
  }
  return result.sort();
}

export function assertTranslationContractParity(key, source, translation) {
  const left = codeBlocks(source);
  const right = codeBlocks(translation);
  if (JSON.stringify(left.blocks) !== JSON.stringify(right.blocks)) {
    throw new Error(`Executable code examples differ between languages: ${key}`);
  }
  const literals = (prose) => [...new Set([...prose.matchAll(/(?<!`)`([^`\n]+)`(?!`)/g)].map(([, value]) => value))].sort();
  const englishLiterals = literals(left.prose);
  const translatedLiterals = literals(right.prose);
  if (JSON.stringify(englishLiterals) !== JSON.stringify(translatedLiterals)) {
    const missing = englishLiterals.filter((value) => !translatedLiterals.includes(value));
    const extra = translatedLiterals.filter((value) => !englishLiterals.includes(value));
    throw new Error(`Contract literals differ: ${key}\nMissing: ${missing.join(', ')}\nExtra: ${extra.join(', ')}`);
  }
  if (JSON.stringify(apiTableRows(left.prose)) !== JSON.stringify(apiTableRows(right.prose))) {
    throw new Error(`API names, types or defaults differ between languages: ${key}`);
  }
}

export function manifestFields(rustSource) {
  const body = rustSource.match(/struct RawExtensionManifest \{([\s\S]*?)\n\}/)?.[1];
  if (!body) throw new Error('Cannot locate RawExtensionManifest; review the docs checker');
  return [...body.matchAll(/^\s+(\w+): ([^\n]+),$/gm)]
    .filter(([, name]) => name !== 'extensions')
    .map(([, name, type]) => [name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase()), type.startsWith('Option<') ? 'optional' : 'required']);
}

export function documentedManifestFields(markdown) {
  const table = markdown.match(/\{\/\* manifest-fields:start \*\/\}([\s\S]*?)\{\/\* manifest-fields:end \*\/\}/)?.[1];
  if (!table) throw new Error('Manifest reference must delimit its field table');
  return [...table.matchAll(/^\| `([^`]+)` \| `(required|optional)` \|/gm)].map(([, name, presence]) => [name, presence]);
}

export function assertManifestFields(rustSource, markdown) {
  if (JSON.stringify(manifestFields(rustSource)) !== JSON.stringify(documentedManifestFields(markdown))) {
    throw new Error('Manifest field names/presence differ from RawExtensionManifest');
  }
}
