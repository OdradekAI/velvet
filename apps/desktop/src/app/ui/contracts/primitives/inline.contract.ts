import type { VelvetInlineProps } from '../../primitives/inline.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetInlineContract = defineComponentContract<
  VelvetInlineProps,
  'row',
  'none',
  'child-preservation' | 'layout-class'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'inline',
  layer: 'primitives',
  owner: 'component',
  props: ['children', 'gap', 'className'],
  slots: ['children'],
  states: ['row'],
  actions: [],
  observables: ['child-preservation', 'layout-class'],
  stories: ['default'],
  documentation: 'docs/engineering/ui-kit/components/primitives/inline.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetInlineProps, 'row', 'none', 'child-preservation' | 'layout-class'>);
