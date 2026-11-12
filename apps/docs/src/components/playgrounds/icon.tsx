import { useState } from 'react';

import { VelvetIcon, type VelvetIconName } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function IconPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [name, setName] = useState<VelvetIconName>('spark');
  const [label, setLabel] = useState('');
  const labels = locale === 'en'
    ? {name: 'name', label: 'label', decorative: 'empty label = decorative', status: 'Accessible mode'}
    : {name: 'name', label: 'label', decorative: 'label 为空表示装饰性图标', status: '无障碍模式'};
  const names: VelvetIconName[] = ['add', 'check', 'search', 'settings', 'spark', 'warning'];

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetIcon label={label || undefined} name={name} />
        <output className="velvet-component-playground__output">{labels.status}: {label ? 'labelled image' : 'decorative'}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.name}</span>
          <select value={name} onChange={(event) => setName(event.currentTarget.value as VelvetIconName)}>
            {names.map((iconName) => <option key={iconName} value={iconName}>{iconName}</option>)}
          </select>
        </label>
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <span className="velvet-component-playground__hint">{labels.decorative}</span>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetIcon name="${name}"${label ? ` label="${label}"` : ''} />`}</code></pre>
    </div>
  );
}
