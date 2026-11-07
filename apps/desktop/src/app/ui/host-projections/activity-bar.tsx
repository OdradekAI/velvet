import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetActivityBarItem {
  readonly id: string;
  readonly label: string;
  readonly icon: ReactNode;
  readonly badge?: string | number;
  readonly active?: boolean;
  readonly disabled?: boolean;
}

export interface VelvetActivityBarProps extends UiCommonProps {
  readonly items: readonly VelvetActivityBarItem[];
  readonly onSelect: (id: string) => void;
  readonly className?: string;
}

export function VelvetActivityBar({ items, onSelect, className = '', ...common }: VelvetActivityBarProps) {
  const activeIndex = Math.max(0, items.findIndex((item) => item.active));
  const move = (index: number, direction: 1 | -1): void => {
    for (let offset = 1; offset <= items.length; offset += 1) {
      const candidate = items[(index + offset * direction + items.length) % items.length];
      if (candidate && !candidate.disabled) {
        onSelect(candidate.id);
        return;
      }
    }
  };
  return (
    <nav {...uiDataProps(common)} aria-label={common.ariaLabel ?? 'Workspaces'} className={`velvet-activity-bar ${className}`.trim()}>
      {items.map((item, index) => (
        <button
          aria-current={item.active ? 'page' : undefined}
          aria-label={item.label}
          aria-pressed={item.active}
          className="velvet-activity-bar__item"
          disabled={item.disabled}
          key={item.id}
          onClick={() => onSelect(item.id)}
          onKeyDown={(event) => {
            if (event.key === 'ArrowDown' || event.key === 'ArrowRight') {
              event.preventDefault();
              move(index, 1);
            } else if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') {
              event.preventDefault();
              move(index, -1);
            } else if (event.key === 'Home') {
              event.preventDefault();
              const first = items.find((candidate) => !candidate.disabled);
              if (first) onSelect(first.id);
            } else if (event.key === 'End') {
              event.preventDefault();
              const last = [...items].reverse().find((candidate) => !candidate.disabled);
              if (last) onSelect(last.id);
            }
          }}
          tabIndex={index === activeIndex ? 0 : -1}
          type="button"
        >
          <span aria-hidden="true">{item.icon}</span>
          {item.badge !== undefined ? <span className="velvet-activity-bar__badge">{item.badge}</span> : null}
        </button>
      ))}
    </nav>
  );
}
