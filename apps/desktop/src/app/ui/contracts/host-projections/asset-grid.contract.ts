import type { VelvetAssetGridProps } from '../../host-projections/asset-grid.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetAssetGridContract = defineComponentContract<
  VelvetAssetGridProps,
  'default' | 'selected' | 'disabled' | 'empty',
  'select' | 'open' | 'navigate',
  'grid-role' | 'selection' | 'focus' | 'open-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'asset-grid',
  layer: 'host-projections',
  owner: 'host',
  props: ['items', 'columns', 'onSelect', 'onOpen'],
  slots: ['items'],
  states: ['default', 'selected', 'disabled', 'empty'],
  actions: ['select', 'open', 'navigate'],
  observables: ['grid-role', 'selection', 'focus', 'open-callback'],
  stories: ['default', 'selected'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/asset-grid.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetAssetGridProps, 'default' | 'selected' | 'disabled' | 'empty', 'select' | 'open' | 'navigate', 'grid-role' | 'selection' | 'focus' | 'open-callback'>);
