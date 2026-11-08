import type { VelvetPreviewHostProps } from '../../host-projections/preview-host.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetPreviewHostContract = defineComponentContract<
  VelvetPreviewHostProps,
  'loading' | 'ready' | 'empty' | 'error' | 'disabled',
  'retry' | 'change-mode' | 'provider-action' | 'open-external' | 'close',
  'preview-region' | 'fallback-state' | 'state-announcement' | 'retry-callback' | 'preview-mode' | 'toolbar' | 'mode-change-callback' | 'provider-actions' | 'provider-action-callback' | 'external-open-callback' | 'close-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'preview-host',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['state', 'message', 'mode', 'toolbar', 'children', 'onRetry', 'onModeChange', 'providerActions', 'onProviderAction', 'onOpenExternal', 'onClose'],
  slots: ['children', 'toolbar', 'provider-actions'],
  states: ['loading', 'ready', 'empty', 'error', 'disabled'],
  actions: ['retry', 'change-mode', 'provider-action', 'open-external', 'close'],
  browserActions: ['retry', 'change-mode', 'provider-action', 'open-external', 'close'],
  observables: ['preview-region', 'fallback-state', 'state-announcement', 'retry-callback', 'preview-mode', 'toolbar', 'mode-change-callback', 'provider-actions', 'provider-action-callback', 'external-open-callback', 'close-callback'],
  asyncLifecycle: ['success', 'cancelled', 'timeout', 'failure', 'retry'],
  stories: ['ready', 'error', 'retry', 'toolbar-mode', 'provider-actions', 'main-window-lifecycle', 'async-success', 'async-cancelled', 'async-timeout', 'async-failure', 'async-retry'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/preview-host.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
    { kind: 'observed', reference: 'docs/research/eagle/eagle-analysis/report.md', claim: 'Main-window detail mode, preview loading/fallback and close lifecycle evidence' },
    { kind: 'observed', reference: 'docs/research/eagle/ui-kit/populated-evidence.md', claim: 'Populated selection changes the persistent Inspector and preview projection' },
  ],
  visual: { profile: 'preview', surfaces: { root: 'canvas', interactive: 'active', overlay: 'overlay' }, radius: 'sm', density: 'compact', geometry: ['zoom:5-800', 'toolbar:48px'] },
  behavior: { stateOwner: 'host', focus: 'restore', selection: 'host', actionLifecycle: [{ action: 'retry', transitions: ['requested', 'committed', 'failed', 'retrying'] }, { action: 'change-mode', transitions: ['requested', 'committed'] }, { action: 'provider-action', transitions: ['requested', 'committed', 'failed'] }, { action: 'open-external', transitions: ['requested', 'committed', 'failed'] }, { action: 'close', transitions: ['requested', 'committed', 'cancelled'] }] },
} satisfies ComponentContract<VelvetPreviewHostProps, 'loading' | 'ready' | 'empty' | 'error' | 'disabled', 'retry' | 'change-mode' | 'provider-action' | 'open-external' | 'close', 'preview-region' | 'fallback-state' | 'state-announcement' | 'retry-callback' | 'preview-mode' | 'toolbar' | 'mode-change-callback' | 'provider-actions' | 'provider-action-callback' | 'external-open-callback' | 'close-callback'>);
