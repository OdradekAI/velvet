import type { VelvetPanelProps } from '../../host-projections/panel.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetPanelContract = defineComponentContract<
  VelvetPanelProps,
  'panel' | 'raised' | 'overlay',
  'none',
  'panel-role' | 'heading' | 'surface' | 'children'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'panel',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['title', 'surface', 'children'],
  slots: ['children'],
  states: ['panel', 'raised', 'overlay'],
  actions: [],
  observables: ['panel-role', 'heading', 'surface', 'children'],
  stories: ['panel', 'raised'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/panel.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetPanelProps, 'panel' | 'raised' | 'overlay', 'none', 'panel-role' | 'heading' | 'surface' | 'children'>);
