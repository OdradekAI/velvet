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
  observables: ['grid-role', 'grid-metrics', 'selection', 'selection-modifier', 'focus', 'open-callback', 'layout', 'drag-and-drop', 'pinned', 'metadata'],
  stories: ['default', 'selected', 'grid-semantics', 'layout-modes', 'selection-modifiers', 'zoom-layout'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/asset-grid.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'docs/engineering/ui-kit-eagle-directive-evidence.md', claim: 'Eagle ng-grid-layout layout, lazy loading, selection, pinned and drag/drop observations' },
    { kind: 'observed', reference: 'docs/engineering/ui-kit-eagle-populated-evidence.md', claim: 'Populated Eagle grid reflow, click/Shift selection, filter and search observations' },
  ],
} satisfies ComponentContract<VelvetAssetGridProps, 'default' | 'selected' | 'disabled' | 'empty' | 'layout' | 'dragging' | 'pinned', 'select' | 'open' | 'navigate' | 'drag' | 'drop', 'grid-role' | 'grid-metrics' | 'selection' | 'selection-modifier' | 'focus' | 'open-callback' | 'layout' | 'drag-and-drop' | 'pinned' | 'metadata'>);
