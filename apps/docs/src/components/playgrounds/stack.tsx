import { useState } from 'react';

import { VelvetStack, VelvetText } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function StackPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [direction, setDirection] = useState<'column' | 'row'>('column');
  const [gap, setGap] = useState<'none' | 'xs' | 'sm' | 'md' | 'lg'>('md');
  const labels = locale === 'en' ? {direction: 'direction', gap: 'gap'} : {direction: 'direction', gap: 'gap'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetStack className="velvet-playground-stack" direction={direction} gap={gap}>
          <VelvetText tone="primary">First item</VelvetText>
          <VelvetText tone="secondary">Second item</VelvetText>
          <VelvetText tone="muted">Third item</VelvetText>
        </VelvetStack>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.direction}</span>
          <select value={direction} onChange={(event) => setDirection(event.currentTarget.value as typeof direction)}>
            <option value="column">column</option>
            <option value="row">row</option>
          </select>
        </label>
        <label>
          <span>{labels.gap}</span>
          <select value={gap} onChange={(event) => setGap(event.currentTarget.value as typeof gap)}>
            {['none', 'xs', 'sm', 'md', 'lg'].map((gapName) => <option key={gapName} value={gapName}>{gapName}</option>)}
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetStack direction="${direction}" gap="${gap}">...</VelvetStack>`}</code></pre>
    </div>
  );
}
