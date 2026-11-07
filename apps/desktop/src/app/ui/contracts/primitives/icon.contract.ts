import type { VelvetIconProps } from '../../primitives/icon.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetIconContract = defineComponentContract<
  VelvetIconProps,
  'default' | 'decorative' | 'labelled',
  'none',
  'semantic-dom' | 'accessible-name'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'icon',
  layer: 'primitives',
  owner: 'component',
  props: ['children', 'label', 'className'],
  slots: ['children'],
  states: ['default', 'decorative', 'labelled'],
  actions: [],
  observables: ['semantic-dom', 'accessible-name'],
  stories: ['default', 'labelled'],
  documentation: 'docs/engineering/ui-kit/components/primitives/icon.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetIconProps, 'default' | 'decorative' | 'labelled', 'none', 'semantic-dom' | 'accessible-name'>);
