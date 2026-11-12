import { useState } from 'react';

import { VelvetSeparator, VelvetText } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function SeparatorPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [orientation, setOrientation] = useState<'horizontal' | 'vertical'>('horizontal');
  const labels = locale === 'en' ? {orientation: 'orientation'} : {orientation: 'orientation'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--separator">
        <VelvetText>Before</VelvetText>
        <VelvetSeparator orientation={orientation} />
        <VelvetText>After</VelvetText>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.orientation}</span>
          <select value={orientation} onChange={(event) => setOrientation(event.currentTarget.value as typeof orientation)}>
            <option value="horizontal">horizontal</option>
            <option value="vertical">vertical</option>
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetSeparator orientation="${orientation}" />`}</code></pre>
    </div>
  );
}
