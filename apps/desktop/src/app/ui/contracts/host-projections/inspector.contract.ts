import type { VelvetInspectorProps } from '../../host-projections/inspector.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetInspectorContract = defineComponentContract<
  VelvetInspectorProps,
  'empty' | 'single' | 'multiple' | 'pinned' | 'editing' | 'saving' | 'saved' | 'error' | 'cancelled' | 'grouped' | 'palette' | 'preview-landscape' | 'preview-portrait' | 'no-preview',
  'pin' | 'open' | 'edit' | 'save' | 'retry' | 'toggle-section',
  'inspector-role' | 'selection-mode' | 'metadata' | 'field-groups' | 'palette' | 'pin-state' | 'open-callback' | 'edit-field' | 'save-state' | 'validation' | 'section-toggle' | 'selected-count' | 'preview-slot' | 'preview-geometry'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'inspector',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['title', 'selectedCount', 'preview', 'fields', 'sections', 'palette', 'emptyMessage', 'onFieldChange', 'onSave', 'onRetry', 'saveState', 'saveError', 'pinned', 'onPin', 'onOpen', 'onSectionToggle', 'children'],
  slots: ['preview', 'metadata', 'sections', 'palette', 'actions', 'children'],
  states: ['empty', 'single', 'multiple', 'pinned', 'editing', 'saving', 'saved', 'error', 'cancelled', 'grouped', 'palette', 'preview-landscape', 'preview-portrait', 'no-preview'],
  actions: ['pin', 'open', 'edit', 'save', 'retry', 'toggle-section'],
  browserActions: ['pin', 'open', 'edit', 'save'],
  observables: ['inspector-role', 'selection-mode', 'metadata', 'field-groups', 'palette', 'pin-state', 'open-callback', 'edit-field', 'save-state', 'validation', 'section-toggle', 'selected-count', 'preview-slot', 'preview-geometry'],
  stories: ['empty', 'single', 'multiple', 'preview-landscape', 'preview-portrait', 'no-preview', 'grouped-palette', 'edit-success', 'edit-validation-error', 'edit-failure-retry'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/inspector.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Selection and Inspector ownership remain host-owned projections' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Inspector selection modes, editable fields, save state and retry behavior' },
    { kind: 'observed', reference: 'docs/research/eagle/eagle-analysis/component-style-evidence.md', claim: 'Inspector preview fit, palette pill, grouped information sections and no-preview branch' },
    { kind: 'observed', reference: 'docs/research/eagle/ui-kit/populated-evidence.md', claim: 'Populated selection updates Inspector and range selection exposes selected count' },
  ],
  visual: { profile: 'dense-panel', surfaces: { root: 'panel', interactive: 'raised', selected: 'active' }, radius: 'sm', density: 'compact', geometry: ['column:250px', 'control:28px', 'control-radius:16px', 'preview:min-height-200px', 'palette:pill-26px'] },
  behavior: { stateOwner: 'host', focus: 'pointer', selection: 'host', actionLifecycle: [{ action: 'pin', transitions: ['requested', 'committed'] }, { action: 'open', transitions: ['requested', 'committed'] }, { action: 'edit', transitions: ['requested', 'committed'] }, { action: 'save', transitions: ['requested', 'committed', 'cancelled', 'failed', 'retrying'] }, { action: 'retry', transitions: ['requested', 'committed', 'failed'] }, { action: 'toggle-section', transitions: ['requested', 'committed'] }] },
} satisfies ComponentContract<VelvetInspectorProps, 'empty' | 'single' | 'multiple' | 'pinned' | 'editing' | 'saving' | 'saved' | 'error' | 'cancelled' | 'grouped' | 'palette' | 'preview-landscape' | 'preview-portrait' | 'no-preview', 'pin' | 'open' | 'edit' | 'save' | 'retry' | 'toggle-section', 'inspector-role' | 'selection-mode' | 'metadata' | 'field-groups' | 'palette' | 'pin-state' | 'open-callback' | 'edit-field' | 'save-state' | 'validation' | 'section-toggle' | 'selected-count' | 'preview-slot' | 'preview-geometry'>);
