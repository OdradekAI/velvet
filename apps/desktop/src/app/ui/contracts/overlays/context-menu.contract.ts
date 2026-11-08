import type { VelvetContextMenuProps } from '../../overlays/context-menu.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetContextMenuContract = defineComponentContract<
  VelvetContextMenuProps,
  'closed' | 'open' | 'disabled' | 'checked' | 'submenu',
  'open' | 'select' | 'dismiss' | 'navigate',
  'menu-role' | 'checked-state' | 'focus' | 'dismissal' | 'submenu'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'context-menu',
  layer: 'overlays',
  qualification: 'browser',
  owner: 'host',
  props: ['actions', 'children'],
  slots: ['trigger', 'actions', 'submenu.actions'],
  states: ['closed', 'open', 'disabled', 'checked', 'submenu'],
  actions: ['open', 'select', 'dismiss', 'navigate'],
  browserActions: ['open', 'select', 'dismiss', 'navigate'],
  observables: ['menu-role', 'checked-state', 'focus', 'dismissal', 'submenu'],
  stories: ['default', 'checked', 'submenu'],
  documentation: 'docs/engineering/ui-kit/components/overlays/context-menu.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
  visual: { profile: 'overlay', surfaces: { root: 'overlay', interactive: 'active', selected: 'active' }, radius: 'sm', density: 'compact', geometry: ['row:28px', 'surface-radius:4px'] },
  behavior: { stateOwner: 'host', focus: 'roving', selection: 'none', actionLifecycle: [{ action: 'open', transitions: ['requested', 'committed'] }, { action: 'select', transitions: ['requested', 'committed'] }, { action: 'dismiss', transitions: ['requested', 'committed', 'cancelled'] }, { action: 'navigate', transitions: ['requested', 'committed'] }] },
} satisfies ComponentContract<VelvetContextMenuProps, 'closed' | 'open' | 'disabled' | 'checked' | 'submenu', 'open' | 'select' | 'dismiss' | 'navigate', 'menu-role' | 'checked-state' | 'focus' | 'dismissal' | 'submenu'>);
