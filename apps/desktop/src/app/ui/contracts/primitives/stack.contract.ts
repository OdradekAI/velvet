import type { VelvetStackProps } from '../../primitives/stack.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetStackContract = defineComponentContract<
  VelvetStackProps,
  'default' | 'direction' | 'gap',
  'none',
  'child-preservation' | 'layout-class'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'stack',
  layer: 'primitives',
  qualification: 'both',
  owner: 'component',
  props: ['children', 'direction', 'gap', 'className'],
  slots: ['children'],
  states: ['default', 'direction', 'gap'],
  actions: [],
  observables: ['child-preservation', 'layout-class'],
  stories: ['default', 'directions'],
  documentation: 'docs/engineering/ui-kit/components/primitives/stack.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetStackProps, 'default' | 'direction' | 'gap', 'none', 'child-preservation' | 'layout-class'>);
