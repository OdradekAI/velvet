import { useRef, useState } from 'react';
import type { ReactNode } from 'react';
import type { HTMLAttributes } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetAssetGridItem {
  readonly id: string;
  readonly label: string;
  readonly secondary?: string;
  /** Host-rendered metadata slot for tags, resolution, rating or extension details. */
  readonly metadata?: ReactNode;
  readonly thumbnailUrl?: string;
  readonly aspectRatio?: string;
  readonly selected?: boolean;
  readonly disabled?: boolean;
  readonly pinned?: boolean;
}

export type VelvetAssetGridLayout = 'grid' | 'square' | 'list' | 'justified';

export interface VelvetAssetGridProps extends UiCommonProps {
  readonly items: readonly VelvetAssetGridItem[];
  /** Reports the host selection mode; `range` is true for a Shift selection. */
  readonly onSelect: (id: string, additive: boolean, range: boolean) => void;
  readonly onOpen?: (id: string) => void;
  readonly onDragStart?: (id: string) => void;
  readonly onDrop?: (sourceId: string, targetId: string) => void;
  readonly columns?: number;
  readonly layout?: VelvetAssetGridLayout;
  readonly className?: string;
}

export function VelvetAssetGrid({
  items,
  onSelect,
  onOpen,
  onDragStart,
  onDrop,
  columns = 4,
  layout = 'grid',
  className = '',
  ...common
}: VelvetAssetGridProps) {
  const current = Math.max(0, items.findIndex((item) => item.selected));
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const effectiveColumns = layout === 'list' ? 1 : columns;
  const semanticColumns = layout === 'justified' ? undefined : effectiveColumns;
  const semanticRows = semanticColumns && items.length > 0 ? Math.ceil(items.length / semanticColumns) : undefined;
  const move = (index: number, delta: number): void => {
    const next = Math.max(0, Math.min(items.length - 1, index + delta));
    const item = items[next];
    if (item && !item.disabled) {
      onSelect(item.id, false, false);
      itemRefs.current[next]?.focus();
    }
  };
  return (
    <div
      {...uiDataProps(common)}
      aria-label={common.ariaLabel ?? 'Assets'}
      aria-colcount={semanticColumns}
      aria-multiselectable="true"
      aria-rowcount={semanticRows}
      className={`velvet-asset-grid velvet-asset-grid--${layout} ${className}`.trim()}
      role="grid"
      style={{ '--velvet-grid-columns': columns } as HTMLAttributes<HTMLDivElement>['style']}
    >
      {items.map((item, index) => (
        <button
          aria-label={item.label}
          aria-colindex={semanticColumns ? (index % semanticColumns) + 1 : undefined}
          aria-rowindex={semanticColumns ? Math.floor(index / semanticColumns) + 1 : undefined}
          aria-selected={item.selected ? 'true' : 'false'}
          className="velvet-asset-grid__item"
          data-dragging={draggingId === item.id ? 'true' : undefined}
          data-pinned={item.pinned ? 'true' : undefined}
          disabled={item.disabled}
          draggable={onDragStart !== undefined}
          key={item.id}
          ref={(element) => { itemRefs.current[index] = element; }}
          onClick={(event) => onSelect(item.id, event.metaKey || event.ctrlKey, event.shiftKey)}
          onDoubleClick={() => onOpen?.(item.id)}
          onDragOver={(event) => { if (onDrop) event.preventDefault(); }}
          onDragStart={(event) => {
            if (!onDragStart) return;
            event.dataTransfer?.setData('text/plain', item.id);
            setDraggingId(item.id);
            onDragStart(item.id);
          }}
          onDragEnd={() => setDraggingId(null)}
          onDrop={(event) => {
            if (!onDrop) return;
            event.preventDefault();
            const sourceId = event.dataTransfer?.getData('text/plain');
            if (sourceId) onDrop(sourceId, item.id);
          }}
          onKeyDown={(event) => {
            if (event.key === 'ArrowRight') { event.preventDefault(); move(index, 1); }
            if (event.key === 'ArrowLeft') { event.preventDefault(); move(index, -1); }
            if (event.key === 'ArrowDown') { event.preventDefault(); move(index, effectiveColumns); }
            if (event.key === 'ArrowUp') { event.preventDefault(); move(index, -effectiveColumns); }
            if (event.key === 'Enter') { event.preventDefault(); onOpen?.(item.id); }
          }}
          role="gridcell"
          tabIndex={index === current ? 0 : -1}
          type="button"
        >
          {item.thumbnailUrl ? <img alt="" src={item.thumbnailUrl} style={item.aspectRatio ? { aspectRatio: item.aspectRatio } : undefined} /> : <span aria-hidden="true" className="velvet-asset-grid__placeholder" style={item.aspectRatio ? { aspectRatio: item.aspectRatio } : undefined} />}
          <span className="velvet-asset-grid__label">{item.label}</span>
          {item.secondary ? <span className="velvet-asset-grid__secondary">{item.secondary}</span> : null}
          {item.metadata ? <span className="velvet-asset-grid__metadata">{item.metadata}</span> : null}
          {item.pinned ? <span aria-label="Pinned" className="velvet-asset-grid__pin-indicator" role="img">⌖</span> : null}
        </button>
      ))}
    </div>
  );
}
