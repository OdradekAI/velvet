import { useMemo, useRef, useState } from 'react';
import {
  VelvetAssetGrid,
  VelvetBadge,
  VelvetButton,
  VelvetContextMenu,
  VelvetDialog,
  VelvetIconButton,
  VelvetInput,
  VelvetInspector,
  VelvetPane,
  VelvetProgress,
  VelvetSelect,
  VelvetSeparator,
  VelvetStack,
  VelvetText,
} from '../ui/velvet-ui.js';

interface FixtureAsset {
  readonly id: string;
  readonly label: string;
  readonly secondary: string;
  readonly thumbnailUrl: string;
}

const FIXTURE_ASSETS: readonly FixtureAsset[] = [
  ['astronaut', 'Astronaut poster', 'Illustration · 2.4 MB', '#4338ca', '#f59e0b'],
  ['editorial', 'Editorial study', 'Photography · 1.8 MB', '#7c3aed', '#fbcfe8'],
  ['landscape', 'Night landscape', 'Photography · 4.1 MB', '#0f766e', '#67e8f9'],
  ['portrait', 'Portrait study', 'Photography · 3.2 MB', '#be123c', '#fed7aa'],
  ['typography', 'Type specimen', 'Graphic · 680 KB', '#1d4ed8', '#bfdbfe'],
  ['interior', 'Interior reference', 'Photography · 2.9 MB', '#854d0e', '#fde68a'],
  ['product', 'Product board', 'Reference · 1.2 MB', '#334155', '#cbd5e1'],
  ['motion', 'Motion frame', 'Video · 8.7 MB', '#166534', '#bbf7d0'],
  ['architecture', 'Architecture', 'Photography · 5.6 MB', '#7e22ce', '#e9d5ff'],
].map(([id, label, secondary, from, to]) => ({
  id,
  label,
  secondary,
  thumbnailUrl: `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 240"><defs><linearGradient id="g" x1="0" x2="1" y1="0" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect width="320" height="240" fill="url(#g)"/><circle cx="252" cy="62" r="48" fill="rgba(255,255,255,.24)"/><path d="M0 202 92 128l58 42 64-58 106 92v36H0Z" fill="rgba(0,0,0,.3)"/><text x="18" y="34" fill="rgba(255,255,255,.86)" font-family="Arial,sans-serif" font-size="15" letter-spacing="2">VELVET</text></svg>`)}`,
}));

const LIBRARY_ENTRIES = [
  ['all', 'All', '2,603'],
  ['uncategorized', 'Uncategorized', '30'],
  ['untagged', 'Untagged', '1,160'],
  ['recent', 'Recently Used', ''],
  ['random', 'Random', ''],
  ['community', 'Community', ''],
  ['tags', 'All Tags', '25'],
  ['trash', 'Trash', '137'],
] as const;

function libraryRowIcon(id: string): string {
  switch (id) {
    case 'recent': return '◷';
    case 'random': return '⌁';
    case 'community': return '▣';
    case 'tags': return '◇';
    case 'trash': return '▱';
    default: return '□';
  }
}

export function VisualQualificationFixture() {
  const [selectedId, setSelectedId] = useState('editorial');
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState('recent');
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogAction, setDialogAction] = useState('idle');
  const [contextAction, setContextAction] = useState('idle');
  const [inspectorPinned, setInspectorPinned] = useState(false);
  const dialogTriggerRef = useRef<HTMLButtonElement>(null);

  const assets = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return FIXTURE_ASSETS
      .filter((asset) => normalizedQuery.length === 0 || asset.label.toLocaleLowerCase().includes(normalizedQuery))
      .map((asset) => ({ ...asset, selected: asset.id === selectedId }));
  }, [query, selectedId]);

  const selectedAsset = FIXTURE_ASSETS.find((asset) => asset.id === selectedId) ?? FIXTURE_ASSETS[0];

  return (
    <main
      aria-label="Velvet visual qualification fixture"
      className="velvet-visual-fixture"
      data-ui-kit-fixture="eagle-baseline"
      data-ui-kit-viewport="1280x800"
    >
      <header className="velvet-visual-fixture__title" data-ui-kit-probe="title-bar">
        <VelvetText as="strong">Graphic Design</VelvetText>
        <VelvetText tone="muted">Library</VelvetText>
        <VelvetStack direction="row" gap="xs" className="velvet-visual-fixture__title-actions">
          <VelvetIconButton icon="⌘K" label="Quick search" variant="quiet" />
          <VelvetIconButton icon="⋯" label="More actions" variant="quiet" />
          <VelvetButton data-ui-kit-probe="dialog-trigger" onClick={() => { setDialogAction('idle'); setDialogOpen(true); }} ref={dialogTriggerRef} variant="quiet">Open dialog</VelvetButton>
        </VelvetStack>
      </header>

      <div className="velvet-visual-fixture__layout">
        <VelvetPane region="navigation" title="Library" className="velvet-visual-fixture__navigation" data-ui-kit-probe="navigation">
          <VelvetStack gap="sm">
            <VelvetStack direction="row" gap="xs" className="velvet-visual-fixture__library-switcher">
              <VelvetButton className="velvet-visual-fixture__library-name" variant="quiet">Graphic Design⌄</VelvetButton>
              <VelvetIconButton icon="+" label="Create folder" variant="quiet" />
            </VelvetStack>
            <VelvetSeparator />
            <nav aria-label="Library navigation" className="velvet-visual-fixture__library-list">
              {LIBRARY_ENTRIES.map(([id, label, count]) => (
                <VelvetButton
                  aria-current={id === 'all' ? 'page' : undefined}
                  aria-pressed={id === 'all'}
                  className="velvet-visual-fixture__library-row"
                  key={id}
                  variant="quiet"
                >
                  <span aria-hidden="true" className="velvet-visual-fixture__library-icon">{libraryRowIcon(id)}</span>
                  <span>{label}</span>
                  {count ? <VelvetBadge>{count}</VelvetBadge> : null}
                </VelvetButton>
              ))}
            </nav>
            <VelvetText as="small" tone="muted" className="velvet-visual-fixture__section-label">Smart Folders (4)</VelvetText>
            <VelvetButton className="velvet-visual-fixture__library-row" variant="quiet">
              <span aria-hidden="true" className="velvet-visual-fixture__library-icon">⌁</span>
              <span>Inspiration</span>
              <VelvetBadge tone="info">91</VelvetBadge>
            </VelvetButton>
            <VelvetText as="small" tone="muted" className="velvet-visual-fixture__section-label">Folders (63)</VelvetText>
            {['Poster Design', 'Illustration', 'Packaging Design'].map((folder) => (
              <VelvetButton className="velvet-visual-fixture__library-row velvet-visual-fixture__library-row--folder" key={folder} variant="quiet">
                <span aria-hidden="true" className="velvet-visual-fixture__library-icon">▸</span>
                <span>{folder}</span>
              </VelvetButton>
            ))}
          </VelvetStack>
        </VelvetPane>

        <section aria-label="Asset browser" className="velvet-visual-fixture__workspace" data-ui-kit-region="preview" data-ui-kit-probe="workspace">
          <div className="velvet-visual-fixture__toolbar" data-ui-kit-probe="toolbar">
            <VelvetStack direction="row" gap="xs">
              <VelvetIconButton icon="←" label="Back" variant="quiet" />
              <VelvetIconButton icon="→" label="Forward" variant="quiet" />
            </VelvetStack>
            <VelvetInput ariaLabel="Search assets" onChange={(event) => setQuery(event.target.value)} placeholder="Search" value={query} />
            <VelvetIconButton icon="⌕" label="Search" variant="quiet" />
            <VelvetIconButton icon="≡" label="Filter" variant="quiet" />
            <VelvetContextMenu actions={[{ id: 'pin', label: 'Pin selection', checked: true, onSelect: () => setContextAction('pin') }, { id: 'open', label: 'Open in preview', onSelect: () => setContextAction('open'), shortcutHint: 'Enter' }]}>
              <div className="velvet-visual-fixture__context-trigger" data-ui-kit-probe="context-trigger" role="button" tabIndex={0}>Actions</div>
            </VelvetContextMenu>
            <VelvetSelect ariaLabel="Sort assets" onChange={(event) => setSort(event.target.value)} value={sort}>
              <option value="recent">Recent</option>
              <option value="name">Name</option>
            </VelvetSelect>
            <VelvetStack direction="row" gap="xs" className="velvet-visual-fixture__zoom-controls">
              <VelvetIconButton icon="−" label="Zoom out" variant="quiet" />
              <VelvetText tone="muted">180%</VelvetText>
              <VelvetIconButton icon="+" label="Zoom in" variant="quiet" />
            </VelvetStack>
          </div>
          <div className="velvet-visual-fixture__facet-strip" data-ui-kit-probe="facets">
            <VelvetBadge tone="info">All</VelvetBadge>
            <VelvetBadge>Illustration</VelvetBadge>
            <VelvetBadge>Photography</VelvetBadge>
            <VelvetBadge>Recent</VelvetBadge>
          </div>
          <VelvetSeparator />
          <div className="velvet-visual-fixture__grid-wrap" data-ui-kit-probe="grid">
            <VelvetAssetGrid columns={3} items={assets} onSelect={(id) => setSelectedId(id)} onOpen={(id) => setSelectedId(id)} />
          </div>
          <section aria-label="Operations" className="velvet-visual-fixture__operations" data-ui-kit-region="operations" data-ui-kit-probe="operations">
            <VelvetText as="small" tone="muted">Indexing library</VelvetText>
            <VelvetProgress label="Assets" value={72} />
          </section>
        </section>

        <VelvetInspector
          className="velvet-visual-fixture__inspector"
          data-ui-kit-probe="inspector"
          fields={[
            { id: 'folder', label: 'Folder', value: 'Illustration' },
            { id: 'tags', label: 'Tags', value: <VelvetBadge tone="info">reference</VelvetBadge> },
            { id: 'added', label: 'Added', value: '2026/11/06 14:28' },
          ]}
          onOpen={() => setSelectedId(selectedId)}
          onPin={setInspectorPinned}
          pinned={inspectorPinned}
          preview={<div data-ui-kit-probe="selection-preview"><img alt="" src={selectedAsset.thumbnailUrl} /></div>}
          selectedCount={1}
          title="Inspector"
        >
          <VelvetStack gap="xs">
            <VelvetText as="strong">{selectedAsset.label}</VelvetText>
            <VelvetText tone="muted">{selectedAsset.secondary}</VelvetText>
          </VelvetStack>
        </VelvetInspector>
      </div>
      <output data-ui-kit-observation="dialog-action">dialog:{dialogAction}</output>
      <output data-ui-kit-observation="context-action">context:{contextAction}</output>
      <VelvetDialog
        description="Dialog focus and dismissal are qualified in the real browser fixture."
        actions={[{ id: 'apply', label: 'Apply', onSelect: () => setDialogAction('submitted'), primary: true }]}
        closeLabel="Cancel"
        onOpenChange={(open) => { setDialogOpen(open); if (!open) setDialogAction('cancelled'); }}
        open={dialogOpen}
        restoreFocusRef={dialogTriggerRef}
        title="Asset actions"
      >
        <VelvetText tone="secondary">The selected asset is ready for preview.</VelvetText>
      </VelvetDialog>
    </main>
  );
}
