import type { VelvetPaneProps } from '../../host-projections/pane.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetPaneContract = defineComponentContract<
  VelvetPaneProps,
  'navigation' | 'context' | 'operations' | 'preview',
  'none',
  'region' | 'region-role' | 'heading' | 'children'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'pane',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['title', 'region', 'children'],
  slots: ['children'],
  states: ['navigation', 'context', 'operations', 'preview'],
  actions: [],
  observables: ['region', 'region-role', 'heading', 'children'],
  stories: ['navigation', 'context'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/pane.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetPaneProps, 'navigation' | 'context' | 'operations' | 'preview', 'none', 'region' | 'region-role' | 'heading' | 'children'>);
