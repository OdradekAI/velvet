import type { VelvetIconButtonProps } from '../../primitives/icon-button.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetIconButtonContract = defineComponentContract<
  VelvetIconButtonProps,
  'default' | 'disabled' | 'focus',
  'activate',
  'semantic-dom' | 'focus' | 'action-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'icon-button',
  layer: 'primitives',
  owner: 'host',
  props: ['icon', 'label', 'disabled', 'busy', 'onClick'],
  slots: ['icon'],
  states: ['default', 'disabled', 'focus'],
  actions: ['activate'],
  observables: ['semantic-dom', 'focus', 'action-callback'],
  stories: ['default', 'disabled'],
  documentation: 'docs/engineering/ui-kit/components/primitives/icon-button.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetIconButtonProps, 'default' | 'disabled' | 'focus', 'activate', 'semantic-dom' | 'focus' | 'action-callback'>);
