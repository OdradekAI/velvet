import type { VelvetTextProps } from '../../primitives/text.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetTextContract = defineComponentContract<
  VelvetTextProps,
  'default' | 'tone',
  'none',
  'semantic-dom' | 'accessible-name'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'text',
  layer: 'primitives',
  owner: 'component',
  props: ['as', 'tone', 'children', 'className', 'ariaLabel'],
  slots: ['children'],
  states: ['default', 'tone'],
  actions: [],
  observables: ['semantic-dom', 'accessible-name'],
  stories: ['default', 'tones'],
  documentation: 'docs/engineering/ui-kit/components/primitives/text.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetTextProps, 'default' | 'tone', 'none', 'semantic-dom' | 'accessible-name'>);
