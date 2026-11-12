import { useState } from 'react';

import { VelvetButton, VelvetToastHost, type VelvetToastItem } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function ToastPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [items, setItems] = useState<readonly VelvetToastItem[]>([]);
  const [nextId, setNextId] = useState(1);
  const labels = locale === 'en'
    ? {info: 'Show info', success: 'Show success', warning: 'Show warning', error: 'Show error', message: 'A host notification is visible.'}
    : {info: '显示 info', success: '显示 success', warning: '显示 warning', error: '显示 error', message: '宿主通知已显示。'};
  const pushToast = (severity: VelvetToastItem['severity']) => {
    const id = `toast-${nextId}`;
    setNextId((current) => current + 1);
    setItems((current) => [...current, {id, message: labels.message, severity, title: severity?.toUpperCase()}]);
  };

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <div className="velvet-component-playground__row">
          <VelvetButton onClick={() => pushToast('info')}>{labels.info}</VelvetButton>
          <VelvetButton onClick={() => pushToast('success')}>{labels.success}</VelvetButton>
          <VelvetButton onClick={() => pushToast('warning')}>{labels.warning}</VelvetButton>
          <VelvetButton variant="danger" onClick={() => pushToast('error')}>{labels.error}</VelvetButton>
        </div>
        <VelvetToastHost items={items} onDismiss={(id) => setItems((current) => current.filter((item) => item.id !== id))} />
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetToastHost items={items} onDismiss={dismiss} />`}</code></pre>
    </div>
  );
}
