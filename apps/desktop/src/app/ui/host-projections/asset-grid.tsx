import { useRef } from 'react';
import type { HTMLAttributes } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetAssetGridItem {
  readonly id: string;
  readonly label: string;
  readonly secondary?: string;
  readonly thumbnailUrl?: string;
  readonly selected?: boolean;
  readonly disabled?: boolean;
}

export interface VelvetAssetGridProps extends UiCommonProps {
  readonly items: readonly VelvetAssetGridItem[];
  readonly onSelect: (id: string, additive: boolean) => void;
  readonly onOpen?: (id: string) => void;
  readonly columns?: number;
  readonly className?: string;
}

export function VelvetAssetGrid({
  items,
  onSelect,
  onOpen,
  columns = 4,
  className = '',
  ...common
}: VelvetAssetGridProps) {
  const current = Math.max(0, items.findIndex((item) => item.selected));
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const move = (index: number, delta: number): void => {
    const next = Math.max(0, Math.min(items.length - 1, index + delta));
    const item = items[next];
    if (item && !item.disabled) {
      onSelect(item.id, false);
      itemRefs.current[next]?.focus();
    }
  };
  return (
    <div
      {...uiDataProps(common)}
      aria-label={common.ariaLabel ?? 'Assets'}
      className={`velvet-asset-grid ${className}`.trim()}
      role="grid"
      style={{ '--velvet-grid-columns': columns } as HTMLAttributes<HTMLDivElement>['style']}
    >
      {items.map((item, index) => (
        <button
          aria-label={item.label}
          aria-selected={item.selected}
          className="velvet-asset-grid__item"
          disabled={item.disabled}
          key={item.id}
          ref={(element) => { itemRefs.current[index] = element; }}
          onClick={(event) => onSelect(item.id, event.metaKey || event.ctrlKey)}
          onDoubleClick={() => onOpen?.(item.id)}
          onKeyDown={(event) => {
            if (event.key === 'ArrowRight') { event.preventDefault(); move(index, 1); }
            if (event.key === 'ArrowLeft') { event.preventDefault(); move(index, -1); }
            if (event.key === 'ArrowDown') { event.preventDefault(); move(index, columns); }
            if (event.key === 'ArrowUp') { event.preventDefault(); move(index, -columns); }
            if (event.key === 'Enter') { event.preventDefault(); onOpen?.(item.id); }
          }}
          role="gridcell"
          tabIndex={index === current ? 0 : -1}
          type="button"
        >
          {item.thumbnailUrl ? <img alt="" src={item.thumbnailUrl} /> : <span aria-hidden="true" className="velvet-asset-grid__placeholder" />}
          <span className="velvet-asset-grid__label">{item.label}</span>
          {item.secondary ? <span className="velvet-asset-grid__secondary">{item.secondary}</span> : null}
        </button>
      ))}
    </div>
  );
}
