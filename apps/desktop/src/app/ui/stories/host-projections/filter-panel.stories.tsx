import { useState } from 'react';
import type { UiKitStory } from '../../story-registry.js';
import { VelvetFilterPanel, type VelvetFilterFacet, type VelvetFilterRule } from '../../host-projections/index.js';

const facets = [
  { id: 'type', label: 'Type', kind: 'multi' as const, options: [{ id: 'image', label: 'Image', count: 12 }, { id: 'video', label: 'Video', count: 2 }] },
  { id: 'folder', label: 'Folder', kind: 'single' as const, options: [{ id: 'reference', label: 'Reference', count: 8 }, { id: 'inspiration', label: 'Inspiration', count: 4 }] },
  { id: 'annotation', label: 'Annotation', kind: 'text' as const, placeholder: 'Contains…' },
];

const semanticFacets = [
  { id: 'folder', label: 'Folder', semantic: 'folder' as const, kind: 'single' as const, options: [{ id: 'reference', label: 'Reference' }] },
  { id: 'tag', label: 'Tag', semantic: 'tag' as const, kind: 'multi' as const, options: [{ id: 'illustration', label: 'Illustration' }] },
  { id: 'color', label: 'Color', semantic: 'color' as const, kind: 'single' as const, options: [{ id: 'blue', label: 'Blue' }] },
  { id: 'rating', label: 'Rating', semantic: 'rating' as const, kind: 'range' as const, placeholder: '0–5' },
  { id: 'date', label: 'Date', semantic: 'date' as const, kind: 'range' as const, placeholder: 'YYYY-MM-DD..YYYY-MM-DD' },
];

let currentRules: readonly VelvetFilterRule[] = [];
let savedRules: readonly VelvetFilterRule[] = [];
let appliedRules: readonly VelvetFilterRule[] = [];
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
    id: 'filter-panel/semantic-facets',
    componentId: 'filter-panel',
    name: 'Semantic facets',
    description: 'Folder, tag, color, rating and date facets publish stable operators.',
    render: () => <StoryFilterPanel facets={semanticFacets} />,
    scenario: {
      covers: ['change'],
      setup: () => { currentRules = []; },
      actions: [
        (runtime) => runtime.act(async () => { setNativeValue(runtime.root.querySelector<HTMLSelectElement>('[data-filter-semantic="folder"] select'), 'reference'); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLInputElement>('[data-filter-semantic="tag"] input[type="checkbox"]')?.click(); }),
        (runtime) => runtime.act(async () => { setNativeValue(runtime.root.querySelector<HTMLSelectElement>('[data-filter-semantic="color"] select'), 'blue'); }),
        (runtime) => runtime.act(async () => { setNativeValue(runtime.root.querySelector<HTMLInputElement>('[data-filter-semantic="rating"] input'), '4'); }),
        (runtime) => runtime.act(async () => { setNativeValue(runtime.root.querySelector<HTMLInputElement>('[data-filter-semantic="date"] input'), '2026-01-01..2026-02-01'); }),
      ],
      checkpoints: [
        { name: 'folder operator', afterAction: 0, assert: () => assertSemanticRule('folder', 'equal') },
        { name: 'tag operator', afterAction: 1, assert: () => assertSemanticRule('tag', 'intersection') },
        { name: 'color operator', afterAction: 2, assert: () => assertSemanticRule('color', 'equal') },
        { name: 'rating operator', afterAction: 3, assert: () => assertSemanticRule('rating', 'between') },
        { name: 'date operator', afterAction: 4, assert: (runtime) => {
          assertSemanticRule('date', 'within');
          const semantics = [...runtime.root.querySelectorAll('[data-filter-semantic]')].map((node) => `${node.getAttribute('data-filter-semantic')}:${node.getAttribute('data-filter-operator')}`);
          if (semantics.join('|') !== 'folder:equal|tag:intersection|color:equal|rating:between|date:within') throw new Error('FilterPanel semantic facet metadata contract failed');
        } },
      ],
    },
  },
  {
    id: 'filter-panel/reset-save',
    componentId: 'filter-panel',
    name: 'Reset and save',
    description: 'Reset, save and apply actions are reported to the host.',
    render: () => <StoryFilterPanel onApply={(rules) => { appliedRules = rules; }} onReset={() => undefined} onSave={(rules) => { savedRules = rules; }} resultCount={42} />,
    scenario: {
      covers: ['reset', 'save', 'apply'],
      setup: () => { currentRules = [{ facetId: 'type', operator: 'intersection', value: ['image'] }]; savedRules = []; appliedRules = []; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('button')?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelectorAll<HTMLButtonElement>('button')[1]?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-filter-action="apply"]')?.click(); }),
      ],
      checkpoints: [
        { name: 'reset', afterAction: 0, assert: () => { if (currentRules.length !== 0) throw new Error('FilterPanel reset contract failed'); } },
        { name: 'save', afterAction: 1, assert: () => { if (savedRules.length !== 0) throw new Error('FilterPanel save contract failed'); } },
        { name: 'apply and result count', afterAction: 2, assert: (runtime) => { if (appliedRules.length !== 0 || runtime.root.querySelector('[data-filter-result-count="true"]')?.textContent !== '42 results') throw new Error('FilterPanel apply/result count contract failed'); } },
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
  {
    id: 'filter-panel/closed',
    componentId: 'filter-panel',
    name: 'Closed',
    description: 'Closed filter projection does not own or render query state.',
    render: () => <StoryFilterPanel open={false} />,
    scenario: {
      actions: [],
      checkpoints: [{ name: 'closed projection', assert: (runtime) => { if (runtime.root.querySelector('[data-ui-kit-component="filter-panel"]') !== null) throw new Error('FilterPanel closed state contract failed'); } }],
    },
  },
];

function StoryFilterPanel({
  facets: storyFacets = facets,
  onClose,
  onApply,
  open = true,
  resultCount,
  onSave,
}: {
  readonly facets?: readonly VelvetFilterFacet[];
  readonly onClose?: () => void;
  readonly onApply?: (rules: readonly VelvetFilterRule[]) => void;
  readonly open?: boolean;
  readonly onReset?: () => void;
  readonly onSave?: (rules: readonly VelvetFilterRule[]) => void;
  readonly resultCount?: number;
}) {
  const [rules, setRules] = useState<readonly VelvetFilterRule[]>(currentRules);
  const updateRules = (nextRules: readonly VelvetFilterRule[]) => {
    currentRules = nextRules;
    setRules(nextRules);
  };
  changeRules = updateRules;
  return (
    <VelvetFilterPanel
      facets={storyFacets}
      onApply={onApply}
      onClose={onClose}
      onReset={() => { updateRules([]); }}
      onRuleChange={updateRules}
      onSave={onSave}
      open={open}
      resultCount={resultCount}
      rules={rules}
    />
  );
}

function setNativeValue(element: HTMLInputElement | HTMLSelectElement | null, value: string): void {
  if (!element) throw new Error('FilterPanel semantic control missing');
  Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), 'value')?.set?.call(element, value);
  element.dispatchEvent(new Event(element.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }));
}

function assertSemanticRule(facetId: string, operator: VelvetFilterRule['operator']): void {
  const rule = currentRules.find((candidate) => candidate.facetId === facetId);
  if (rule?.operator !== operator) throw new Error(`FilterPanel ${facetId} semantic operator contract failed`);
}
