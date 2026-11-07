import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetDataTableColumn<Row> {
  readonly id: string;
  readonly label: string;
  readonly render: (row: Row) => ReactNode;
  readonly sortable?: boolean;
}

export interface VelvetDataTableProps<Row> extends UiCommonProps {
  readonly rows: readonly Row[];
  readonly columns: readonly VelvetDataTableColumn<Row>[];
  readonly rowId: (row: Row) => string;
  readonly selectedId?: string;
  readonly onSelect?: (id: string) => void;
  readonly onSort?: (columnId: string) => void;
  readonly className?: string;
}

export function VelvetDataTable<Row>({
  rows,
  columns,
  rowId,
  selectedId,
  onSelect,
  onSort,
  className = '',
  ...common
}: VelvetDataTableProps<Row>) {
  return (
    <div {...uiDataProps(common)} className={`velvet-data-table ${className}`.trim()} role="region">
      <table aria-label={common.ariaLabel ?? 'Data table'}>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.id} scope="col">
                {column.sortable ? <button className="velvet-data-table__sort" onClick={() => onSort?.(column.id)} type="button">{column.label}</button> : column.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
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
