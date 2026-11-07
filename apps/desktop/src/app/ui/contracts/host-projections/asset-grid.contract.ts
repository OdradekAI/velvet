import type { VelvetAssetGridProps } from '../../host-projections/asset-grid.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetAssetGridContract = defineComponentContract<
  VelvetAssetGridProps,
  'default' | 'selected' | 'disabled' | 'empty' | 'layout' | 'dragging' | 'pinned',
  'select' | 'open' | 'navigate' | 'drag' | 'drop',
  'grid-role' | 'grid-metrics' | 'selection' | 'selection-modifier' | 'focus' | 'open-callback' | 'layout' | 'drag-and-drop' | 'pinned' | 'metadata'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'asset-grid',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['items', 'columns', 'layout', 'onSelect', 'onOpen', 'onDragStart', 'onDrop'],
  slots: ['items', 'items.metadata'],
  states: ['default', 'selected', 'disabled', 'empty', 'layout', 'dragging', 'pinned'],
  actions: ['select', 'open', 'navigate', 'drag', 'drop'],
  browserActions: ['select', 'open', 'navigate'],
  observables: ['grid-role', 'grid-metrics', 'selection', 'selection-modifier', 'focus', 'open-callback', 'layout', 'drag-and-drop', 'pinned', 'metadata'],
  stories: ['default', 'selected', 'grid-semantics', 'layout-modes', 'selection-modifiers', 'zoom-layout'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/asset-grid.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Grid layout, selection modifiers, pinned metadata and drag/drop behavior' },
    { kind: 'observed', reference: 'apps/desktop/tools/qualify-ui-host-projections.mjs', claim: 'Browser qualification for grid focus, range selection and row/column semantics' },
  ],
} satisfies ComponentContract<VelvetAssetGridProps, 'default' | 'selected' | 'disabled' | 'empty' | 'layout' | 'dragging' | 'pinned', 'select' | 'open' | 'navigate' | 'drag' | 'drop', 'grid-role' | 'grid-metrics' | 'selection' | 'selection-modifier' | 'focus' | 'open-callback' | 'layout' | 'drag-and-drop' | 'pinned' | 'metadata'>);
