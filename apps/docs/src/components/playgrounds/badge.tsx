import { useState } from 'react';

import { VelvetBadge } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function BadgePlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [label, setLabel] = useState(locale === 'en' ? 'qualified' : '已通过');
  const [tone, setTone] = useState<'neutral' | 'info' | 'success' | 'warning' | 'danger'>('success');
  const labels = locale === 'en'
    ? {preview: 'Preview', label: 'label', tone: 'tone', status: 'Rendered tone'}
    : {preview: '预览', label: 'label', tone: 'tone', status: '当前 tone'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetBadge tone={tone}>{label || 'Badge'}</VelvetBadge>
        <output className="velvet-component-playground__output">{labels.status}: {tone}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.tone}</span>
          <select value={tone} onChange={(event) => setTone(event.currentTarget.value as typeof tone)}>
            <option value="neutral">neutral</option>
            <option value="info">info</option>
            <option value="success">success</option>
            <option value="warning">warning</option>
            <option value="danger">danger</option>
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetBadge tone="${tone}">${label || 'Badge'}</VelvetBadge>`}</code></pre>
    </div>
  );
}
