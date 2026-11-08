import type { VelvetDataTableProps } from '../../host-projections/data-table.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetDataTableContract = defineComponentContract<
  VelvetDataTableProps<unknown>,
  'default' | 'selected' | 'sortable' | 'empty',
  'select' | 'sort',
  'table-role' | 'selection' | 'sort-callback' | 'sort-direction' | 'empty-state' | 'focus'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'data-table',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['rows', 'columns', 'selectedId', 'sort', 'onSelect', 'onSort', 'emptyMessage'],
  slots: ['cells'],
  states: ['default', 'selected', 'sortable', 'empty'],
  actions: ['select', 'sort'],
  browserActions: ['select', 'sort'],
  observables: ['table-role', 'selection', 'sort-callback', 'sort-direction', 'empty-state', 'focus'],
  stories: ['default', 'selected', 'sort-direction', 'empty'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/data-table.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
  visual: { profile: 'dense-panel', surfaces: { root: 'panel', interactive: 'raised', selected: 'active' }, radius: 'sm', density: 'compact', geometry: ['row:32px', 'header:32px'] },
  behavior: { stateOwner: 'host', focus: 'pointer', selection: 'host', actionLifecycle: [{ action: 'select', transitions: ['requested', 'committed'] }, { action: 'sort', transitions: ['requested', 'committed', 'failed'] }] },
} satisfies ComponentContract<VelvetDataTableProps<unknown>, 'default' | 'selected' | 'sortable' | 'empty', 'select' | 'sort', 'table-role' | 'selection' | 'sort-callback' | 'sort-direction' | 'empty-state' | 'focus'>);
