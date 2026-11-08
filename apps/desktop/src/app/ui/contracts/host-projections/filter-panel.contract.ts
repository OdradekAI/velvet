import type { VelvetFilterPanelProps } from '../../host-projections/filter-panel.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetFilterPanelContract = defineComponentContract<
  VelvetFilterPanelProps,
  'open' | 'closed' | 'filtered' | 'empty' | 'applied',
  'change' | 'apply' | 'reset' | 'save' | 'close',
  'region-role' | 'facet-groups' | 'rule-values' | 'operator-values' | 'result-count' | 'selection' | 'reset-callback' | 'apply-callback' | 'save-callback' | 'focus'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'filter-panel',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['open', 'title', 'facets', 'rules', 'resultCount', 'onRuleChange', 'onApply', 'onReset', 'onSave', 'onClose'],
  slots: ['facets', 'facet.options', 'actions'],
  states: ['open', 'closed', 'filtered', 'empty', 'applied'],
  actions: ['change', 'apply', 'reset', 'save', 'close'],
  browserActions: ['change', 'apply', 'reset', 'save', 'close'],
  observables: ['region-role', 'facet-groups', 'rule-values', 'operator-values', 'result-count', 'selection', 'reset-callback', 'apply-callback', 'save-callback', 'focus'],
  stories: ['default', 'multi-select', 'reset-save', 'keyboard-close', 'closed'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/filter-panel.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Host owns query state and the component reports rule changes.' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/host-projections/filter-panel.stories.tsx', claim: 'Facet rule changes, reset/save and close scenarios.' },
    { kind: 'observed', reference: 'apps/desktop/tools/qualify-ui-host-projections.mjs', claim: 'Browser qualification for text rules, reset/save and close.' },
  ],
  visual: { profile: 'dense-panel', surfaces: { root: 'panel', interactive: 'active', selected: 'active' }, radius: 'md', density: 'compact', geometry: ['chip:24px', 'panel-radius:6px'] },
  behavior: { stateOwner: 'host', focus: 'pointer', selection: 'host', actionLifecycle: [{ action: 'change', transitions: ['requested', 'committed'] }, { action: 'apply', transitions: ['requested', 'committed', 'failed'] }, { action: 'reset', transitions: ['requested', 'committed'] }, { action: 'save', transitions: ['requested', 'committed', 'failed'] }, { action: 'close', transitions: ['requested', 'committed', 'cancelled'] }] },
} satisfies ComponentContract<VelvetFilterPanelProps, 'open' | 'closed' | 'filtered' | 'empty' | 'applied', 'change' | 'apply' | 'reset' | 'save' | 'close', 'region-role' | 'facet-groups' | 'rule-values' | 'operator-values' | 'result-count' | 'selection' | 'reset-callback' | 'apply-callback' | 'save-callback' | 'focus'>);
