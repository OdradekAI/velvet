type VelvetSeparatorProps = { orientation?: 'horizontal' | 'vertical' };
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetSeparatorContract = defineComponentContract<
  VelvetSeparatorProps,
  'horizontal' | 'vertical',
  'none',
  'semantic-dom'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'separator',
  layer: 'primitives',
  owner: 'component',
  props: ['orientation'],
  slots: [],
  states: ['horizontal', 'vertical'],
  actions: [],
  observables: ['semantic-dom'],
  stories: ['horizontal', 'vertical'],
  documentation: 'docs/engineering/ui-kit/components/primitives/separator.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetSeparatorProps, 'horizontal' | 'vertical', 'none', 'semantic-dom'>);
