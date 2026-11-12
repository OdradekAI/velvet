import { useRef, useState } from 'react';

import { VelvetButton, VelvetDialog, type VelvetDialogAction } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function DialogPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [open, setOpen] = useState(false);
  const [severity, setSeverity] = useState<'normal' | 'warning' | 'danger'>('warning');
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState(locale === 'en' ? 'Dialog closed' : 'Dialog 已关闭');
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const labels = locale === 'en'
    ? {open: 'Open dialog', title: 'Delete asset?', description: 'This action removes the selected asset from the current workspace.', confirm: 'Delete', close: 'Close', status: 'Dialog closed'}
    : {open: '打开 Dialog', title: '删除 Asset？', description: '此操作会从当前 workspace 移除选中的 Asset。', confirm: '删除', close: '关闭', status: 'Dialog 已关闭'};
  const actions: readonly VelvetDialogAction[] = [
    {
      id: 'confirm',
      label: labels.confirm,
      primary: true,
      busy,
      onSelect: () => {
        setStatus(locale === 'en' ? 'Delete action committed' : '删除 action 已提交');
        setOpen(false);
      },
    },
  ];

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetButton ref={triggerRef} onClick={() => { setStatus(labels.status); setOpen(true); }}>{labels.open}</VelvetButton>
        <output className="velvet-component-playground__output">{status}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>severity</span>
          <select value={severity} onChange={(event) => setSeverity(event.currentTarget.value as typeof severity)}>
            <option value="normal">normal</option>
            <option value="warning">warning</option>
            <option value="danger">danger</option>
          </select>
        </label>
        <label className="velvet-component-playground__check">
          <input checked={busy} type="checkbox" onChange={(event) => setBusy(event.currentTarget.checked)} />
          <span>busy</span>
        </label>
      </div>
      <VelvetDialog
        actions={actions}
        closeLabel={labels.close}
        description={labels.description}
        open={open}
        restoreFocusRef={triggerRef}
        severity={severity}
        title={labels.title}
        onOpenChange={setOpen}
      >
        <p>{locale === 'en' ? 'The host owns overlay placement, focus trapping, escape dismissal, and focus restoration.' : '宿主拥有 Overlay 位置、焦点捕获、Escape 关闭和焦点恢复。'}</p>
      </VelvetDialog>
      <pre className="velvet-component-playground__code"><code>{`<VelvetDialog open={open} severity="${severity}" actions={actions} />`}</code></pre>
    </div>
  );
}
