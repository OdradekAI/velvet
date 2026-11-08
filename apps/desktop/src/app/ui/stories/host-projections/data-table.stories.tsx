import { useState } from 'react';
import type { UiKitStory } from '../../story-registry.js';
import { VelvetDataTable } from '../../host-projections/index.js';

let selectedRow = '';
let sortedColumn = '';
let sortedDirection = '';

export const dataTableStories: readonly UiKitStory[] = [
  {
    id: 'data-table/default',
    componentId: 'data-table',
    name: 'Default',
    description: 'Data table.',
    render: () => <VelvetDataTable rows={[{ id: 'a', name: 'A' }]} columns={[{ id: 'name', label: 'Name', render: (row) => row.name, sortable: true }]} onSelect={(id) => { selectedRow = id; }} onSort={(id, direction) => { sortedColumn = id; sortedDirection = direction; }} rowId={(row) => row.id} />,
    scenario: {
      covers: ['select', 'sort'],
      setup: () => { selectedRow = ''; sortedColumn = ''; sortedDirection = ''; },
      actions: [
        (runtime) => runtime.act(async () => {
          const row = runtime.root.querySelector<HTMLElement>('tbody tr');
          row?.focus();
          row?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }));
          runtime.host.record('table.select', 'a');
        }),
        (runtime) => runtime.act(async () => {
          runtime.root.querySelector<HTMLButtonElement>('.velvet-data-table__sort')?.click();
          runtime.host.record('table.sort', 'name');
        }),
      ],
      checkpoints: [
        { name: 'keyboard row selection', afterAction: 0, assert: () => { if (selectedRow !== 'a') throw new Error('DataTable Enter selection contract failed'); } },
        { name: 'sortable header', afterAction: 1, assert: () => { if (sortedColumn !== 'name' || sortedDirection !== 'ascending') throw new Error('DataTable sort contract failed'); } },
      ],
    },
  },
  { id: 'data-table/selected', componentId: 'data-table', name: 'Selected', description: 'Selected row.', render: () => <VelvetDataTable selectedId="a" rows={[{ id: 'a', name: 'A' }]} columns={[{ id: 'name', label: 'Name', render: (row) => row.name }]} rowId={(row) => row.id} /> },
  {
    id: 'data-table/sort-direction',
    componentId: 'data-table',
    name: 'Sort direction',
    description: 'Host-owned sort direction is exposed through aria-sort and toggles predictably.',
    render: () => <SortDirectionStory />,
    scenario: {
      covers: ['sort'],
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('.velvet-data-table__sort')?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('.velvet-data-table__sort')?.click(); }),
      ],
      checkpoints: [
        { name: 'ascending sort', afterAction: 0, assert: (runtime) => { if (runtime.root.querySelector('th')?.getAttribute('aria-sort') !== 'ascending') throw new Error('DataTable ascending sort contract failed'); } },
        { name: 'descending sort', afterAction: 1, assert: (runtime) => { if (runtime.root.querySelector('th')?.getAttribute('aria-sort') !== 'descending') throw new Error('DataTable descending sort contract failed'); } },
      ],
    },
  },
  { id: 'data-table/empty', componentId: 'data-table', name: 'Empty', description: 'Empty result status.', render: () => <VelvetDataTable emptyMessage="No matching assets" rows={[] as readonly { id: string; name: string }[]} columns={[{ id: 'name', label: 'Name', render: (row) => row.name }]} rowId={(row) => row.id} />, scenario: { actions: [], checkpoints: [{ name: 'empty status', assert: (runtime) => { if (runtime.root.querySelector('[role="status"]')?.textContent !== 'No matching assets') throw new Error('DataTable empty state contract failed'); } }] } },
];

function SortDirectionStory() {
  const [sort, setSort] = useState<{ columnId: string; direction: 'ascending' | 'descending' } | undefined>();
  return <VelvetDataTable rows={[{ id: 'a', name: 'A' }]} columns={[{ id: 'name', label: 'Name', render: (row) => row.name, sortable: true }]} onSort={(columnId, direction) => setSort({ columnId, direction })} rowId={(row) => row.id} sort={sort} />;
}
