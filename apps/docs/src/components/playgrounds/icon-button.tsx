import { useState } from 'react';

import { VelvetIcon, VelvetIconButton } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function IconButtonPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [label, setLabel] = useState(locale === 'en' ? 'Add asset' : '添加 Asset');
  const [busy, setBusy] = useState(false);
  const [disabled, setDisabled] = useState(false);
  const [status, setStatus] = useState(locale === 'en' ? 'No action yet' : '尚未触发 action');
  const labels = locale === 'en'
    ? {preview: 'Preview', label: 'label', busy: 'busy', disabled: 'disabled'}
    : {preview: '预览', label: 'label', busy: 'busy', disabled: 'disabled'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetIconButton
          busy={busy}
          disabled={disabled}
          icon={<VelvetIcon name="add" />}
          label={label || 'Icon button'}
          onClick={() => setStatus(locale === 'en' ? 'button.activate committed' : 'button.activate 已提交')}
        />
        <output className="velvet-component-playground__output">{status}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--wide">
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <label className="velvet-component-playground__check">
          <input checked={busy} type="checkbox" onChange={(event) => setBusy(event.currentTarget.checked)} />
          <span>{labels.busy}</span>
        </label>
        <label className="velvet-component-playground__check">
          <input checked={disabled} type="checkbox" onChange={(event) => setDisabled(event.currentTarget.checked)} />
          <span>{labels.disabled}</span>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetIconButton
  icon={<VelvetIcon name="add" />}
  label="${label || 'Icon button'}"
  busy={${busy}}
  disabled={${disabled}}
/>`}</code></pre>
    </div>
  );
}
