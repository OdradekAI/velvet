import { useRef, useState } from 'react';
import type { UiKitStory } from '../../story-registry.js';
import { VelvetButton } from '../../primitives/index.js';
import { VelvetQuickSearch, type VelvetQuickSearchResult, type VelvetQuickSearchTabId } from '../../host-projections/index.js';

const tabs = [
  { id: 'folders' as const, label: 'Folders' },
  { id: 'items' as const, label: 'Items' },
  { id: 'tags' as const, label: 'Tags' },
  { id: 'smart-folders' as const, label: 'Smart Folders' },
];

const results: readonly VelvetQuickSearchResult[] = [
  { id: 'item-beijing', kind: 'items', label: '北京海报', searchAliases: ['beijing haibao', '北京海報'], score: 0.9 },
  { id: 'item-reference', kind: 'items', label: '参考资料', searchAliases: ['canka o ziliao', '參考資料'], score: 0.8 },
  { id: 'folder-reference', kind: 'folders', label: '参考文件夹', searchAliases: ['canka o wenjianjia', '參考文件夾'], score: 0.7 },
  { id: 'tag-inspiration', kind: 'tags', label: '灵感', searchAliases: ['linggan', '靈感'], score: 0.8 },
  { id: 'smart-recent', kind: 'smart-folders', label: '最近添加', searchAliases: ['recent'] },
];

let selectedResult = '';
let selectedKind: VelvetQuickSearchTabId | '' = '';

export const quickSearchStories: readonly UiKitStory[] = [
  {
    id: 'quick-search/open',
    componentId: 'quick-search',
    name: 'Open and alias match',
    description: 'Pinyin aliases match a host-owned item result and retain active descendant semantics.',
    render: () => <VelvetQuickSearch
      activeTab="items"
      onClose={() => undefined}
      onQueryChange={() => undefined}
      onSelect={(id, kind) => { selectedResult = id; selectedKind = kind; }}
      open
      query="beijing"
      results={results}
      tabs={tabs}
    />,
    scenario: {
      covers: ['query', 'navigate', 'select'],
      setup: () => { selectedResult = ''; selectedKind = ''; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input')?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowDown' })); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input')?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' })); }),
      ],
      checkpoints: [
        { name: 'alias match', afterAction: 0, assert: (runtime) => {
          if (runtime.root.querySelector('[role="option"]')?.textContent?.trim() !== '北京海报' || runtime.root.querySelector('[role="dialog"] input')?.getAttribute('aria-activedescendant') === null) throw new Error('Quick Search pinyin alias contract failed');
        } },
        { name: 'keyboard select', afterAction: 1, assert: () => { if (selectedResult !== 'item-beijing' || selectedKind !== 'items') throw new Error('Quick Search keyboard selection contract failed'); } },
      ],
    },
  },
  {
    id: 'quick-search/tabs-history',
    componentId: 'quick-search',
    name: 'Tabs and history',
    description: 'Host-owned tabs and bounded deduplicated history remain visible actions.',
    render: () => <TabsHistoryStory />,
    scenario: {
      covers: ['tab-change', 'history-select'],
      setup: () => { selectedResult = ''; selectedKind = ''; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[role="tab"][aria-label="Tags"]')?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-quick-search-history-item]')?.click(); }),
      ],
      checkpoints: [
        { name: 'tags tab', afterAction: 0, assert: (runtime) => { if (runtime.root.querySelector('[role="tab"][aria-selected="true"]')?.textContent?.trim() !== 'Tags' || runtime.root.querySelector('[role="option"]')?.textContent?.trim() !== '灵感') throw new Error('Quick Search tab contract failed'); } },
        { name: 'history selection', afterAction: 1, assert: (runtime) => { if (runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input')?.value !== '灵感' || runtime.root.querySelector('[data-quick-search-history="true"]') !== null) throw new Error('Quick Search history contract failed'); } },
      ],
    },
  },
  {
    id: 'quick-search/keyboard-focus',
    componentId: 'quick-search',
    name: 'Keyboard and focus restore',
    description: 'Escape closes the overlay and restores the trigger focus.',
    render: () => <KeyboardFocusStory />,
    scenario: {
      covers: ['close', 'navigate'],
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-quick-search-trigger]')?.click(); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input')?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' })); }),
      ],
      checkpoints: [
        { name: 'input focus', afterAction: 0, assert: (runtime) => { if (runtime.root.querySelector('[role="dialog"] input') !== runtime.root.ownerDocument.activeElement) throw new Error('Quick Search focus entry contract failed'); } },
        { name: 'focus restore', afterAction: 1, assert: (runtime) => { if (runtime.root.querySelector('[role="dialog"]') !== null || runtime.root.ownerDocument.activeElement?.getAttribute('data-quick-search-trigger') !== 'true') throw new Error('Quick Search focus restore contract failed'); } },
      ],
    },
  },
];

function TabsHistoryStory() {
  const [activeTab, setActiveTab] = useState<VelvetQuickSearchTabId>('items');
  const [query, setQuery] = useState('');
  return <VelvetQuickSearch activeTab={activeTab} history={['灵感', '灵感', '北京']} historyLimit={2} onClose={() => undefined} onHistorySelect={setQuery} onQueryChange={setQuery} onSelect={(id, kind) => { selectedResult = id; selectedKind = kind; }} onTabChange={setActiveTab} open query={query} results={results} tabs={tabs} />;
}

function KeyboardFocusStory() {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  return <>
    <VelvetButton data-quick-search-trigger="true" onClick={() => setOpen(true)} ref={triggerRef}>Quick Search</VelvetButton>
    <VelvetQuickSearch activeTab="items" onClose={() => setOpen(false)} onSelect={() => undefined} open={open} restoreFocusRef={triggerRef} results={results} tabs={tabs} />
  </>;
}
