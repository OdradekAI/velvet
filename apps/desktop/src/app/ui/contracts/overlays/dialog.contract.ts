import type { VelvetDialogProps } from '../../overlays/dialog.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetDialogContract = defineComponentContract<
  VelvetDialogProps,
  'closed' | 'open' | 'warning' | 'danger' | 'busy',
  'open' | 'close' | 'submit',
  'dialog-role' | 'focus-trap' | 'focus-restore' | 'action-callback'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'dialog',
  layer: 'overlays',
  qualification: 'browser',
  owner: 'host',
  props: ['open', 'title', 'description', 'actions', 'onOpenChange', 'restoreFocusRef'],
  slots: ['children', 'actions'],
  states: ['closed', 'open', 'warning', 'danger', 'busy'],
  actions: ['open', 'close', 'submit'],
  observables: ['dialog-role', 'focus-trap', 'focus-restore', 'action-callback'],
  stories: ['closed', 'open'],
  documentation: 'docs/engineering/ui-kit/components/overlays/dialog.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetDialogProps, 'closed' | 'open' | 'warning' | 'danger' | 'busy', 'open' | 'close' | 'submit', 'dialog-role' | 'focus-trap' | 'focus-restore' | 'action-callback'>);
