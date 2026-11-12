import { useState } from 'react';

import Link from '@docusaurus/Link';

import { VelvetBadge } from '@velvet-desktop/app/ui/primitives/badge';
import { VelvetButton } from '@velvet-desktop/app/ui/primitives/button';
import { VelvetCheckbox } from '@velvet-desktop/app/ui/primitives/checkbox';
import { VelvetInput } from '@velvet-desktop/app/ui/primitives/input';
import { VelvetSelect } from '@velvet-desktop/app/ui/primitives/select';
import { VelvetState } from '@velvet-desktop/app/ui/primitives/state';
import { VelvetTabs } from '@velvet-desktop/app/ui/primitives/tabs';

import '../css/ui-kit-showcase.css';

type UiKitShowcaseProps = {
  readonly locale?: 'zh-Hans' | 'en';
};

const copy = {
  'zh-Hans': {
    showcaseTitle: '真实 UI Kit 组件',
    description: '这些示例直接复用桌面应用中的 Velvet primitive。页面展示交互状态，组件 contract 和 qualification 仍然以工程文档为准。',
    idleStatus: '点击 Save changes 查看 action。',
    savedStatus: 'Host action committed。',
    contractTitle: '组件页的阅读顺序',
    contractSteps: [
      '先看 semantic props 和 state owner。',
      '再看 story、keyboard 和 accessibility evidence。',
      '最后看 browser qualification、migration 和 compatibility。',
    ],
    contractLink: '打开 Button API 与 Playground →',
  },
  en: {
    showcaseTitle: 'Real UI Kit components',
    description: 'These examples reuse the real Velvet primitives from the desktop application. The page shows interaction states, while engineering references define the component contract and qualification evidence.',
    idleStatus: 'Click Save changes to exercise the action.',
    savedStatus: 'Host action committed.',
    contractTitle: 'How to read a component page',
    contractSteps: [
      'Start with semantic props and state ownership.',
      'Then read story, keyboard, and accessibility evidence.',
      'Finish with browser qualification, migration, and compatibility.',
    ],
    contractLink: 'Open Button API and Playground →',
  },
} as const;

export default function UiKitShowcase({ locale = 'zh-Hans' }: UiKitShowcaseProps) {
  const [checked, setChecked] = useState(true);
  const [selectedTab, setSelectedTab] = useState('primitive');
  const [status, setStatus] = useState<'idle' | 'saved'>('idle');
  const labels = copy[locale];

  return (
    <div className="velvet-ui-kit-showcase">
      <section className="velvet-ui-kit-showcase__card">
        <div className="velvet-ui-kit-showcase__eyebrow">Primitives</div>
        <h2>{labels.showcaseTitle}</h2>
        <p className="velvet-ui-kit-showcase__description">{labels.description}</p>

        <div className="velvet-ui-kit-showcase__group">
          <div className="velvet-ui-kit-showcase__label">Button states</div>
          <div className="velvet-ui-kit-showcase__row">
            <VelvetButton onClick={() => setStatus('saved')}>Save changes</VelvetButton>
            <VelvetButton variant="quiet">Secondary</VelvetButton>
            <VelvetButton variant="danger">Delete</VelvetButton>
            <VelvetButton busy>Saving</VelvetButton>
            <VelvetButton disabled>Disabled</VelvetButton>
          </div>
          <div className="velvet-ui-kit-showcase__status" role="status">
            {status === 'saved' ? labels.savedStatus : labels.idleStatus}
          </div>
        </div>

        <div className="velvet-ui-kit-showcase__group">
          <div className="velvet-ui-kit-showcase__label">Inputs</div>
          <div className="velvet-ui-kit-showcase__form-grid">
            <label>
              <span>Asset name</span>
              <VelvetInput ariaLabel="Asset name" defaultValue="sunset.raw" />
            </label>
            <label>
              <span>Preview mode</span>
              <VelvetSelect ariaLabel="Preview mode" defaultValue="inspect">
                <option value="quick">Quick</option>
                <option value="inspect">Inspect</option>
                <option value="analyze">Analyze</option>
              </VelvetSelect>
            </label>
          </div>
          <VelvetCheckbox checked={checked} label="Keep inspector open" onCheckedChange={setChecked} />
        </div>

        <div className="velvet-ui-kit-showcase__group">
          <div className="velvet-ui-kit-showcase__label">Tabs and status</div>
          <VelvetTabs
            ariaLabel="UI Kit demo tabs"
            onSelect={setSelectedTab}
            selectedId={selectedTab}
            tabs={[
              { id: 'primitive', label: 'Primitive', badge: 7 },
              { id: 'overlay', label: 'Overlay' },
              { id: 'projection', label: 'Projection', disabled: true },
            ]}
          />
          <div className="velvet-ui-kit-showcase__row velvet-ui-kit-showcase__row--aligned">
            <VelvetBadge tone="info">bridge 1</VelvetBadge>
            <VelvetBadge tone="success">qualified</VelvetBadge>
            <VelvetState
              description={`Selected tab: ${selectedTab}`}
              kind="empty"
              title="Ready for a host projection"
            />
          </div>
        </div>
      </section>

      <section className="velvet-ui-kit-showcase__card velvet-ui-kit-showcase__card--contract">
        <div className="velvet-ui-kit-showcase__eyebrow">Contract surface</div>
        <h2>{labels.contractTitle}</h2>
        <ol>
          {labels.contractSteps.map((step) => <li key={step}>{step}</li>)}
        </ol>
        <Link to="/ui-kit/components/button">{labels.contractLink}</Link>
      </section>
    </div>
  );
}
