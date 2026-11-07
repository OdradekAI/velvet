import type { VelvetActivityBarProps } from '../../host-projections/activity-bar.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetActivityBarContract = defineComponentContract<
  VelvetActivityBarProps,
  'selected' | 'disabled' | 'focus' | 'badge',
  'select' | 'navigate',
  'navigation-role' | 'selection' | 'roving-focus'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'activity-bar',
  layer: 'host-projections',
  owner: 'host',
  props: ['items', 'onSelect'],
  slots: ['items'],
  states: ['selected', 'disabled', 'focus', 'badge'],
  actions: ['select', 'navigate'],
  observables: ['navigation-role', 'selection', 'roving-focus'],
  stories: ['default', 'badges'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/activity-bar.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetActivityBarProps, 'selected' | 'disabled' | 'focus' | 'badge', 'select' | 'navigate', 'navigation-role' | 'selection' | 'roving-focus'>);
