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
  readonly disabled?: boolean;
}

export interface VelvetQuickSearchProps extends UiCommonProps {
  readonly open: boolean;
  readonly tabs: readonly VelvetQuickSearchTab[];
  /** Results are already scoped, filtered and ordered by the host adapter. */
  readonly results: readonly VelvetQuickSearchResult[];
  readonly activeTab?: VelvetQuickSearchTabId;
  readonly query?: string;
  /** History is already ordered, deduplicated and bounded by the host. */
  readonly history?: readonly string[];
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

  useEffect(() => setIndex(0), [currentTab, results]);
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
    const result = results[index];
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
          aria-activedescendant={results[index] ? `${listId}-${results[index].id}` : undefined}
          aria-controls={listId}
          ariaLabel={placeholder}
          autoFocus
          data-quick-search-results="host"
          onChange={(event) => updateQuery(event.currentTarget.value)}
          onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
            if (event.key === 'Escape') onClose();
            if (event.key === 'ArrowDown') { event.preventDefault(); setIndex((value) => Math.min(results.length - 1, value + 1)); }
            if (event.key === 'ArrowUp') { event.preventDefault(); setIndex((value) => Math.max(0, value - 1)); }
            if (event.key === 'Home') { event.preventDefault(); setIndex(0); }
            if (event.key === 'End') { event.preventDefault(); setIndex(Math.max(0, results.length - 1)); }
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
        {resolvedQuery.length === 0 && history.length > 0 ? (
          <div aria-label="Recent searches" className="velvet-quick-search__history" data-quick-search-history="true">
            <span className="velvet-quick-search__section-label">Recent searches</span>
            <div className="velvet-quick-search__history-list">
              {history.map((historyQuery, historyIndex) => <button className="velvet-quick-search__history-item" data-quick-search-history-item="true" key={`${historyQuery}-${historyIndex}`} onClick={() => selectHistory(historyQuery)} type="button">{historyQuery}</button>)}
            </div>
          </div>
        ) : null}
        <div className="velvet-quick-search__list" id={listId} role="listbox">
          {results.length === 0 ? <VelvetState kind="empty" title="No results" /> : null}
          {results.map((result, resultIndex) => (
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
