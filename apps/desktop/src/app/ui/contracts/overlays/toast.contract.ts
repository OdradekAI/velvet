import type { VelvetToastHostProps } from '../../overlays/toast.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetToastHostContract = defineComponentContract<
  VelvetToastHostProps,
  'info' | 'success' | 'warning' | 'error',
  'dismiss',
  'live-region' | 'alert' | 'dismiss-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'toast',
  layer: 'overlays',
  qualification: 'both',
  owner: 'host',
  props: ['items', 'onDismiss'],
  slots: ['action'],
  states: ['info', 'success', 'warning', 'error'],
  actions: ['dismiss'],
  browserActions: ['dismiss'],
  observables: ['live-region', 'alert', 'dismiss-callback'],
  stories: ['default', 'error'],
  documentation: 'docs/engineering/ui-kit/components/overlays/toast.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetToastHostProps, 'info' | 'success' | 'warning' | 'error', 'dismiss', 'live-region' | 'alert' | 'dismiss-callback'>);
