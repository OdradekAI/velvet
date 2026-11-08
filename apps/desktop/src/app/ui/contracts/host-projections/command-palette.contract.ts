import type { VelvetCommandPaletteProps } from '../../host-projections/command-palette.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetCommandPaletteContract = defineComponentContract<
  VelvetCommandPaletteProps,
  'open' | 'empty' | 'selected' | 'disabled',
  'query' | 'scope-change' | 'select' | 'navigate' | 'close',
  'dialog-role' | 'listbox-role' | 'focus-entry' | 'active-descendant' | 'scope' | 'query' | 'query-change-callback' | 'selection'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'command-palette',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['open', 'commands', 'scope', 'scopeOptions', 'onScopeChange', 'query', 'onQueryChange', 'onSelect', 'onClose', 'restoreFocusRef'],
  slots: ['commands', 'scope-options'],
  states: ['open', 'empty', 'selected', 'disabled'],
  actions: ['query', 'scope-change', 'select', 'navigate', 'close'],
  browserActions: ['query', 'scope-change', 'select', 'navigate', 'close'],
  observables: ['dialog-role', 'listbox-role', 'focus-entry', 'active-descendant', 'scope', 'query', 'query-change-callback', 'selection'],
  stories: ['open', 'empty', 'scope', 'controlled-query'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/command-palette.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
  visual: { profile: 'overlay', surfaces: { root: 'overlay', interactive: 'active', selected: 'active' }, radius: 'md', density: 'compact', geometry: ['surface:600px', 'row:34px', 'scope-chip:24px'] },
  behavior: { stateOwner: 'host', focus: 'restore', selection: 'host', actionLifecycle: [{ action: 'query', transitions: ['requested', 'committed'] }, { action: 'scope-change', transitions: ['requested', 'committed'] }, { action: 'select', transitions: ['requested', 'committed', 'failed'] }, { action: 'navigate', transitions: ['requested', 'committed'] }, { action: 'close', transitions: ['requested', 'committed', 'cancelled'] }] },
} satisfies ComponentContract<VelvetCommandPaletteProps, 'open' | 'empty' | 'selected' | 'disabled', 'query' | 'scope-change' | 'select' | 'navigate' | 'close', 'dialog-role' | 'listbox-role' | 'focus-entry' | 'active-descendant' | 'scope' | 'query' | 'query-change-callback' | 'selection'>);
