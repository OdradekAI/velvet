import type { VelvetStateProps } from '../../primitives/state.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetStateContract = defineComponentContract<
  VelvetStateProps,
  'loading' | 'empty' | 'error',
  'retry' | 'action',
  'status' | 'alert' | 'retry-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'state',
  layer: 'primitives',
  owner: 'host',
  props: ['kind', 'title', 'description', 'retryLabel', 'onRetry', 'action'],
  slots: ['action'],
  states: ['loading', 'empty', 'error'],
  actions: ['retry', 'action'],
  observables: ['status', 'alert', 'retry-callback'],
  stories: ['loading', 'empty', 'error-retry'],
  documentation: 'docs/engineering/ui-kit/components/primitives/state.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetStateProps, 'loading' | 'empty' | 'error', 'retry' | 'action', 'status' | 'alert' | 'retry-callback'>);
