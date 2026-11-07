import { useRef, useState } from 'react';
import {
  VelvetAssetGrid,
  VelvetButton,
  VelvetCommandPalette,
  VelvetDataTable,
  VelvetInspector,
  VelvetStack,
  VelvetText,
} from '../ui/index.js';

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
  { id: 'command.open', label: 'Open selected asset', description: 'Open the current preview' },
  { id: 'command.pin', label: 'Pin selected asset', description: 'Keep the asset visible' },
] as const;

export function HostProjectionQualificationFixture() {
  const [selectedAsset, setSelectedAsset] = useState('asset-1');
  const [openedAsset, setOpenedAsset] = useState('');
  const [selectedRow, setSelectedRow] = useState('');
  const [sortedBy, setSortedBy] = useState('');
  const [selectedCommand, setSelectedCommand] = useState('');
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [inspectorPinned, setInspectorPinned] = useState(false);
  const [inspectorName, setInspectorName] = useState('Astronaut poster');
  const [inspectorSaveState, setInspectorSaveState] = useState<'idle' | 'saved'>('idle');
  const paletteTriggerRef = useRef<HTMLButtonElement>(null);

  return (
    <main aria-label="Velvet host projection qualification fixture" data-ui-kit-fixture="host-projections" data-ui-kit-viewport="1280x800" style={{ display: 'grid', gap: 24, padding: 24, minHeight: '100%', background: 'var(--velvet-surface-app)' }}>
      <VelvetStack gap="sm">
        <VelvetText as="h1">Host projection qualification</VelvetText>
        <VelvetText tone="muted">Keyboard and focus contract fixture</VelvetText>
      </VelvetStack>
      <section aria-label="Asset grid qualification" data-ui-kit-probe="asset-grid">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Asset Grid</VelvetText>
          <VelvetAssetGrid ariaLabel="Qualification assets" columns={2} items={ASSETS.map((asset) => ({ ...asset, selected: asset.id === selectedAsset }))} onOpen={setOpenedAsset} onSelect={(id) => setSelectedAsset(id)} />
          <output data-ui-kit-observation="asset-selection">selected:{selectedAsset};opened:{openedAsset}</output>
        </VelvetStack>
      </section>
      <section aria-label="Data table qualification" data-ui-kit-probe="data-table">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Data Table</VelvetText>
          <VelvetDataTable ariaLabel="Qualification rows" columns={[{ id: 'name', label: 'Name', render: (row) => row.name, sortable: true }, { id: 'type', label: 'Type', render: (row) => row.type }]} onSelect={setSelectedRow} onSort={setSortedBy} rowId={(row) => row.id} rows={ROWS} selectedId={selectedRow} />
          <output data-ui-kit-observation="table-selection">selected:{selectedRow};sorted:{sortedBy}</output>
        </VelvetStack>
      </section>
      <section aria-label="Command palette qualification" data-ui-kit-probe="command-palette">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Command Palette</VelvetText>
          <VelvetButton onClick={() => setPaletteOpen(true)} ref={paletteTriggerRef} testId="command-palette-trigger">Open commands</VelvetButton>
          <VelvetCommandPalette commands={COMMANDS} onClose={() => setPaletteOpen(false)} onSelect={(id) => { setSelectedCommand(id); setPaletteOpen(false); }} open={paletteOpen} restoreFocusRef={paletteTriggerRef} />
          <output data-ui-kit-observation="command-selection">selected:{selectedCommand}</output>
        </VelvetStack>
      </section>
      <section aria-label="Inspector qualification" data-ui-kit-probe="inspector">
        <VelvetStack gap="sm">
          <VelvetText as="h2">Inspector</VelvetText>
          <VelvetInspector
            fields={[
              { id: 'name', label: 'Name', value: inspectorName, inputValue: inspectorName, editable: true },
              { id: 'type', label: 'Type', value: 'Illustration' },
            ]}
            onFieldChange={(fieldId, value) => { if (fieldId === 'name') { setInspectorName(value); setInspectorSaveState('idle'); } }}
            onPin={setInspectorPinned}
            onSave={() => setInspectorSaveState('saved')}
            pinned={inspectorPinned}
            saveState={inspectorSaveState}
            selectedCount={1}
            title="Inspector"
          >
            <span data-ui-kit-probe="inspector-preview">Preview: Astronaut poster</span>
          </VelvetInspector>
          <output data-ui-kit-observation="inspector-state">name:{inspectorName};pinned:{String(inspectorPinned)};save:{inspectorSaveState}</output>
        </VelvetStack>
      </section>
    </main>
  );
}
