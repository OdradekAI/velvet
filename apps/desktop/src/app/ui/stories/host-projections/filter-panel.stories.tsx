import { useState } from 'react';
import type { UiKitStory } from '../../story-registry.js';
import { VelvetFilterPanel, type VelvetFilterRule } from '../../host-projections/index.js';

const facets = [
  { id: 'type', label: 'Type', kind: 'multi' as const, options: [{ id: 'image', label: 'Image', count: 12 }, { id: 'video', label: 'Video', count: 2 }] },
  { id: 'folder', label: 'Folder', kind: 'single' as const, options: [{ id: 'reference', label: 'Reference', count: 8 }, { id: 'inspiration', label: 'Inspiration', count: 4 }] },
  { id: 'annotation', label: 'Annotation', kind: 'text' as const, placeholder: 'Contains…' },
];

let currentRules: readonly VelvetFilterRule[] = [];
let savedRules: readonly VelvetFilterRule[] = [];
let closed = false;
let changeRules: ((rules: readonly VelvetFilterRule[]) => void) | undefined;

export const filterPanelStories: readonly UiKitStory[] = [
  {
    id: 'filter-panel/default',
    componentId: 'filter-panel',
    name: 'Default',
    description: 'Host-owned filter facets with query rules and result counts.',
    render: () => <StoryFilterPanel />,
    scenario: {
      covers: ['change'],
      setup: () => { currentRules = []; },
      actions: [(runtime) => runtime.act(async () => { changeRules?.([{ facetId: 'annotation', operator: 'contain', value: 'poster' }]); })],
      checkpoints: [{ name: 'text rule change', afterAction: 0, assert: () => {
        if (currentRules.length !== 1 || currentRules[0]?.facetId !== 'annotation' || currentRules[0]?.value !== 'poster') throw new Error('FilterPanel text rule contract failed');
      } }],
    },
  },
  {
    id: 'filter-panel/multi-select',
    componentId: 'filter-panel',
    name: 'Multi-select',
    description: 'Multi-value facets use intersection semantics and preserve host ownership.',
    render: () => <StoryFilterPanel />,
    scenario: {
      covers: ['change'],
      setup: () => { currentRules = []; },
      actions: [(runtime) => runtime.act(async () => { changeRules?.([{ facetId: 'type', operator: 'intersection', value: ['image'] }]); })],
      checkpoints: [{ name: 'multi rule change', afterAction: 0, assert: () => {
        const rule = currentRules.find((candidate) => candidate.facetId === 'type');
        if (!rule || !Array.isArray(rule.value) || rule.value[0] !== 'image') throw new Error('FilterPanel multi rule contract failed');
      } }],
    },
  },
  {
    id: 'filter-panel/reset-save',
    componentId: 'filter-panel',
    name: 'Reset and save',
    description: 'Reset and save actions are reported to the host.',
    render: () => <StoryFilterPanel onReset={() => undefined} onSave={(rules) => { savedRules = rules; }} />,
    scenario: {
      covers: ['reset', 'save'],
      setup: () => { currentRules = [{ facetId: 'type', operator: 'intersection', value: ['image'] }]; savedRules = []; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('button')?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelectorAll<HTMLButtonElement>('button')[1]?.click(); }),
      ],
      checkpoints: [
        { name: 'reset', afterAction: 0, assert: () => { if (currentRules.length !== 0) throw new Error('FilterPanel reset contract failed'); } },
        { name: 'save', afterAction: 1, assert: () => { if (savedRules.length !== 0) throw new Error('FilterPanel save contract failed'); } },
      ],
    },
  },
  {
    id: 'filter-panel/keyboard-close',
    componentId: 'filter-panel',
    name: 'Keyboard close',
    description: 'Close is a host action and remains a native keyboard button.',
    render: () => <StoryFilterPanel onClose={() => { closed = true; }} />,
    scenario: {
      covers: ['close'],
      setup: () => { closed = false; },
      actions: [(runtime) => runtime.act(async () => {
        const button = runtime.root.querySelector<HTMLButtonElement>('button');
        button?.focus();
        button?.click();
      })],
      checkpoints: [{ name: 'close action', afterAction: 0, assert: () => { if (!closed) throw new Error('FilterPanel close contract failed'); } }],
    },
  },
];

function StoryFilterPanel({
  onClose,
  onSave,
}: {
  readonly onClose?: () => void;
  readonly onReset?: () => void;
  readonly onSave?: (rules: readonly VelvetFilterRule[]) => void;
}) {
  const [rules, setRules] = useState<readonly VelvetFilterRule[]>(currentRules);
  const updateRules = (nextRules: readonly VelvetFilterRule[]) => {
    currentRules = nextRules;
    setRules(nextRules);
  };
  changeRules = updateRules;
  return (
    <VelvetFilterPanel
      facets={facets}
      onClose={onClose}
      onReset={() => { updateRules([]); }}
      onRuleChange={updateRules}
      onSave={onSave}
      rules={rules}
    />
  );
}
