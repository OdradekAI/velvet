import type { VelvetCheckboxProps } from '../../primitives/checkbox.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetCheckboxContract = defineComponentContract<
  VelvetCheckboxProps,
  'checked' | 'unchecked' | 'disabled' | 'focus',
  'toggle',
  'semantic-dom' | 'focus' | 'checked'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'checkbox',
  layer: 'primitives',
  owner: 'host',
  props: ['checked', 'label', 'disabled', 'onCheckedChange'],
  slots: [],
  states: ['checked', 'unchecked', 'disabled', 'focus'],
  actions: ['toggle'],
  observables: ['semantic-dom', 'focus', 'checked'],
  stories: ['checked', 'disabled'],
  documentation: 'docs/engineering/ui-kit/components/primitives/checkbox.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetCheckboxProps, 'checked' | 'unchecked' | 'disabled' | 'focus', 'toggle', 'semantic-dom' | 'focus' | 'checked'>);
