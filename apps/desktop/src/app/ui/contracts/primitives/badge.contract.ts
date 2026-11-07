import type { VelvetBadgeProps } from '../../primitives/badge.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetBadgeContract = defineComponentContract<
  VelvetBadgeProps,
  'default' | 'tone',
  'none',
  'semantic-dom' | 'non-focusable'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'badge',
  layer: 'primitives',
  owner: 'component',
  props: ['children', 'tone', 'className'],
  slots: ['children'],
  states: ['default', 'tone'],
  actions: [],
  observables: ['semantic-dom', 'non-focusable'],
  stories: ['default', 'tones'],
  documentation: 'docs/engineering/ui-kit/components/primitives/badge.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetBadgeProps, 'default' | 'tone', 'none', 'semantic-dom' | 'non-focusable'>);
