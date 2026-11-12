import { useState } from 'react';

import { VelvetTabs, type VelvetTab } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function TabsPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [selectedId, setSelectedId] = useState('overview');
  const [showBadges, setShowBadges] = useState(true);
  const [disableDetails, setDisableDetails] = useState(false);
  const labels = locale === 'en'
    ? {
        overview: 'Overview',
        activity: 'Activity',
        details: 'Details',
        selected: 'Selected tab',
        badges: 'show badges',
        disabled: 'disable Details',
        keyboard: 'Use Arrow keys, Home, and End to move selection.',
        ariaLabel: 'Tabs playground',
      }
    : {
        overview: '概览',
        activity: '活动',
        details: '详情',
        selected: '当前 Tab',
        badges: '显示 badge',
        disabled: '禁用详情',
        keyboard: '使用方向键、Home 和 End 移动选择。',
        ariaLabel: 'Tabs Playground',
      };
  const tabs: readonly VelvetTab[] = [
    { id: 'overview', label: labels.overview, badge: showBadges ? 3 : undefined },
    { id: 'activity', label: labels.activity, badge: showBadges ? 7 : undefined },
    { id: 'details', label: labels.details, disabled: disableDetails },
  ];

  const handleDisableDetails = (nextDisabled: boolean) => {
    setDisableDetails(nextDisabled);
    if (nextDisabled && selectedId === 'details') setSelectedId('overview');
  };

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--tabs">
        <VelvetTabs
          ariaLabel={labels.ariaLabel}
          selectedId={selectedId}
          tabs={tabs}
          testId="tabs-playground"
          onSelect={setSelectedId}
        />
        <output aria-live="polite" className="velvet-component-playground__output">
          {labels.selected}: {selectedId}
        </output>
        <span className="velvet-component-playground__hint">{labels.keyboard}</span>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label className="velvet-component-playground__check">
          <input checked={showBadges} type="checkbox" onChange={(event) => setShowBadges(event.currentTarget.checked)} />
          <span>{labels.badges}</span>
        </label>
        <label className="velvet-component-playground__check">
          <input checked={disableDetails} type="checkbox" onChange={(event) => handleDisableDetails(event.currentTarget.checked)} />
          <span>{labels.disabled}</span>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetTabs
  selectedId="${selectedId}"
  tabs={tabs}
  onSelect={setSelectedId}
/>`}</code></pre>
    </div>
  );
}
