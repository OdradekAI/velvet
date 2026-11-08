import type { VelvetInspectorProps } from '../../host-projections/inspector.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetInspectorContract = defineComponentContract<
  VelvetInspectorProps,
  'empty' | 'single' | 'multiple' | 'pinned' | 'editing' | 'saving' | 'saved' | 'error' | 'cancelled' | 'grouped' | 'palette',
  'pin' | 'open' | 'edit' | 'save' | 'retry',
  'inspector-role' | 'selection-mode' | 'metadata' | 'field-groups' | 'palette' | 'pin-state' | 'open-callback' | 'edit-field' | 'save-state' | 'validation'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'inspector',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['title', 'selectedCount', 'preview', 'fields', 'sections', 'palette', 'emptyMessage', 'onFieldChange', 'onSave', 'onRetry', 'saveState', 'saveError', 'pinned', 'onPin', 'onOpen', 'children'],
  slots: ['preview', 'metadata', 'sections', 'palette', 'actions', 'children'],
  states: ['empty', 'single', 'multiple', 'pinned', 'editing', 'saving', 'saved', 'error', 'cancelled', 'grouped', 'palette'],
  actions: ['pin', 'open', 'edit', 'save', 'retry'],
  browserActions: ['pin', 'open', 'edit', 'save'],
  observables: ['inspector-role', 'selection-mode', 'metadata', 'field-groups', 'palette', 'pin-state', 'open-callback', 'edit-field', 'save-state', 'validation'],
  stories: ['empty', 'single', 'multiple', 'grouped-palette', 'edit-success', 'edit-validation-error', 'edit-failure-retry'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/inspector.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Selection and Inspector ownership remain host-owned projections' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Inspector selection modes, editable fields, save state and retry behavior' },
  ],
  visual: { profile: 'dense-panel', surfaces: { root: 'panel', interactive: 'raised', selected: 'active' }, radius: 'sm', density: 'compact', geometry: ['column:250px', 'control:28px', 'control-radius:16px'] },
  behavior: { stateOwner: 'host', focus: 'pointer', selection: 'host', actionLifecycle: [{ action: 'pin', transitions: ['requested', 'committed'] }, { action: 'open', transitions: ['requested', 'committed'] }, { action: 'edit', transitions: ['requested', 'committed'] }, { action: 'save', transitions: ['requested', 'committed', 'cancelled', 'failed', 'retrying'] }, { action: 'retry', transitions: ['requested', 'committed', 'failed'] }] },
} satisfies ComponentContract<VelvetInspectorProps, 'empty' | 'single' | 'multiple' | 'pinned' | 'editing' | 'saving' | 'saved' | 'error' | 'cancelled' | 'grouped' | 'palette', 'pin' | 'open' | 'edit' | 'save' | 'retry', 'inspector-role' | 'selection-mode' | 'metadata' | 'field-groups' | 'palette' | 'pin-state' | 'open-callback' | 'edit-field' | 'save-state' | 'validation'>);
