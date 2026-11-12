import { useState } from 'react';

import { VelvetButton } from '@velvet-desktop/app/ui/primitives/button';

export function ButtonPlayground() {
  const [label, setLabel] = useState('Save changes');
  const [variant, setVariant] = useState<'default' | 'quiet' | 'danger'>('default');
  const [busy, setBusy] = useState(false);
  const [disabled, setDisabled] = useState(false);
  const [lastAction, setLastAction] = useState('No action yet');

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview">
        <VelvetButton
          busy={busy}
          disabled={disabled}
          onClick={() => setLastAction('button.activate committed')}
          variant={variant}
        >
          {label}
        </VelvetButton>
        <span className="velvet-component-playground__status" role="status">{lastAction}</span>
      </div>
      <div className="velvet-component-playground__controls">
        <label>
          <span>label</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <label>
          <span>variant</span>
          <select value={variant} onChange={(event) => setVariant(event.currentTarget.value as typeof variant)}>
            <option value="default">default</option>
            <option value="quiet">quiet</option>
            <option value="danger">danger</option>
          </select>
        </label>
        <label className="velvet-component-playground__check">
          <input checked={busy} type="checkbox" onChange={(event) => setBusy(event.currentTarget.checked)} />
          <span>busy</span>
        </label>
        <label className="velvet-component-playground__check">
          <input checked={disabled} type="checkbox" onChange={(event) => setDisabled(event.currentTarget.checked)} />
          <span>disabled</span>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetButton variant="${variant}" busy={${busy}} disabled={${disabled}}>
  ${label || 'Button'}
</VelvetButton>`}</code></pre>
    </div>
  );
}
