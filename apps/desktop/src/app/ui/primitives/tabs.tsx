import { useRef } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetTab {
  readonly id: string;
  readonly label: string;
  readonly disabled?: boolean;
  readonly badge?: string | number;
}

export interface VelvetTabsProps extends UiCommonProps {
  readonly tabs: readonly VelvetTab[];
  readonly selectedId: string;
  readonly onSelect: (id: string) => void;
  readonly className?: string;
}

export function VelvetTabs({
  tabs,
  selectedId,
  onSelect,
  className = '',
  ...common
}: VelvetTabsProps) {
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const selectAt = (index: number): void => {
    const tab = tabs[index];
    if (!tab || tab.disabled) return;
    onSelect(tab.id);
    tabRefs.current[index]?.focus();
  };
  const selectRelative = (start: number, direction: 1 | -1): void => {
    if (tabs.length === 0) return;
    for (let offset = 1; offset <= tabs.length; offset += 1) {
      const nextIndex = (start + offset * direction + tabs.length) % tabs.length;
      const candidate = tabs[nextIndex];
      if (candidate && !candidate.disabled) {
        selectAt(nextIndex);
        return;
      }
    }
  };

  return (
    <div {...uiDataProps(common)} className={`velvet-tabs ${className}`.trim()}>
      <div aria-label={common.ariaLabel ?? 'Tabs'} aria-orientation="horizontal" className="velvet-tabs__list" role="tablist">
        {tabs.map((tab, index) => {
          const selected = tab.id === selectedId;
          return (
            <button
              aria-selected={selected}
              className="velvet-tabs__tab"
              disabled={tab.disabled}
              id={`velvet-tab-${tab.id}`}
              key={tab.id}
              ref={(element) => { tabRefs.current[index] = element; }}
              onClick={() => onSelect(tab.id)}
              onKeyDown={(event) => {
                if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
                  event.preventDefault();
                  selectRelative(index, 1);
                } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
                  event.preventDefault();
                  selectRelative(index, -1);
                } else if (event.key === 'Home') {
                  event.preventDefault();
                  const firstIndex = tabs.findIndex((candidate) => !candidate.disabled);
                  if (firstIndex >= 0) selectAt(firstIndex);
                } else if (event.key === 'End') {
                  event.preventDefault();
                  let lastIndex = -1;
                  for (let candidateIndex = tabs.length - 1; candidateIndex >= 0; candidateIndex -= 1) {
                    if (!tabs[candidateIndex]?.disabled) {
                      lastIndex = candidateIndex;
                      break;
                    }
                  }
                  if (lastIndex >= 0) selectAt(lastIndex);
                }
              }}
              role="tab"
              tabIndex={selected ? 0 : -1}
              type="button"
            >
              {tab.label}
              {tab.badge !== undefined ? <span className="velvet-tabs__badge">{tab.badge}</span> : null}
            </button>
          );
        })}
      </div>
    </div>
  );
}
