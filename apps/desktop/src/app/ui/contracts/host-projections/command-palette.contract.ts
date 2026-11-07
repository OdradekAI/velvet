import type { VelvetCommandPaletteProps } from '../../host-projections/command-palette.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetCommandPaletteContract = defineComponentContract<
  VelvetCommandPaletteProps,
  'open' | 'empty' | 'selected' | 'disabled',
  'query' | 'select' | 'navigate' | 'close',
  'dialog-role' | 'listbox-role' | 'focus-entry' | 'selection'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'command-palette',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['open', 'commands', 'onSelect', 'onClose', 'restoreFocusRef'],
  slots: ['commands'],
  states: ['open', 'empty', 'selected', 'disabled'],
  actions: ['query', 'select', 'navigate', 'close'],
  observables: ['dialog-role', 'listbox-role', 'focus-entry', 'selection'],
  stories: ['open', 'empty'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/command-palette.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet host ownership and semantic behavior rules' },
    { kind: 'observed', reference: 'apps/desktop/src/app/ui/stories/velvet-ui.stories.tsx', claim: 'Existing compatibility story coverage' },
  ],
} satisfies ComponentContract<VelvetCommandPaletteProps, 'open' | 'empty' | 'selected' | 'disabled', 'query' | 'select' | 'navigate' | 'close', 'dialog-role' | 'listbox-role' | 'focus-entry' | 'selection'>);
