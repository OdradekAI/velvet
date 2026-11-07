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
  const selectRelative = (start: number, direction: 1 | -1): void => {
    if (tabs.length === 0) return;
    for (let offset = 1; offset <= tabs.length; offset += 1) {
      const candidate = tabs[(start + offset * direction + tabs.length) % tabs.length];
      if (candidate && !candidate.disabled) {
        onSelect(candidate.id);
        return;
      }
    }
  };

  return (
    <div {...uiDataProps(common)} className={`velvet-tabs ${className}`.trim()}>
      <div aria-label={common.ariaLabel ?? 'Tabs'} className="velvet-tabs__list" role="tablist">
        {tabs.map((tab, index) => {
          const selected = tab.id === selectedId;
          return (
            <button
              aria-controls={`velvet-tab-panel-${tab.id}`}
              aria-selected={selected}
              className="velvet-tabs__tab"
              disabled={tab.disabled}
              id={`velvet-tab-${tab.id}`}
              key={tab.id}
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
                  const first = tabs.find((candidate) => !candidate.disabled);
                  if (first) onSelect(first.id);
                } else if (event.key === 'End') {
                  event.preventDefault();
                  const last = [...tabs].reverse().find((candidate) => !candidate.disabled);
                  if (last) onSelect(last.id);
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
