import { useRef, useState } from 'react';
import {
  VelvetAssetGrid,
  VelvetActivityBar,
  VelvetButton,
  VelvetCommandPalette,
  VelvetDataTable,
  VelvetFilterFacetMenu,
  VelvetFilterPanel,
  VelvetInspector,
  VelvetPane,
  VelvetPanel,
  VelvetPreviewHost,
  VelvetQuickSearch,
  VelvetStack,
  VelvetText,
  VelvetZoomControl,
} from '../ui/index.js';
import type { VelvetFilterRule } from '../ui/index.js';
import type { VelvetFilterFolderOption, VelvetFilterFolderRule, VelvetInspectorSection } from '../ui/index.js';

const ASSETS = [
  { id: 'asset-1', label: 'Astronaut poster', secondary: 'Illustration' },
  { id: 'asset-2', label: 'Editorial study', secondary: 'Photography' },
  { id: 'asset-3', label: 'Night landscape', secondary: 'Photography' },
  { id: 'asset-4', label: 'Type specimen', secondary: 'Graphic' },
] as const;

const ROWS = [
  { id: 'row-1', name: 'Astronaut poster', type: 'Illustration' },
  { id: 'row-2', name: 'Editorial study', type: 'Photography' },
] as const;

const COMMANDS = [
  { id: 'command.open', label: 'Open selected asset', description: 'Open the current preview', scope: 'all' as const },
  { id: 'command.pin', label: 'Pin selected asset', description: 'Keep the asset visible', scope: 'current' as const },
] as const;

const QUICK_SEARCH_TABS = [
  { id: 'folders' as const, label: 'Folders' },
  { id: 'items' as const, label: 'Items' },
  { id: 'tags' as const, label: 'Tags' },
  { id: 'smart-folders' as const, label: 'Smart Folders' },
];

const QUICK_SEARCH_RESULTS = [
  { id: 'quick.item.beijing', kind: 'items' as const, label: '北京海报' },
  { id: 'quick.folder.reference', kind: 'folders' as const, label: '参考资料' },
  { id: 'quick.tag.inspiration', kind: 'tags' as const, label: '灵感' },
  { id: 'quick.smart.recent', kind: 'smart-folders' as const, label: '最近添加' },
];

const FILTER_FACETS = [
  { id: 'type', label: 'Type', kind: 'multi' as const, options: [{ id: 'image', label: 'Image', count: 12 }, { id: 'video', label: 'Video', count: 2 }] },
  { id: 'folder', label: 'Folder', semantic: 'folder' as const, kind: 'single' as const, options: [{ id: 'reference', label: 'Reference', count: 8 }, { id: 'inspiration', label: 'Inspiration', count: 4 }] },
  { id: 'tag', label: 'Tag', semantic: 'tag' as const, kind: 'multi' as const, options: [{ id: 'illustration', label: 'Illustration', count: 5 }] },
  { id: 'color', label: 'Color', semantic: 'color' as const, kind: 'single' as const, options: [{ id: 'blue', label: 'Blue', count: 6 }] },
  { id: 'rating', label: 'Rating', semantic: 'rating' as const, kind: 'range' as const, placeholder: '0–5' },
  { id: 'date', label: 'Date', semantic: 'date' as const, kind: 'range' as const, placeholder: 'YYYY-MM-DD..YYYY-MM-DD' },
  { id: 'annotation', label: 'Annotation', kind: 'text' as const, placeholder: 'Contains…' },
] as const;

const FOLDER_FACET_OPTIONS: readonly VelvetFilterFolderOption[] = [
  { id: 'reference', label: 'Reference', count: 12, depth: 0, state: 'idle' },
  { id: 'illustration', label: 'Illustration', count: 8, depth: 1, state: 'idle' },
  { id: 'archive', label: 'Archive', count: 3, depth: 0, state: 'idle' },
];

const COLOR_FACET_SWATCHES = [
  { id: 'black', color: '#111111', label: 'Black' },
  { id: 'white', color: '#FFFFFF', label: 'White' },
  { id: 'blue', color: '#2B76E7', label: 'Blue' },
  { id: 'purple', color: '#6D50ED', label: 'Purple' },
] as const;

const INSPECTOR_LANDSCAPE_PREVIEW = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180"><rect width="320" height="180" fill="#2b76e7"/><circle cx="250" cy="54" r="34" fill="#b9d4ff"/><path d="M0 145 90 75l58 42 62-58 110 86v35H0Z" fill="#142c63"/></svg>')}`;
const INSPECTOR_PORTRAIT_PREVIEW = `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 180 320"><rect width="180" height="320" fill="#fc85b3"/><circle cx="90" cy="90" r="48" fill="#ffd8e8"/><path d="M0 260 52 180l38 38 35-54 55 68v88H0Z" fill="#6c214d"/></svg>')}`;

type InspectorScenario = 'landscape' | 'portrait' | 'no-preview' | 'multiple';

function createInspectorSections(scenario: InspectorScenario, name: string): VelvetInspectorSection[] {
  if (scenario === 'multiple') {
    return [
      { id: 'metadata', label: 'Metadata', fields: [{ id: 'selection', label: 'Selection', value: '2 assets' }, { id: 'dimensions', label: 'Dimensions', value: 'Mixed' }], expanded: true },
      { id: 'source', label: 'Source', fields: [{ id: 'folder', label: 'Folder', value: 'Reference' }], expanded: true },
    ];
  }
  if (scenario === 'no-preview') {
    return [
      { id: 'metadata', label: 'Metadata', fields: [{ id: 'type', label: 'Type', value: 'Raw file' }, { id: 'preview', label: 'Preview', value: 'Unavailable' }], expanded: true },
      { id: 'source', label: 'Source', fields: [{ id: 'folder', label: 'Folder', value: 'Archive' }], expanded: true },
    ];
  }
  const portrait = scenario === 'portrait';
  return [
    { id: 'metadata', label: 'Metadata', fields: [{ id: 'name', label: 'Name', value: name, inputValue: name, editable: true }, { id: 'dimensions', label: 'Dimensions', value: portrait ? '180 × 320' : '320 × 180' }], expanded: true },
    { id: 'source', label: 'Source', fields: [{ id: 'folder', label: 'Folder', value: portrait ? 'Portraits' : 'Reference' }], expanded: true },
  ];
}

export function HostProjectionQualificationFixture() {
  const [selectedAsset, setSelectedAsset] = useState('asset-1');
  const [activeWorkspace, setActiveWorkspace] = useState('library');
  const [openedAsset, setOpenedAsset] = useState('');
  const [assetSelectionMode, setAssetSelectionMode] = useState('replace:single');
  const [selectedRow, setSelectedRow] = useState('');
  const [sortedBy, setSortedBy] = useState('');
  const [tableSort, setTableSort] = useState<{ columnId: string; direction: 'ascending' | 'descending' } | undefined>();
  const [selectedCommand, setSelectedCommand] = useState('');
  const [commandScope, setCommandScope] = useState<'all' | 'current'>('all');
  const [commandQuery, setCommandQuery] = useState('');
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [quickSearchOpen, setQuickSearchOpen] = useState(false);
  const [quickSearchTab, setQuickSearchTab] = useState<'folders' | 'items' | 'tags' | 'smart-folders'>('items');
  const [quickSearchQuery, setQuickSearchQuery] = useState('');
  const [quickSearchSelection, setQuickSearchSelection] = useState('');
  const [quickSearchSelectionKind, setQuickSearchSelectionKind] = useState('');
  const [inspectorPinned, setInspectorPinned] = useState(false);
  const [inspectorName, setInspectorName] = useState('Astronaut poster');
  const [inspectorSaveState, setInspectorSaveState] = useState<'idle' | 'saved'>('idle');
  const [previewState, setPreviewState] = useState<'error' | 'ready'>('error');
  const [previewMode, setPreviewMode] = useState<'fit' | 'actual'>('fit');
  const [previewProviderAction, setPreviewProviderAction] = useState('');
  const [previewExternalOpened, setPreviewExternalOpened] = useState(false);
  const [mainPreviewOpen, setMainPreviewOpen] = useState(false);
  const [mainPreviewState, setMainPreviewState] = useState<'loading' | 'ready' | 'empty' | 'error'>('empty');
  const [mainPreviewAsset, setMainPreviewAsset] = useState<'landscape' | 'portrait' | 'no-preview'>('landscape');
  const [mainPreviewMode, setMainPreviewMode] = useState<'fit' | 'actual'>('fit');
  const [zoom, setZoom] = useState(180);
  const [filterRules, setFilterRules] = useState<readonly VelvetFilterRule[]>([]);
  const [savedFilterRules, setSavedFilterRules] = useState<readonly VelvetFilterRule[]>([]);
  const [appliedFilterRules, setAppliedFilterRules] = useState<readonly VelvetFilterRule[]>([]);
  const [filterClosed, setFilterClosed] = useState(false);
  const [facetMenu, setFacetMenu] = useState<'folder' | 'color' | null>(null);
  const [folderFacetOptions, setFolderFacetOptions] = useState(FOLDER_FACET_OPTIONS);
  const [folderFacetRule, setFolderFacetRule] = useState<VelvetFilterFolderRule>('OR');
  const [folderFacetQuery, setFolderFacetQuery] = useState('');
  const [colorFacetHex, setColorFacetHex] = useState('#FFFFFF');
  const [colorFacetAccuracy, setColorFacetAccuracy] = useState(20);
  const [inspectorScenario, setInspectorScenario] = useState<InspectorScenario>('landscape');
  const [inspectorSections, setInspectorSections] = useState<VelvetInspectorSection[]>(() => createInspectorSections('landscape', inspectorName));
  const paletteTriggerRef = useRef<HTMLButtonElement>(null);
  const quickSearchTriggerRef = useRef<HTMLButtonElement>(null);
  const folderFacetTriggerRef = useRef<HTMLButtonElement>(null);
  const colorFacetTriggerRef = useRef<HTMLButtonElement>(null);
  const mainPreviewTriggerRef = useRef<HTMLButtonElement>(null);
  const hostCommands = COMMANDS.filter((command) => commandScope === 'all' || command.scope === undefined || command.scope === commandScope).filter((command) => commandQuery.length === 0 || `${command.label} ${command.description ?? ''}`.toLowerCase().includes(commandQuery.toLowerCase()));
  const hostQuickSearchResults = QUICK_SEARCH_RESULTS.filter((result) => result.kind === quickSearchTab);
  const setMainPreview = (asset: 'landscape' | 'portrait' | 'no-preview'): void => {
    setMainPreviewAsset(asset);
    setMainPreviewOpen(true);
    setMainPreviewState(asset === 'no-preview' ? 'empty' : 'loading');
  };
  const selectInspectorScenario = (scenario: InspectorScenario): void => {
    setInspectorScenario(scenario);
    setInspectorName(scenario === 'portrait' ? 'Portrait study' : 'Astronaut poster');
    setInspectorSaveState('idle');
    setInspectorSections(createInspectorSections(scenario, scenario === 'portrait' ? 'Portrait study' : 'Astronaut poster'));
  };
  const inspectorSelectedCount = inspectorScenario === 'multiple' ? 2 : 1;
  const inspectorPreview = inspectorScenario === 'landscape'
    ? <div data-ui-kit-probe="inspector-preview"><img alt="Landscape asset preview" height={180} src={INSPECTOR_LANDSCAPE_PREVIEW} width={320} /></div>
    : inspectorScenario === 'portrait'
      ? <div data-ui-kit-probe="inspector-preview"><img alt="Portrait asset preview" height={320} src={INSPECTOR_PORTRAIT_PREVIEW} width={180} /></div>
      : undefined;
  const inspectorPalette = inspectorScenario === 'landscape'
    ? [{ id: 'blue', color: '#2B76E7', ratio: 42.5, label: 'Blue' }, { id: 'gray', color: '#8D95A2', ratio: 18.25, label: 'Gray' }, { id: 'pink', color: '#FC85B3', ratio: 9.5, label: 'Pink' }]
    : inspectorScenario === 'portrait'
      ? [{ id: 'pink', color: '#FC85B3', ratio: 61.25, label: 'Pink' }, { id: 'purple', color: '#6D50ED', ratio: 14.5, label: 'Purple' }]
      : [];

  return (
    <main aria-label="Velvet host projection qualification fixture" data-ui-kit-fixture="host-projections" data-ui-kit-viewport="1280x800" style={{ display: 'grid', gap: 24, padding: 24, minHeight: '100%', background: 'var(--velvet-surface-app)' }}>
      <VelvetStack gap="sm">
        <VelvetText as="h1">Host projection qualification</VelvetText>
        <VelvetText tone="muted">Keyboard and focus contract fixture</VelvetText>
      </VelvetStack>
      <section aria-label="Activity bar qualification" data-ui-kit-probe="activity-bar">
        <VelvetActivityBar
          ariaLabel="Qualification workspaces"
          items={[{ id: 'library', label: 'Library', icon: '▦', active: activeWorkspace === 'library' }, { id: 'settings', label: 'Settings', icon: '⚙', disabled: true }, { id: 'feed', label: 'Feed', icon: '◌', active: activeWorkspace === 'feed' }]}
          onSelect={setActiveWorkspace}
        />
        <output data-ui-kit-observation="activity-bar-state">selected:{activeWorkspace}</output>
      </section>
      <section aria-label="Pane qualification" data-ui-kit-probe="pane">
        <VelvetPane region="navigation" title="Qualification navigation">Pane content</VelvetPane>
      </section>
      <section aria-label="Panel qualification" data-ui-kit-probe="panel">
        <VelvetPanel surface="raised" title="Qualification panel">Panel content</VelvetPanel>
      </section>
      <section aria-label="Preview host qualification" data-ui-kit-probe="preview-host">
        <VelvetPreviewHost
          mode={previewMode}
          message="Provider failed"
          onModeChange={setPreviewMode}
          onOpenExternal={() => setPreviewExternalOpened(true)}
          onProviderAction={setPreviewProviderAction}
          onRetry={() => setPreviewState('ready')}
          providerActions={previewState === 'ready' ? [{ id: 'rotate-right', label: 'Rotate', pressed: previewProviderAction === 'rotate-right' }, { id: 'slideshow', label: 'Slideshow', disabled: true }] : undefined}
          state={previewState}
          toolbar={previewState === 'ready' ? <button data-preview-toolbar-action="rotate" type="button">Rotate</button> : undefined}
        >
          <VelvetText>Preview ready</VelvetText>
        </VelvetPreviewHost>
        <output data-ui-kit-observation="preview-host-state">{previewState}</output>
        <output data-ui-kit-observation="preview-provider-action">{previewProviderAction}</output>
        <output data-ui-kit-observation="preview-external-opened">{String(previewExternalOpened)}</output>
      </section>
      <section aria-label="Main window preview lifecycle qualification" data-ui-kit-probe="main-window-preview">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Main window preview lifecycle</VelvetText>
          <VelvetStack direction="row" gap="sm">
            <VelvetButton onClick={() => setMainPreview('landscape')} ref={mainPreviewTriggerRef} testId="main-preview-open-landscape">Open landscape</VelvetButton>
            <VelvetButton onClick={() => setMainPreview('portrait')} testId="main-preview-open-portrait" variant="quiet">Open portrait</VelvetButton>
            <VelvetButton onClick={() => setMainPreview('no-preview')} testId="main-preview-open-no-preview" variant="quiet">Open no-preview</VelvetButton>
            {mainPreviewOpen && mainPreviewState === 'loading' ? <VelvetButton onClick={() => setMainPreviewState('ready')} testId="main-preview-resolve" variant="quiet">Resolve</VelvetButton> : null}
            {mainPreviewOpen && mainPreviewState !== 'loading' ? <VelvetButton onClick={() => setMainPreviewState('error')} testId="main-preview-fail" variant="quiet">Fail</VelvetButton> : null}
          </VelvetStack>
          {mainPreviewOpen ? <VelvetPreviewHost
            mode={mainPreviewMode}
            message={mainPreviewState === 'error' ? 'Preview provider failed' : mainPreviewState === 'empty' ? 'No preview available' : undefined}
            onClose={() => { setMainPreviewOpen(false); setMainPreviewState('empty'); mainPreviewTriggerRef.current?.focus(); }}
            onModeChange={setMainPreviewMode}
            onRetry={() => setMainPreviewState('loading')}
            state={mainPreviewState}
          >
            {mainPreviewState === 'ready' ? <img alt={`${mainPreviewAsset} main window preview`} data-main-preview-asset={mainPreviewAsset} height={mainPreviewAsset === 'portrait' ? 320 : 180} src={mainPreviewAsset === 'portrait' ? INSPECTOR_PORTRAIT_PREVIEW : INSPECTOR_LANDSCAPE_PREVIEW} width={mainPreviewAsset === 'portrait' ? 180 : 320} /> : null}
          </VelvetPreviewHost> : <p data-main-preview-closed="true">Preview closed</p>}
          <output data-ui-kit-observation="main-preview-state">{mainPreviewOpen ? `${mainPreviewState};asset:${mainPreviewAsset}` : 'closed'}</output>
        </VelvetStack>
      </section>
      <section aria-label="Zoom control qualification" data-ui-kit-probe="zoom-control">
        <VelvetZoomControl max={500} min={25} onChange={setZoom} step={5} value={zoom} />
        <output data-ui-kit-observation="zoom-control-value">{zoom}</output>
      </section>
      <section aria-label="Asset grid qualification" data-ui-kit-probe="asset-grid">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Asset Grid</VelvetText>
          <VelvetAssetGrid ariaLabel="Qualification assets" columns={2} items={ASSETS.map((asset) => ({ ...asset, selected: asset.id === selectedAsset }))} onOpen={setOpenedAsset} onSelect={(id, additive, range) => { setSelectedAsset(id); setAssetSelectionMode(`${additive ? 'additive' : 'replace'}:${range ? 'range' : 'single'}`); }} />
          <output data-ui-kit-observation="asset-selection">selected:{selectedAsset};opened:{openedAsset}</output>
          <output data-ui-kit-observation="asset-selection-mode">{assetSelectionMode}</output>
        </VelvetStack>
      </section>
      <section aria-label="Data table qualification" data-ui-kit-probe="data-table">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Data Table</VelvetText>
          <VelvetDataTable ariaLabel="Qualification rows" columns={[{ id: 'name', label: 'Name', render: (row) => row.name, sortable: true }, { id: 'type', label: 'Type', render: (row) => row.type }]} onSelect={setSelectedRow} onSort={(columnId, direction) => { setSortedBy(columnId); setTableSort({ columnId, direction }); }} rowId={(row) => row.id} rows={ROWS} selectedId={selectedRow} sort={tableSort} />
          <output data-ui-kit-observation="table-selection">selected:{selectedRow};sorted:{sortedBy}</output>
        </VelvetStack>
      </section>
      <section aria-label="Command palette qualification" data-ui-kit-probe="command-palette">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Command Palette</VelvetText>
          <VelvetButton onClick={() => setPaletteOpen(true)} ref={paletteTriggerRef} testId="command-palette-trigger">Open commands</VelvetButton>
          <VelvetCommandPalette commands={hostCommands} onClose={() => setPaletteOpen(false)} onQueryChange={setCommandQuery} onScopeChange={setCommandScope} onSelect={(id) => { setSelectedCommand(id); setPaletteOpen(false); }} open={paletteOpen} query={commandQuery} restoreFocusRef={paletteTriggerRef} scope={commandScope} scopeOptions={[{ id: 'all', label: 'All' }, { id: 'current', label: 'Current' }]} />
          <output data-ui-kit-observation="command-selection">selected:{selectedCommand}</output>
          <output data-ui-kit-observation="command-query">{commandQuery}</output>
        </VelvetStack>
      </section>
      <section aria-label="Quick Search qualification" data-ui-kit-probe="quick-search">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Quick Search</VelvetText>
          <VelvetButton onClick={() => { setQuickSearchQuery(''); setQuickSearchOpen(true); }} ref={quickSearchTriggerRef} testId="quick-search-trigger">Open quick search</VelvetButton>
          <VelvetQuickSearch
            activeTab={quickSearchTab}
            history={['北京', '灵感']}
            onClose={() => setQuickSearchOpen(false)}
            onHistorySelect={setQuickSearchQuery}
            onQueryChange={setQuickSearchQuery}
            onSelect={(id, kind) => { setQuickSearchSelection(id); setQuickSearchSelectionKind(kind); setQuickSearchOpen(false); }}
            onTabChange={setQuickSearchTab}
            open={quickSearchOpen}
            query={quickSearchQuery}
            restoreFocusRef={quickSearchTriggerRef}
            results={hostQuickSearchResults}
            tabs={QUICK_SEARCH_TABS}
          />
          <output data-ui-kit-observation="quick-search-query">{quickSearchQuery}</output>
          <output data-ui-kit-observation="quick-search-tab">{quickSearchTab}</output>
          <output data-ui-kit-observation="quick-search-selection">{quickSearchSelection};{quickSearchSelectionKind}</output>
          <output data-ui-kit-observation="quick-search-history">北京,灵感</output>
        </VelvetStack>
      </section>
      <section aria-label="Filter panel qualification" data-ui-kit-probe="filter-panel">
        <VelvetFilterPanel
          facets={FILTER_FACETS}
          onApply={setAppliedFilterRules}
          onClose={() => setFilterClosed(true)}
          onReset={() => setFilterRules([])}
          onRuleChange={setFilterRules}
          onSave={setSavedFilterRules}
          resultCount={12}
          rules={filterRules}
        />
        <output data-ui-kit-observation="filter-rules">{JSON.stringify(filterRules)}</output>
        <output data-ui-kit-observation="filter-saved">{JSON.stringify(savedFilterRules)}</output>
        <output data-ui-kit-observation="filter-applied">{JSON.stringify(appliedFilterRules)}</output>
        <output data-ui-kit-observation="filter-closed">{String(filterClosed)}</output>
      </section>
      <section aria-label="Filter facet menu qualification" data-ui-kit-probe="filter-facet-menu">
        <VelvetStack direction="row" gap="sm">
          <VelvetButton onClick={() => setFacetMenu('folder')} ref={folderFacetTriggerRef} testId="folder-facet-trigger">Open folder facet</VelvetButton>
          <VelvetButton onClick={() => setFacetMenu('color')} ref={colorFacetTriggerRef} testId="color-facet-trigger">Open color facet</VelvetButton>
        </VelvetStack>
        {facetMenu === 'folder' ? <VelvetFilterFacetMenu
          kind="folder"
          onClear={() => setFolderFacetOptions(FOLDER_FACET_OPTIONS)}
          onClose={() => { setFacetMenu(null); folderFacetTriggerRef.current?.focus(); }}
          onOptionAction={(id, action) => setFolderFacetOptions((options) => options.map((option) => option.id === id ? { ...option, state: action === 'include' ? 'included' : 'excluded' } : option))}
          onQueryChange={setFolderFacetQuery}
          onRuleChange={setFolderFacetRule}
          open
          options={folderFacetOptions}
          query={folderFacetQuery}
          rule={folderFacetRule}
          title="Folders"
        /> : null}
        {facetMenu === 'color' ? <VelvetFilterFacetMenu
          accuracy={colorFacetAccuracy}
          hexValue={colorFacetHex}
          kind="color"
          onAccuracyChange={setColorFacetAccuracy}
          onClose={() => { setFacetMenu(null); colorFacetTriggerRef.current?.focus(); }}
          onHexChange={setColorFacetHex}
          onPresetChange={(value) => { if (value && value !== 'gray') setColorFacetHex(value); }}
          open
          selectedColor={colorFacetHex}
          swatches={COLOR_FACET_SWATCHES}
          title="Color"
        /> : null}
        <output data-ui-kit-observation="facet-menu-state">{facetMenu ?? 'closed'}</output>
        <output data-ui-kit-observation="facet-menu-folder">{folderFacetOptions.map((option) => `${option.id}:${option.state}`).join('|')}</output>
        <output data-ui-kit-observation="facet-menu-color">{colorFacetHex};accuracy:{colorFacetAccuracy}</output>
      </section>
      <section aria-label="Inspector qualification" data-ui-kit-probe="inspector">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Inspector</VelvetText>
          <VelvetStack direction="row" gap="sm">
            <VelvetButton onClick={() => selectInspectorScenario('landscape')} testId="inspector-scenario-landscape" variant={inspectorScenario === 'landscape' ? 'default' : 'quiet'}>Landscape</VelvetButton>
            <VelvetButton onClick={() => selectInspectorScenario('portrait')} testId="inspector-scenario-portrait" variant={inspectorScenario === 'portrait' ? 'default' : 'quiet'}>Portrait</VelvetButton>
            <VelvetButton onClick={() => selectInspectorScenario('no-preview')} testId="inspector-scenario-no-preview" variant={inspectorScenario === 'no-preview' ? 'default' : 'quiet'}>No preview</VelvetButton>
            <VelvetButton onClick={() => selectInspectorScenario('multiple')} testId="inspector-scenario-multiple" variant={inspectorScenario === 'multiple' ? 'default' : 'quiet'}>Multiple</VelvetButton>
          </VelvetStack>
          <VelvetInspector
            data-ui-kit-observation={inspectorScenario}
            fields={inspectorScenario === 'landscape' ? [{ id: 'name', label: 'Name', value: inspectorName, inputValue: inspectorName, editable: true }, { id: 'type', label: 'Type', value: 'Illustration' }] : undefined}
            onFieldChange={(fieldId, value) => { if (fieldId === 'name') { setInspectorName(value); setInspectorSections((sections) => sections.map((section) => ({ ...section, fields: section.fields.map((field) => field.id === 'name' ? { ...field, value, inputValue: value } : field) }))); setInspectorSaveState('idle'); } }}
            onPin={setInspectorPinned}
            onSave={() => setInspectorSaveState('saved')}
            onSectionToggle={(sectionId, expanded) => setInspectorSections((sections) => sections.map((section) => section.id === sectionId ? { ...section, expanded } : section))}
            pinned={inspectorPinned}
            palette={inspectorPalette}
            saveState={inspectorSaveState}
            sections={inspectorSections}
            selectedCount={inspectorSelectedCount}
            title="Inspector"
            preview={inspectorPreview}
          >
          </VelvetInspector>
          <output data-ui-kit-observation="inspector-state">name:{inspectorName};pinned:{String(inspectorPinned)};save:{inspectorSaveState}</output>
          <output data-ui-kit-observation="inspector-scenario">{inspectorScenario}</output>
        </VelvetStack>
      </section>
    </main>
  );
}
