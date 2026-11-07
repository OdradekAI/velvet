import type { VelvetPreviewHostProps } from '../../host-projections/preview-host.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetPreviewHostContract = defineComponentContract<
  VelvetPreviewHostProps,
  'loading' | 'ready' | 'empty' | 'error' | 'disabled',
  'retry',
  'preview-region' | 'fallback-state' | 'retry-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'preview-host',
  layer: 'host-projections',
  owner: 'host',
  props: ['state', 'message', 'children', 'onRetry'],
  slots: ['children'],
  states: ['loading', 'ready', 'empty', 'error', 'disabled'],
  actions: ['retry'],
  observables: ['preview-region', 'fallback-state', 'retry-callback'],
  asyncLifecycle: ['success', 'cancelled', 'timeout', 'failure', 'retry'],
  stories: ['ready', 'error', 'retry', 'async-success', 'async-cancelled', 'async-timeout', 'async-failure', 'async-retry'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/preview-host.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetPreviewHostProps, 'loading' | 'ready' | 'empty' | 'error' | 'disabled', 'retry', 'preview-region' | 'fallback-state' | 'retry-callback'>);
