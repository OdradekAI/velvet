import type { VelvetButtonProps } from '../../primitives/button.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetButtonContract = defineComponentContract<
  VelvetButtonProps,
  'default' | 'quiet' | 'danger' | 'disabled' | 'busy' | 'focus',
  'activate',
  'semantic-dom' | 'focus' | 'action-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'button',
  layer: 'primitives',
  owner: 'host',
  props: ['children', 'label', 'variant', 'disabled', 'busy', 'onClick'],
  slots: ['children'],
  states: ['default', 'quiet', 'danger', 'disabled', 'busy', 'focus'],
  actions: ['activate'],
  observables: ['semantic-dom', 'focus', 'action-callback'],
  stories: ['default', 'busy', 'disabled'],
  documentation: 'docs/engineering/ui-kit/components/primitives/button.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetButtonProps, 'default' | 'quiet' | 'danger' | 'disabled' | 'busy' | 'focus', 'activate', 'semantic-dom' | 'focus' | 'action-callback'>);
