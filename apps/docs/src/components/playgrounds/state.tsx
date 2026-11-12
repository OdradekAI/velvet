import { useState } from 'react';

import { VelvetState } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function StatePlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [kind, setKind] = useState<'loading' | 'empty' | 'error'>('error');
  const [status, setStatus] = useState(locale === 'en' ? 'No retry yet' : '尚未重试');
  const labels = locale === 'en'
    ? {
        kind: 'kind',
        title: 'title',
        description: 'description',
        loading: 'Loading assets',
        empty: 'No assets found',
        error: 'Asset index failed',
        retry: 'Retry indexing',
        retryStatus: 'Retry requested',
      }
    : {
        kind: 'kind',
        title: 'title',
        description: 'description',
        loading: '正在加载 Asset',
        empty: '没有找到 Asset',
        error: 'Asset 索引失败',
        retry: '重试索引',
        retryStatus: '已请求重试',
      };
  const title = kind === 'loading' ? labels.loading : kind === 'empty' ? labels.empty : labels.error;
  const description = locale === 'en' ? 'The host owns loading, empty, error, and retry semantics.' : '宿主拥有 loading、empty、error 和 retry 语义。';

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetState
          description={description}
          kind={kind}
          onRetry={kind === 'error' ? () => setStatus(labels.retryStatus) : undefined}
          retryLabel={labels.retry}
          title={title}
        />
        <output aria-live="polite" className="velvet-component-playground__output">{status}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.kind}</span>
          <select value={kind} onChange={(event) => setKind(event.currentTarget.value as typeof kind)}>
            <option value="loading">loading</option>
            <option value="empty">empty</option>
            <option value="error">error</option>
          </select>
        </label>
        <label>
          <span>{labels.title}</span>
          <input value={title} readOnly />
        </label>
        <label>
          <span>{labels.description}</span>
          <input value={description} readOnly />
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetState
  kind="${kind}"
  title="${title}"
  onRetry={${kind === 'error' ? 'handleRetry' : 'undefined'}}
/>`}</code></pre>
    </div>
  );
}
