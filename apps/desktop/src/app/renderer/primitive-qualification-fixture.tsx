import { useState } from 'react';
import {
  VelvetBadge,
  VelvetButton,
  VelvetCheckbox,
  VelvetIcon,
  VelvetIconButton,
  VelvetInline,
  VelvetInput,
  VelvetProgress,
  VelvetSelect,
  VelvetSeparator,
  VelvetStack,
  VelvetState,
  VelvetTabs,
  VelvetText,
  VelvetToastHost,
} from '../ui/index.js';

export function PrimitiveQualificationFixture() {
  const [activated, setActivated] = useState(0);
  const [inputValue, setInputValue] = useState('');
  const [selectValue, setSelectValue] = useState('recent');
  const [checked, setChecked] = useState(false);
  const [selectedTab, setSelectedTab] = useState('all');
  const [retried, setRetried] = useState(0);
  const [toasts, setToasts] = useState([{ id: 'toast-1', message: 'Saved', severity: 'info' as const }]);

  return (
    <main aria-label="Velvet primitives and overlays qualification fixture" data-ui-kit-fixture="primitives-overlays" data-ui-kit-viewport="1280x800" style={{ display: 'grid', gap: 16, padding: 24, minHeight: '100%', background: 'var(--velvet-surface-app)' }}>
      <section aria-label="Static primitives" data-ui-kit-probe="static-primitives">
        <VelvetStack gap="sm">
          <VelvetText as="h1">Primitive semantics</VelvetText>
          <VelvetInline data-ui-kit-probe="inline"><VelvetBadge tone="info">Info</VelvetBadge><VelvetIcon label="Favorite">★</VelvetIcon><VelvetText as="strong">Named content</VelvetText></VelvetInline>
          <VelvetStack data-ui-kit-probe="stack"><VelvetText>Stack one</VelvetText><VelvetText>Stack two</VelvetText></VelvetStack>
          <VelvetSeparator orientation="vertical" />
          <VelvetProgress label="Progress" value={60} />
        </VelvetStack>
      </section>
      <section aria-label="Button qualification" data-ui-kit-probe="button">
        <VelvetStack direction="row" gap="sm">
          <VelvetButton ariaLabel="Run action" onClick={() => setActivated((value) => value + 1)}>Run</VelvetButton>
          <VelvetIconButton icon="+" label="Add item" onClick={() => setActivated((value) => value + 1)} />
          <VelvetButton ariaLabel="Busy action" busy>Working</VelvetButton>
        </VelvetStack>
        <output data-ui-kit-observation="button-state">{activated}</output>
      </section>
      <section aria-label="Native controls" data-ui-kit-probe="native-controls">
        <VelvetStack gap="sm">
          <VelvetInput ariaLabel="Primitive input" onChange={(event) => setInputValue(event.currentTarget.value)} value={inputValue} />
          <VelvetSelect ariaLabel="Primitive select" onChange={(event) => setSelectValue(event.currentTarget.value)} value={selectValue}>
            <option value="recent">Recent</option>
            <option value="name">Name</option>
          </VelvetSelect>
          <VelvetCheckbox checked={checked} label="Include archived" onCheckedChange={setChecked} />
        </VelvetStack>
        <output data-ui-kit-observation="native-values">{`input:${inputValue};select:${selectValue};checked:${String(checked)}`}</output>
      </section>
      <section aria-label="Tabs qualification" data-ui-kit-probe="tabs">
        <VelvetTabs onSelect={setSelectedTab} selectedId={selectedTab} tabs={[{ id: 'all', label: 'All' }, { id: 'recent', label: 'Recent' }, { id: 'disabled', label: 'Disabled', disabled: true }]} />
        <output data-ui-kit-observation="tab-state">{selectedTab}</output>
      </section>
      <section aria-label="State qualification" data-ui-kit-probe="state">
        <VelvetState kind="error" onRetry={() => setRetried((value) => value + 1)} title="Failed" />
        <output data-ui-kit-observation="state-retry">{retried}</output>
      </section>
      <section aria-label="Toast qualification" data-ui-kit-probe="toast">
        <VelvetToastHost items={toasts} onDismiss={(id) => setToasts((current) => current.filter((item) => item.id !== id))} />
        <output data-ui-kit-observation="toast-count">{String(toasts.length)}</output>
      </section>
    </main>
  );
}
