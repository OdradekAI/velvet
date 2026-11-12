import { useState } from 'react';

import { VelvetContextMenu, type VelvetContextMenuAction } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function ContextMenuPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [status, setStatus] = useState(locale === 'en' ? 'Right-click the target' : '右键点击目标区域');
  const labels = locale === 'en'
    ? {target: 'Right-click this target', open: 'Open asset', favorite: 'Favorite', arrange: 'Arrange', front: 'Bring to front', back: 'Send to back'}
    : {target: '右键点击这个目标区域', open: '打开 Asset', favorite: '收藏', arrange: '排列', front: '置于顶层', back: '置于底层'};
  const actions: readonly VelvetContextMenuAction[] = [
    {id: 'open', label: labels.open, shortcutHint: 'Enter', onSelect: () => setStatus(labels.open)},
    {id: 'favorite', label: labels.favorite, checked: true, onSelect: () => setStatus(labels.favorite)},
    {
      id: 'arrange',
      label: labels.arrange,
      submenu: [
        {id: 'front', label: labels.front, onSelect: () => setStatus(labels.front)},
        {id: 'back', label: labels.back, onSelect: () => setStatus(labels.back)},
      ],
    },
  ];

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetContextMenu actions={actions}>
          <div className="velvet-playground-context-target">{labels.target}</div>
        </VelvetContextMenu>
        <output className="velvet-component-playground__output">{status}</output>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetContextMenu actions={actions}>...</VelvetContextMenu>`}</code></pre>
    </div>
  );
}
