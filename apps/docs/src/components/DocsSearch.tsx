import { useEffect, useId, useState } from 'react';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

type SearchEntry = {
  readonly locale: string;
  readonly route: string;
  readonly title: string;
  readonly text: string;
  readonly audience: string;
};

function isSearchEntry(value: unknown): value is SearchEntry {
  if (typeof value !== 'object' || value === null) return false;
  const entry = value as Record<string, unknown>;
  return ['locale', 'route', 'title', 'text', 'audience'].every((key) => typeof entry[key] === 'string')
    && typeof entry.route === 'string' && entry.route.startsWith('/') && !entry.route.startsWith('//');
}

export default function DocsSearch() {
  const { i18n } = useDocusaurusContext();
  const chinese = i18n.currentLocale === 'zh-Hans';
  const url = useBaseUrl('/search-index.json');
  const inputId = useId();
  const [query, setQuery] = useState('');
  const [entries, setEntries] = useState<readonly SearchEntry[]>([]);
  const [state, setState] = useState<'loading' | 'ready' | 'error'>('loading');
  useEffect(() => {
    const controller = new AbortController();
    setState('loading');
    void fetch(url, { signal: controller.signal }).then(async (response) => {
      if (!response.ok) throw new Error('Search index unavailable');
      const value: unknown = await response.json();
      if (!Array.isArray(value) || !value.every(isSearchEntry)) throw new Error('Invalid search index');
      if (!controller.signal.aborted) {
        setEntries(value);
        setState('ready');
      }
    }).catch(() => { if (!controller.signal.aborted) setState('error'); });
    return () => controller.abort();
  }, [url]);

  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const results = terms.length === 0 ? [] : entries.filter((entry) => entry.locale === i18n.currentLocale
    && terms.every((term) => `${entry.title} ${entry.text}`.toLocaleLowerCase().includes(term)))
    .sort((left, right) => Number(right.title.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())) - Number(left.title.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())));
  const audiences: Record<string, string> = chinese
    ? { extension: '扩展契约', 'host-ui': '可信宿主 UI', fixture: 'Fixture', maintainer: '维护者' }
    : { extension: 'Extension contract', 'host-ui': 'Trusted host UI', fixture: 'Fixture', maintainer: 'Maintainer' };
  return (
    <div className="velvet-doc-search">
      <label htmlFor={inputId}>{chinese ? '搜索当前语言的文档或 API 标识符' : 'Search this language by topic or API identifier'}</label>
      <input id={inputId} type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="manifestSchema, surface.command, Jobs…" />
      <p role="status" aria-live="polite">
        {state === 'loading' ? (chinese ? '正在加载索引…' : 'Loading index…') : state === 'error' ? (chinese ? '索引加载失败，请刷新页面重试。' : 'Could not load the index. Reload to retry.') : terms.length === 0 ? (chinese ? '输入关键词。搜索在本机执行。' : 'Enter a query. Search runs locally.') : chinese ? `${results.length} 个结果${results.length > 50 ? '，显示前 50 个' : ''}` : `${results.length} results${results.length > 50 ? ', showing the first 50' : ''}`}
      </p>
      <ul>
        {results.slice(0, 50).map((entry) => <li key={entry.route}>
          <Link to={entry.route}>{entry.title}</Link>{' '}<small>{audiences[entry.audience]}</small>
          <p>{entry.text.slice(0, 180)}…</p>
        </li>)}
      </ul>
    </div>
  );
}
