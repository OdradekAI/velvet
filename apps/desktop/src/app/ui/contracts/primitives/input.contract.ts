import type { VelvetInputProps } from '../../primitives/input.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetInputContract = defineComponentContract<
  VelvetInputProps,
  'default' | 'focus' | 'disabled' | 'busy',
  'change',
  'semantic-dom' | 'focus' | 'value'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'input',
  layer: 'primitives',
  owner: 'component',
  props: ['value', 'defaultValue', 'placeholder', 'disabled', 'busy', 'onChange', 'onKeyDown'],
  slots: [],
  states: ['default', 'focus', 'disabled', 'busy'],
  actions: ['change'],
  observables: ['semantic-dom', 'focus', 'value'],
  stories: ['default', 'change', 'busy'],
  documentation: 'docs/engineering/ui-kit/components/primitives/input.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetInputProps, 'default' | 'focus' | 'disabled' | 'busy', 'change', 'semantic-dom' | 'focus' | 'value'>);
