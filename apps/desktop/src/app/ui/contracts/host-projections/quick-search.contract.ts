import type { VelvetQuickSearchProps, VelvetQuickSearchTabId } from '../../host-projections/quick-search.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetQuickSearchContract = defineComponentContract<
  VelvetQuickSearchProps,
  'closed' | 'open' | 'empty' | 'history' | 'selected',
  'query' | 'tab-change' | 'history-select' | 'select' | 'navigate' | 'close',
  'dialog-role' | 'input-focus' | 'tablist-role' | 'active-descendant' | 'pinyin-alias' | 'traditional-alias' | 'history' | 'selection' | 'focus-restore'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'quick-search',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['open', 'tabs', 'activeTab', 'results', 'query', 'history', 'historyLimit', 'onQueryChange', 'onTabChange', 'onHistorySelect', 'onSelect', 'onClose', 'restoreFocusRef'],
  slots: ['tabs', 'results', 'history'],
  states: ['closed', 'open', 'empty', 'history', 'selected'],
  actions: ['query', 'tab-change', 'history-select', 'select', 'navigate', 'close'],
  browserActions: ['query', 'tab-change', 'history-select', 'select', 'navigate', 'close'],
  observables: ['dialog-role', 'input-focus', 'tablist-role', 'active-descendant', 'pinyin-alias', 'traditional-alias', 'history', 'selection', 'focus-restore'],
  stories: ['open', 'tabs-history', 'keyboard-focus'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/quick-search.md',
  evidence: [
    { kind: 'observed', reference: 'docs/engineering/ui-kit/components/host-projections/quick-search.md', claim: 'The host projection exposes four candidate tabs, alias matching, scoring, bounded history and keyboard actions.' },
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Open/close focus and history persistence remain host-owned boundaries.' },
    { kind: 'unknown', reference: 'docs/engineering/ui-kit/components/host-projections/quick-search.md', claim: 'History persistence and renderer timing are intentionally outside the component contract.' },
  ],
  visual: { profile: 'overlay', surfaces: { root: 'overlay', interactive: 'active', selected: 'active' }, radius: 'md', density: 'compact', geometry: ['surface:600px', 'row:34px', 'history-chip:24px', 'tabs:24px'] },
  behavior: { stateOwner: 'host', focus: 'restore', selection: 'host', actionLifecycle: [{ action: 'query', transitions: ['requested', 'committed'] }, { action: 'tab-change', transitions: ['requested', 'committed'] }, { action: 'history-select', transitions: ['requested', 'committed'] }, { action: 'select', transitions: ['requested', 'committed', 'failed'] }, { action: 'navigate', transitions: ['requested', 'committed'] }, { action: 'close', transitions: ['requested', 'committed', 'cancelled'] }] },
} satisfies ComponentContract<VelvetQuickSearchProps, 'closed' | 'open' | 'empty' | 'history' | 'selected', 'query' | 'tab-change' | 'history-select' | 'select' | 'navigate' | 'close', 'dialog-role' | 'input-focus' | 'tablist-role' | 'active-descendant' | 'pinyin-alias' | 'traditional-alias' | 'history' | 'selection' | 'focus-restore'>);

export type VelvetQuickSearchContractTab = VelvetQuickSearchTabId;
