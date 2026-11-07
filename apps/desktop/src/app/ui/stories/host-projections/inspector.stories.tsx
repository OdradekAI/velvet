import type { UiKitStory } from '../../story-registry.js';
import { VelvetInspector } from '../../host-projections/index.js';
import { useState } from 'react';

let opened = false;
let editedValue = '';
let retryCount = 0;
let changeField: ((fieldId: string, value: string) => void) | undefined;

function InspectorSingleStory() {
  const [pinned, setPinned] = useState(false);
  return <VelvetInspector title="Inspector" selectedCount={1} preview={<div aria-label="Asset preview" role="img" />} fields={[{ id: 'folder', label: 'Folder', value: 'Illustration' }, { id: 'tags', label: 'Tags', value: 'reference' }]} onOpen={() => { opened = true; }} onPin={setPinned} pinned={pinned} />;
}

function InspectorEditSuccessStory() {
  const [name, setName] = useState('Old title');
  const [saveState, setSaveState] = useState<'idle' | 'saved'>('idle');
  changeField = (_fieldId, value) => { editedValue = value; setName(value); };
  return (
    <VelvetInspector
      fields={[{ id: 'name', label: 'Name', value: name, editable: true, inputValue: name, placeholder: 'Title' }]}
      onFieldChange={changeField}
      onSave={() => setSaveState('saved')}
      saveState={saveState}
      selectedCount={1}
      title="Inspector"
    />
  );
}

function InspectorFailureRetryStory() {
  const [saveState, setSaveState] = useState<'error' | 'saved'>('error');
  return (
    <VelvetInspector
      fields={[{ id: 'name', label: 'Name', value: 'Draft', editable: true, inputValue: 'Draft' }]}
      onRetry={() => { retryCount += 1; setSaveState('saved'); }}
      onSave={() => undefined}
      saveError="Could not save Inspector changes."
      saveState={saveState}
      selectedCount={1}
      title="Inspector"
    />
  );
}

export const inspectorStories: readonly UiKitStory[] = [
  {
    id: 'inspector/empty',
    componentId: 'inspector',
    name: 'Empty',
    description: 'No selected asset.',
    render: () => <VelvetInspector title="Inspector" selectedCount={0} />,
  },
  {
    id: 'inspector/single',
    componentId: 'inspector',
    name: 'Single selection',
    description: 'Single selected asset with host actions.',
    render: () => <InspectorSingleStory />,
    scenario: {
      setup: () => { opened = false; editedValue = ''; retryCount = 0; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[aria-label="Pin inspector"]')?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('button:not([aria-label])')?.click(); }),
      ],
      checkpoints: [
        { name: 'pin action', afterAction: 0, assert: (runtime) => { if (runtime.root.querySelector('[aria-pressed="true"]') === null) throw new Error('Inspector pin contract failed'); } },
        { name: 'open action', afterAction: 1, assert: () => { if (!opened) throw new Error('Inspector open contract failed'); } },
      ],
    },
  },
  {
    id: 'inspector/multiple',
    componentId: 'inspector',
    name: 'Multiple selection',
    description: 'Aggregated fields for multiple selected assets.',
    render: () => <VelvetInspector title="Inspector" selectedCount={2} fields={[{ id: 'selection', label: 'Selection', value: '2 assets' }, { id: 'size', label: 'Size', value: '4.2 MB' }]} />,
  },
  {
    id: 'inspector/edit-success',
    componentId: 'inspector',
    name: 'Edit success',
    description: 'Controlled field edit and committed save.',
    render: () => <InspectorEditSuccessStory />,
    scenario: {
      setup: () => { editedValue = ''; },
      actions: [
        (runtime) => runtime.act(async () => { changeField?.('name', 'New title'); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-inspector-action="save"]')?.click(); }),
      ],
      checkpoints: [
        { name: 'field edit', afterAction: 0, assert: () => { if (editedValue !== 'New title') throw new Error('Inspector field edit contract failed'); } },
        { name: 'save committed', afterAction: 1, assert: (runtime) => { if (runtime.root.querySelector('[role="status"]')?.textContent !== 'Saved') throw new Error('Inspector save success contract failed'); } },
      ],
    },
  },
  {
    id: 'inspector/edit-validation-error',
    componentId: 'inspector',
    name: 'Edit validation error',
    description: 'Host-owned validation error on an editable field.',
    render: () => <VelvetInspector fields={[{ id: 'name', label: 'Name', value: '', editable: true, inputValue: '', error: 'Name is required.' }]} onSave={() => undefined} saveError="Fix the highlighted field." saveState="error" selectedCount={1} title="Inspector" />,
  },
  {
    id: 'inspector/edit-failure-retry',
    componentId: 'inspector',
    name: 'Edit failure retry',
    description: 'Failed save with host-owned retry.',
    render: () => <InspectorFailureRetryStory />,
    scenario: {
      setup: () => { retryCount = 0; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-inspector-action="retry"]')?.click(); })],
      checkpoints: [{ name: 'retry committed', afterAction: 0, assert: (runtime) => { if (retryCount !== 1 || runtime.root.querySelector('[role="status"]')?.textContent !== 'Saved') throw new Error('Inspector retry contract failed'); } }],
    },
  },
];
