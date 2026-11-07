import type { VelvetSelectProps } from '../../primitives/select.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetSelectContract = defineComponentContract<
  VelvetSelectProps,
  'default' | 'focus' | 'disabled',
  'change',
  'semantic-dom' | 'focus' | 'value'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'select',
  layer: 'primitives',
  qualification: 'both',
  owner: 'component',
  props: ['value', 'defaultValue', 'disabled', 'busy', 'onChange', 'children'],
  slots: ['options'],
  states: ['default', 'focus', 'disabled'],
  actions: ['change'],
  browserActions: ['change'],
  observables: ['semantic-dom', 'focus', 'value'],
  stories: ['default', 'disabled'],
  documentation: 'docs/engineering/ui-kit/components/primitives/select.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetSelectProps, 'default' | 'focus' | 'disabled', 'change', 'semantic-dom' | 'focus' | 'value'>);
