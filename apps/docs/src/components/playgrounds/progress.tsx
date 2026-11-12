import { useState } from 'react';

import { VelvetProgress } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function ProgressPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [value, setValue] = useState('64');
  const [tone, setTone] = useState<'accent' | 'success' | 'danger'>('accent');
  const [label, setLabel] = useState(locale === 'en' ? 'Indexing assets' : '正在索引 Asset');
  const parsedValue = Number(value);
  const numericValue = value.trim() === '' || Number.isNaN(parsedValue) ? undefined : parsedValue;
  const labels = locale === 'en'
    ? {label: 'label', value: 'value', tone: 'tone', indeterminate: 'empty value = indeterminate'}
    : {label: 'label', value: 'value', tone: 'tone', indeterminate: '留空表示 indeterminate'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetProgress label={label || undefined} tone={tone} value={numericValue} />
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--wide">
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.value}</span>
          <input inputMode="decimal" value={value} onChange={(event) => setValue(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.tone}</span>
          <select value={tone} onChange={(event) => setTone(event.currentTarget.value as typeof tone)}>
            <option value="accent">accent</option>
            <option value="success">success</option>
            <option value="danger">danger</option>
          </select>
        </label>
        <span className="velvet-component-playground__hint">{labels.indeterminate}</span>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetProgress
  value={${numericValue === undefined ? 'undefined' : numericValue}}
  label="${label}"
  tone="${tone}"
/>`}</code></pre>
    </div>
  );
}
