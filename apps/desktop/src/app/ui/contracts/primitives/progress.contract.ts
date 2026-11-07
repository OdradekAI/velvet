import type { VelvetProgressProps } from '../../primitives/progress.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetProgressContract = defineComponentContract<
  VelvetProgressProps,
  'determinate' | 'indeterminate' | 'success' | 'danger',
  'none',
  'semantic-dom' | 'value'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'progress',
  layer: 'primitives',
  qualification: 'both',
  owner: 'host',
  props: ['value', 'label', 'tone'],
  slots: [],
  states: ['determinate', 'indeterminate', 'success', 'danger'],
  actions: [],
  observables: ['semantic-dom', 'value'],
  stories: ['determinate', 'indeterminate'],
  documentation: 'docs/engineering/ui-kit/components/primitives/progress.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetProgressProps, 'determinate' | 'indeterminate' | 'success' | 'danger', 'none', 'semantic-dom' | 'value'>);
