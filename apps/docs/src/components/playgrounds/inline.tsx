import { useState } from 'react';

import { VelvetBadge, VelvetInline, VelvetText } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function InlinePlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [gap, setGap] = useState<'none' | 'xs' | 'sm' | 'md' | 'lg'>('md');
  const labels = locale === 'en' ? {gap: 'gap'} : {gap: 'gap'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetInline className="velvet-playground-stack" gap={gap}>
          <VelvetBadge tone="info">View</VelvetBadge>
          <VelvetBadge tone="success">Ready</VelvetBadge>
          <VelvetText tone="muted">Inline content</VelvetText>
        </VelvetInline>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.gap}</span>
          <select value={gap} onChange={(event) => setGap(event.currentTarget.value as typeof gap)}>
            {['none', 'xs', 'sm', 'md', 'lg'].map((gapName) => <option key={gapName} value={gapName}>{gapName}</option>)}
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetInline gap="${gap}">...</VelvetInline>`}</code></pre>
    </div>
  );
}
