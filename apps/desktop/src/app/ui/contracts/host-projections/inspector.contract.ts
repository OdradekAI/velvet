import type { VelvetInspectorProps } from '../../host-projections/inspector.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetInspectorContract = defineComponentContract<
  VelvetInspectorProps,
  'empty' | 'single' | 'multiple' | 'pinned' | 'editing' | 'saving' | 'saved' | 'error' | 'cancelled',
  'pin' | 'open' | 'edit' | 'save' | 'retry',
  'inspector-role' | 'selection-mode' | 'metadata' | 'pin-state' | 'open-callback' | 'edit-field' | 'save-state' | 'validation'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'inspector',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['title', 'selectedCount', 'preview', 'fields', 'emptyMessage', 'onFieldChange', 'onSave', 'onRetry', 'saveState', 'saveError', 'pinned', 'onPin', 'onOpen', 'children'],
  slots: ['preview', 'metadata', 'actions', 'children'],
  states: ['empty', 'single', 'multiple', 'pinned', 'editing', 'saving', 'saved', 'error', 'cancelled'],
  actions: ['pin', 'open', 'edit', 'save', 'retry'],
  observables: ['inspector-role', 'selection-mode', 'metadata', 'pin-state', 'open-callback', 'edit-field', 'save-state', 'validation'],
  stories: ['empty', 'single', 'multiple', 'edit-success', 'edit-validation-error', 'edit-failure-retry'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/inspector.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Selection and Inspector ownership remain host-owned projections' },
    { kind: 'observed', reference: 'docs/engineering/ui-kit-eagle-directive-evidence.md', claim: 'Eagle Inspector derives different fields for zero, one and many selected items' },
  ],
} satisfies ComponentContract<VelvetInspectorProps, 'empty' | 'single' | 'multiple' | 'pinned' | 'editing' | 'saving' | 'saved' | 'error' | 'cancelled', 'pin' | 'open' | 'edit' | 'save' | 'retry', 'inspector-role' | 'selection-mode' | 'metadata' | 'pin-state' | 'open-callback' | 'edit-field' | 'save-state' | 'validation'>);
