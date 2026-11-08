import type { VelvetFilterFacetMenuProps } from '../../host-projections/filter-facet-menu.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetFilterFacetMenuContract = defineComponentContract<
  VelvetFilterFacetMenuProps,
  'closed' | 'folder' | 'color' | 'included' | 'excluded' | 'selected',
  'query' | 'folder-rule' | 'folder-include' | 'folder-exclude' | 'color-select' | 'color-hex' | 'color-accuracy' | 'clear' | 'close',
  'menu-role' | 'folder-rule' | 'folder-option-state' | 'color-palette' | 'color-accuracy' | 'focus'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'filter-facet-menu',
  layer: 'host-projections',
  qualification: 'node',
  owner: 'host',
  props: ['open', 'kind', 'options', 'rule', 'swatches', 'selectedColor', 'graySelected', 'hexValue', 'accuracy', 'onQueryChange', 'onRuleChange', 'onOptionAction', 'onPresetChange', 'onHexChange', 'onAccuracyChange', 'onClear', 'onClose'],
  slots: ['folder.options', 'color.swatches', 'footer'],
  states: ['closed', 'folder', 'color', 'included', 'excluded', 'selected'],
  actions: ['query', 'folder-rule', 'folder-include', 'folder-exclude', 'color-select', 'color-hex', 'color-accuracy', 'clear', 'close'],
  observables: ['menu-role', 'folder-rule', 'folder-option-state', 'color-palette', 'color-accuracy', 'focus'],
  visual: { profile: 'overlay', surfaces: { root: 'overlay', interactive: 'active', selected: 'active' }, radius: 'md', density: 'compact', geometry: ['folder:360px', 'color:202px', 'header:40px', 'row:26px', 'swatch:20px', 'footer:40px'] },
  behavior: { stateOwner: 'host', focus: 'pointer', selection: 'host', actionLifecycle: [
    { action: 'query', transitions: ['requested', 'committed'] },
    { action: 'folder-rule', transitions: ['requested', 'committed'] },
    { action: 'folder-include', transitions: ['requested', 'committed'] },
    { action: 'folder-exclude', transitions: ['requested', 'committed'] },
    { action: 'color-select', transitions: ['requested', 'committed'] },
    { action: 'color-hex', transitions: ['requested', 'committed'] },
    { action: 'color-accuracy', transitions: ['requested', 'committed'] },
    { action: 'clear', transitions: ['requested', 'committed'] },
    { action: 'close', transitions: ['requested', 'committed', 'cancelled'] },
  ] },
  stories: ['folder-default', 'color-default', 'closed'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/filter-facet-menu.md',
  evidence: [
    { kind: 'observed', reference: 'docs/research/eagle/eagle-analysis/component-style-evidence.md', claim: 'Folder menu uses a 360px surface, 40px header, 26px options, OR/AND/EQUAL tabs and include/exclude actions.' },
    { kind: 'observed', reference: 'docs/research/eagle/eagle-analysis/component-style-evidence.md', claim: 'Color menu uses a 202px surface, 20px swatches, hex input and accuracy range.' },
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Filter execution and query ownership remain with the host.' },
  ],
} satisfies ComponentContract<
  VelvetFilterFacetMenuProps,
  'closed' | 'folder' | 'color' | 'included' | 'excluded' | 'selected',
  'query' | 'folder-rule' | 'folder-include' | 'folder-exclude' | 'color-select' | 'color-hex' | 'color-accuracy' | 'clear' | 'close',
  'menu-role' | 'folder-rule' | 'folder-option-state' | 'color-palette' | 'color-accuracy' | 'focus'
>);
