import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetDataTableColumn<Row> {
  readonly id: string;
  readonly label: string;
  readonly render: (row: Row) => ReactNode;
  readonly sortable?: boolean;
}

export type VelvetDataTableSortDirection = 'ascending' | 'descending';

export interface VelvetDataTableSort {
  readonly columnId: string;
  readonly direction: VelvetDataTableSortDirection;
}

export interface VelvetDataTableProps<Row> extends UiCommonProps {
  readonly rows: readonly Row[];
  readonly columns: readonly VelvetDataTableColumn<Row>[];
  readonly rowId: (row: Row) => string;
  readonly selectedId?: string;
  readonly onSelect?: (id: string) => void;
  readonly sort?: VelvetDataTableSort;
  readonly onSort?: (columnId: string, direction: VelvetDataTableSortDirection) => void;
  readonly emptyMessage?: string;
  readonly className?: string;
}

export function VelvetDataTable<Row>({
  rows,
  columns,
  rowId,
  selectedId,
  sort,
  onSelect,
  onSort,
  emptyMessage = 'No results',
  className = '',
  ...common
}: VelvetDataTableProps<Row>) {
  return (
    <div {...uiDataProps(common)} className={`velvet-data-table ${className}`.trim()} role="region">
      <table aria-label={common.ariaLabel ?? 'Data table'}>
        <thead>
          <tr>
            {columns.map((column) => (
              <th aria-sort={sort?.columnId === column.id ? sort.direction : column.sortable ? 'none' : undefined} key={column.id} scope="col">
                {column.sortable ? (
                  <button
                    aria-label={`Sort by ${column.label}`}
                    className="velvet-data-table__sort"
                    data-sort-direction={sort?.columnId === column.id ? sort.direction : 'none'}
                    onClick={() => onSort?.(column.id, sort?.columnId === column.id && sort.direction === 'ascending' ? 'descending' : 'ascending')}
                    type="button"
                  >
                    {column.label}
                  </button>
                ) : column.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td colSpan={Math.max(1, columns.length)} role="status">{emptyMessage}</td>
            </tr>
          ) : rows.map((row) => {
            const id = rowId(row);
            return (
              <tr
                aria-selected={selectedId === id}
                key={id}
                onClick={() => onSelect?.(id)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    onSelect?.(id);
                  }
                }}
                tabIndex={0}
              >
                {columns.map((column) => <td key={column.id}>{column.render(row)}</td>)}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
