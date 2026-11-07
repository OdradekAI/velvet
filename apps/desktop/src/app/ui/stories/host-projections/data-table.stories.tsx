import type { UiKitStory } from '../../story-registry.js';
import { VelvetDataTable } from '../../host-projections/index.js';

let selectedRow = '';
let sortedColumn = '';

export const dataTableStories: readonly UiKitStory[] = [
  {
    id: 'data-table/default',
    componentId: 'data-table',
    name: 'Default',
    description: 'Data table.',
    render: () => <VelvetDataTable rows={[{ id: 'a', name: 'A' }]} columns={[{ id: 'name', label: 'Name', render: (row) => row.name, sortable: true }]} onSelect={(id) => { selectedRow = id; }} onSort={(id) => { sortedColumn = id; }} rowId={(row) => row.id} />,
    scenario: {
      setup: () => { selectedRow = ''; sortedColumn = ''; },
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
        { name: 'sortable header', afterAction: 1, assert: () => { if (sortedColumn !== 'name') throw new Error('DataTable sort contract failed'); } },
      ],
    },
  },
  { id: 'data-table/selected', componentId: 'data-table', name: 'Selected', description: 'Selected row.', render: () => <VelvetDataTable selectedId="a" rows={[{ id: 'a', name: 'A' }]} columns={[{ id: 'name', label: 'Name', render: (row) => row.name }]} rowId={(row) => row.id} /> },
];
