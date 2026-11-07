import type { VelvetContextMenuProps } from '../../overlays/context-menu.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetContextMenuContract = defineComponentContract<
  VelvetContextMenuProps,
  'closed' | 'open' | 'disabled' | 'checked',
  'open' | 'select' | 'dismiss',
  'menu-role' | 'checked-state' | 'focus' | 'dismissal'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'context-menu',
  layer: 'overlays',
  qualification: 'browser',
  owner: 'host',
  props: ['actions', 'children'],
  slots: ['trigger', 'actions'],
  states: ['closed', 'open', 'disabled', 'checked'],
  actions: ['open', 'select', 'dismiss'],
  observables: ['menu-role', 'checked-state', 'focus', 'dismissal'],
  stories: ['default', 'checked'],
  documentation: 'docs/engineering/ui-kit/components/overlays/context-menu.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetContextMenuProps, 'closed' | 'open' | 'disabled' | 'checked', 'open' | 'select' | 'dismiss', 'menu-role' | 'checked-state' | 'focus' | 'dismissal'>);
