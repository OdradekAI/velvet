export const UI_KIT_CATALOG_SCHEMA = 'velvet.ui.catalog' as const;
export const UI_KIT_CATALOG_SCHEMA_VERSION = 1 as const;

export type UiKitComponentCategory = 'primitive' | 'composite' | 'overlay';

export interface ComponentMeta {
  readonly id: string;
  readonly name: string;
  readonly category: UiKitComponentCategory;
  readonly purpose: string;
  readonly ownership: 'host';
  readonly states: readonly string[];
  readonly props: readonly string[];
  readonly keyboard: readonly string[];
  readonly accessibility: readonly string[];
  readonly stories: readonly string[];
}

export const COMPONENT_META = [
  { id: 'text', name: 'VelvetText', category: 'primitive', purpose: 'Semantic text with host typography and tone tokens.', ownership: 'host', states: ['default', 'tone'], props: ['as', 'tone', 'children'], keyboard: [], accessibility: ['preserves semantic heading/paragraph elements'], stories: ['default', 'tones'] },
  { id: 'icon', name: 'VelvetIcon', category: 'primitive', purpose: 'Accessible or decorative icon wrapper for toolbars and controls.', ownership: 'host', states: ['default', 'decorative'], props: ['label', 'children'], keyboard: [], accessibility: ['decorative icons are hidden; labelled icons expose a name'], stories: ['default', 'labelled'] },
  { id: 'badge', name: 'VelvetBadge', category: 'primitive', purpose: 'Compact status or count marker.', ownership: 'host', states: ['default', 'tone'], props: ['tone', 'children'], keyboard: [], accessibility: ['renders as text without stealing focus'], stories: ['default', 'tones'] },
  { id: 'stack', name: 'VelvetStack', category: 'primitive', purpose: 'Token-spaced vertical or horizontal layout primitive.', ownership: 'host', states: ['default', 'direction', 'gap'], props: ['direction', 'gap', 'children'], keyboard: [], accessibility: ['layout wrapper preserves child semantics'], stories: ['default', 'directions'] },
  { id: 'button', name: 'VelvetButton', category: 'primitive', purpose: 'Command or local action control.', ownership: 'host', states: ['default', 'quiet', 'danger', 'disabled', 'busy', 'focus'], props: ['variant', 'label', 'busy', 'disabled', 'onClick'], keyboard: ['Enter', 'Space'], accessibility: ['native button semantics', 'busy state is announced'], stories: ['default', 'busy', 'disabled'] },
  { id: 'icon-button', name: 'VelvetIconButton', category: 'primitive', purpose: 'Compact icon-first action with an accessible label.', ownership: 'host', states: ['default', 'disabled', 'focus'], props: ['icon', 'label', 'disabled', 'onClick'], keyboard: ['Enter', 'Space'], accessibility: ['requires an accessible label'], stories: ['default', 'disabled'] },
  { id: 'input', name: 'VelvetInput', category: 'primitive', purpose: 'Text entry control with host density and focus styling.', ownership: 'host', states: ['default', 'focus', 'disabled', 'busy'], props: ['value', 'placeholder', 'disabled', 'busy', 'onChange'], keyboard: ['Tab', 'Escape'], accessibility: ['uses native input semantics', 'label is supplied by ariaLabel or surrounding label'], stories: ['default', 'busy'] },
  { id: 'select', name: 'VelvetSelect', category: 'primitive', purpose: 'Native select control with host styling.', ownership: 'host', states: ['default', 'focus', 'disabled'], props: ['value', 'disabled', 'onChange', 'children'], keyboard: ['Arrow keys', 'Enter'], accessibility: ['uses native select semantics'], stories: ['default', 'disabled'] },
  { id: 'checkbox', name: 'VelvetCheckbox', category: 'primitive', purpose: 'Controlled boolean setting control.', ownership: 'host', states: ['checked', 'unchecked', 'disabled', 'focus'], props: ['checked', 'label', 'onCheckedChange', 'disabled'], keyboard: ['Space'], accessibility: ['native checkbox semantics', 'label is associated'], stories: ['checked', 'disabled'] },
  { id: 'separator', name: 'VelvetSeparator', category: 'primitive', purpose: 'Visual and semantic divider.', ownership: 'host', states: ['horizontal', 'vertical'], props: ['orientation'], keyboard: [], accessibility: ['exposes separator orientation'], stories: ['horizontal', 'vertical'] },
  { id: 'panel', name: 'VelvetPanel', category: 'composite', purpose: 'Surface container for host panes and product views.', ownership: 'host', states: ['panel', 'raised', 'overlay'], props: ['title', 'surface', 'children'], keyboard: [], accessibility: ['section label and heading are preserved'], stories: ['panel', 'raised'] },
  { id: 'dialog', name: 'VelvetDialog', category: 'overlay', purpose: 'Host-owned modal overlay with focus restoration.', ownership: 'host', states: ['closed', 'open', 'warning', 'danger', 'busy'], props: ['open', 'title', 'description', 'actions', 'onOpenChange', 'restoreFocusRef'], keyboard: ['Escape', 'Tab', 'Shift+Tab'], accessibility: ['Dialog Title/Description semantics', 'focus trap and restore'], stories: ['closed', 'open'] },
  { id: 'context-menu', name: 'VelvetContextMenu', category: 'overlay', purpose: 'Trigger-driven contextual command menu.', ownership: 'host', states: ['closed', 'open', 'disabled', 'checked'], props: ['actions', 'children'], keyboard: ['Arrow keys', 'Enter', 'Escape', 'typeahead'], accessibility: ['menu and menuitem semantics'], stories: ['default', 'checked'] },
  { id: 'tabs', name: 'VelvetTabs', category: 'primitive', purpose: 'Roving-focus tab selection control.', ownership: 'host', states: ['selected', 'disabled', 'focus'], props: ['tabs', 'selectedId', 'onSelect'], keyboard: ['Arrow keys', 'Home', 'End'], accessibility: ['tablist/tab semantics and roving focus'], stories: ['default', 'disabled'] },
  { id: 'progress', name: 'VelvetProgress', category: 'primitive', purpose: 'Job progress projection.', ownership: 'host', states: ['determinate', 'indeterminate', 'success', 'danger'], props: ['value', 'label', 'tone'], keyboard: [], accessibility: ['progress label and value are exposed'], stories: ['determinate', 'indeterminate'] },
  { id: 'state', name: 'VelvetState', category: 'primitive', purpose: 'Loading, empty and error state composition.', ownership: 'host', states: ['loading', 'empty', 'error'], props: ['kind', 'title', 'description', 'onRetry', 'action'], keyboard: ['Enter on retry'], accessibility: ['status/alert live-region semantics'], stories: ['loading', 'empty', 'error'] },
  { id: 'toast', name: 'VelvetToastHost', category: 'overlay', purpose: 'Live notification projection with dismissal.', ownership: 'host', states: ['info', 'success', 'warning', 'error'], props: ['items', 'onDismiss'], keyboard: ['Tab', 'Enter'], accessibility: ['live region and alert semantics'], stories: ['default', 'error'] },
  { id: 'activity-bar', name: 'VelvetActivityBar', category: 'composite', purpose: 'Workbench app navigation rail.', ownership: 'host', states: ['selected', 'disabled', 'focus', 'badge'], props: ['items', 'onSelect'], keyboard: ['Arrow keys', 'Home', 'End'], accessibility: ['navigation landmark and roving focus'], stories: ['default', 'badges'] },
  { id: 'pane', name: 'VelvetPane', category: 'composite', purpose: 'Semantic Workbench zone container.', ownership: 'host', states: ['navigation', 'context', 'operations', 'preview'], props: ['title', 'region', 'children'], keyboard: [], accessibility: ['labelled section and region marker'], stories: ['navigation', 'context'] },
  { id: 'preview-host', name: 'VelvetPreviewHost', category: 'composite', purpose: 'Host frame for Preview lifecycle and fallback states.', ownership: 'host', states: ['loading', 'ready', 'empty', 'error', 'disabled'], props: ['state', 'message', 'onRetry', 'children'], keyboard: ['Enter on retry'], accessibility: ['preview landmark and state announcement'], stories: ['ready', 'error'] },
  { id: 'asset-grid', name: 'VelvetAssetGrid', category: 'composite', purpose: 'Keyboard-navigable Asset projection grid.', ownership: 'host', states: ['default', 'selected', 'disabled', 'empty'], props: ['items', 'columns', 'onSelect', 'onOpen'], keyboard: ['Arrow keys', 'Enter'], accessibility: ['grid/gridcell semantics and selection'], stories: ['default', 'selected'] },
  { id: 'data-table', name: 'VelvetDataTable', category: 'composite', purpose: 'Sortable tabular projection for host-owned data.', ownership: 'host', states: ['default', 'selected', 'sortable', 'empty'], props: ['rows', 'columns', 'selectedId', 'onSelect', 'onSort'], keyboard: ['Tab', 'Enter'], accessibility: ['table headers, labelled rows and selection'], stories: ['default', 'selected'] },
  { id: 'command-palette', name: 'VelvetCommandPalette', category: 'composite', purpose: 'Host-owned command search overlay.', ownership: 'host', states: ['open', 'empty', 'selected', 'disabled'], props: ['open', 'commands', 'onSelect', 'onClose'], keyboard: ['Arrow keys', 'Enter', 'Escape'], accessibility: ['dialog/listbox semantics and focus entry'], stories: ['open', 'empty'] },
] as const satisfies readonly ComponentMeta[];

export type UiKitComponentId = (typeof COMPONENT_META)[number]['id'];

export interface UiKitCatalog {
  readonly schema: typeof UI_KIT_CATALOG_SCHEMA;
  readonly schemaVersion: typeof UI_KIT_CATALOG_SCHEMA_VERSION;
  readonly apiVersion: '1.0';
  readonly components: readonly ComponentMeta[];
}

export const UI_KIT_CATALOG: UiKitCatalog = {
  schema: UI_KIT_CATALOG_SCHEMA,
  schemaVersion: UI_KIT_CATALOG_SCHEMA_VERSION,
  apiVersion: '1.0',
  components: COMPONENT_META,
};

export function componentMeta(id: UiKitComponentId): ComponentMeta {
  const meta = COMPONENT_META.find((candidate) => candidate.id === id);
  if (!meta) {
    throw new Error(`Unknown Velvet UI Kit component: ${id}`);
  }
  return meta;
}
