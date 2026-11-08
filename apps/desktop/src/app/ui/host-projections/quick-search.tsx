import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent, RefObject } from 'react';
import { VelvetInput, VelvetState } from '../primitives/index.js';
import type { UiCommonProps } from '../internal/types.js';

export type VelvetQuickSearchTabId = 'folders' | 'items' | 'tags' | 'smart-folders';

export interface VelvetQuickSearchTab {
  readonly id: VelvetQuickSearchTabId;
  readonly label: string;
}

export interface VelvetQuickSearchResult {
  readonly id: string;
  readonly kind: VelvetQuickSearchTabId;
  readonly label: string;
  readonly description?: string;
  /** Host-provided simplified/traditional/pinyin aliases used for matching. */
  readonly searchAliases?: readonly string[];
  readonly score?: number;
  readonly disabled?: boolean;
}

export interface VelvetQuickSearchProps extends UiCommonProps {
  readonly open: boolean;
  readonly tabs: readonly VelvetQuickSearchTab[];
  readonly results: readonly VelvetQuickSearchResult[];
  readonly activeTab?: VelvetQuickSearchTabId;
  readonly query?: string;
  readonly history?: readonly string[];
  readonly historyLimit?: number;
  readonly onQueryChange?: (query: string) => void;
  readonly onTabChange?: (tab: VelvetQuickSearchTabId) => void;
  readonly onHistorySelect?: (query: string) => void;
  readonly onSelect: (id: string, kind: VelvetQuickSearchTabId) => void;
  readonly onClose: () => void;
  readonly restoreFocusRef?: RefObject<HTMLElement | null>;
  readonly placeholder?: string;
}

export function VelvetQuickSearch({
  open,
  tabs,
  results,
  activeTab,
  query,
  history = [],
  historyLimit = 100,
  onQueryChange,
  onTabChange,
  onHistorySelect,
  onSelect,
  onClose,
  restoreFocusRef,
  placeholder = 'Quick Search',
}: VelvetQuickSearchProps) {
  const [internalQuery, setInternalQuery] = useState('');
  const [index, setIndex] = useState(0);
  const previousOpenRef = useRef(open);
  const listId = useId();
  const currentTab = activeTab ?? tabs[0]?.id ?? 'items';
  const resolvedQuery = query ?? internalQuery;
  const normalizedQuery = normalizeQuickSearchValue(resolvedQuery);
  const tabResults = results.filter((result) => result.kind === currentTab);
  const filteredResults = [...tabResults
    .filter((result) => matchesQuickSearchResult(result, normalizedQuery))]
    .sort((left, right) => (right.score ?? 0) - (left.score ?? 0));
  const visibleHistory = dedupeHistory(history, historyLimit);

  useEffect(() => setIndex(0), [currentTab, normalizedQuery]);
  useEffect(() => {
    if (previousOpenRef.current && !open) queueMicrotask(() => restoreFocusRef?.current?.focus());
    previousOpenRef.current = open;
  }, [open, restoreFocusRef]);

  if (!open) return null;

  const updateQuery = (nextQuery: string): void => {
    if (query === undefined) setInternalQuery(nextQuery);
    onQueryChange?.(nextQuery);
  };
  const selectCurrent = (): void => {
    const result = filteredResults[index];
    if (result && !result.disabled) onSelect(result.id, result.kind);
  };
  const selectHistory = (historyQuery: string): void => {
    updateQuery(historyQuery);
    onHistorySelect?.(historyQuery);
  };

  return (
    <div className="velvet-quick-search__overlay" onMouseDown={onClose}>
      <section aria-label="Quick Search" className="velvet-quick-search" onMouseDown={(event) => event.stopPropagation()} role="dialog">
        <VelvetInput
          aria-activedescendant={filteredResults[index] ? `${listId}-${filteredResults[index].id}` : undefined}
          aria-controls={listId}
          ariaLabel={placeholder}
          autoFocus
          data-quick-search-matching="aliases"
          onChange={(event) => updateQuery(event.currentTarget.value)}
          onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
            if (event.key === 'Escape') onClose();
            if (event.key === 'ArrowDown') { event.preventDefault(); setIndex((value) => Math.min(filteredResults.length - 1, value + 1)); }
            if (event.key === 'ArrowUp') { event.preventDefault(); setIndex((value) => Math.max(0, value - 1)); }
            if (event.key === 'Home') { event.preventDefault(); setIndex(0); }
            if (event.key === 'End') { event.preventDefault(); setIndex(Math.max(0, filteredResults.length - 1)); }
            if (event.key === 'Enter') { event.preventDefault(); selectCurrent(); }
          }}
          placeholder={placeholder}
          value={resolvedQuery}
        />
        <div aria-label="Quick Search categories" className="velvet-quick-search__tabs" role="tablist">
          {tabs.map((tab) => (
            <button aria-label={tab.label} aria-selected={currentTab === tab.id} className="velvet-quick-search__tab" key={tab.id} onClick={() => onTabChange?.(tab.id)} role="tab" type="button">
              {tab.label}
            </button>
          ))}
        </div>
        {normalizedQuery.length === 0 && visibleHistory.length > 0 ? (
          <div aria-label="Recent searches" className="velvet-quick-search__history" data-quick-search-history="true">
            <span className="velvet-quick-search__section-label">Recent searches</span>
            <div className="velvet-quick-search__history-list">
              {visibleHistory.map((historyQuery) => <button className="velvet-quick-search__history-item" data-quick-search-history-item="true" key={historyQuery} onClick={() => selectHistory(historyQuery)} type="button">{historyQuery}</button>)}
            </div>
          </div>
        ) : null}
        <div className="velvet-quick-search__list" id={listId} role="listbox">
          {filteredResults.length === 0 ? <VelvetState kind="empty" title={normalizedQuery.length === 0 ? 'No results' : 'No matches'} /> : null}
          {filteredResults.map((result, resultIndex) => (
            <button
              aria-selected={resultIndex === index}
              className="velvet-quick-search__item"
              disabled={result.disabled}
              id={`${listId}-${result.id}`}
              key={result.id}
              onClick={() => { if (!result.disabled) onSelect(result.id, result.kind); }}
              role="option"
              type="button"
            >
              <span><strong>{result.label}</strong>{result.description ? <small>{result.description}</small> : null}</span>
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}

function normalizeQuickSearchValue(value: string): string {
  return value.normalize('NFKC').toLocaleLowerCase().trim();
}

function matchesQuickSearchResult(result: VelvetQuickSearchResult, normalizedQuery: string): boolean {
  if (!normalizedQuery) return true;
  return [result.label, result.description ?? '', ...(result.searchAliases ?? [])]
    .some((value) => normalizeQuickSearchValue(value).includes(normalizedQuery));
}

function dedupeHistory(history: readonly string[], historyLimit: number): readonly string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of history) {
    const normalized = normalizeQuickSearchValue(value);
    if (!normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    result.push(value);
    if (result.length >= Math.max(0, historyLimit)) break;
  }
  return result;
}
