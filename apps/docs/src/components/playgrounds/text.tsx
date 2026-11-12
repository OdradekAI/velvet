import { useState } from 'react';

import { VelvetText } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function TextPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [as, setAs] = useState<'p' | 'span' | 'strong' | 'small' | 'h1' | 'h2' | 'h3'>('p');
  const [tone, setTone] = useState<'primary' | 'secondary' | 'muted' | 'danger' | 'success' | 'warning'>('primary');
  const [content, setContent] = useState(locale === 'en' ? 'Semantic host text' : '语义化宿主文本');
  const labels = locale === 'en'
    ? {as: 'as', tone: 'tone', content: 'children'}
    : {as: 'as', tone: 'tone', content: 'children'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetText as={as} tone={tone}>{content || 'Text'}</VelvetText>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--wide">
        <label>
          <span>{labels.as}</span>
          <select value={as} onChange={(event) => setAs(event.currentTarget.value as typeof as)}>
            {['p', 'span', 'strong', 'small', 'h1', 'h2', 'h3'].map((tag) => <option key={tag} value={tag}>{tag}</option>)}
          </select>
        </label>
        <label>
          <span>{labels.tone}</span>
          <select value={tone} onChange={(event) => setTone(event.currentTarget.value as typeof tone)}>
            {['primary', 'secondary', 'muted', 'danger', 'success', 'warning'].map((toneName) => <option key={toneName} value={toneName}>{toneName}</option>)}
          </select>
        </label>
        <label>
          <span>{labels.content}</span>
          <input value={content} onChange={(event) => setContent(event.currentTarget.value)} />
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetText as="${as}" tone="${tone}">${content || 'Text'}</VelvetText>`}</code></pre>
    </div>
  );
}
