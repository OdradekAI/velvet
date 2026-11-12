import { useState } from 'react';

import { VelvetCheckbox } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function CheckboxPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [checked, setChecked] = useState(true);
  const [label, setLabel] = useState(locale === 'en' ? 'Keep inspector open' : '保持 Inspector 打开');
  const [disabled, setDisabled] = useState(false);
  const labels = locale === 'en'
    ? {
        preview: 'Preview',
        checked: 'Checked',
        unchecked: 'Unchecked',
        labelProp: 'label',
        disabled: 'disabled',
        fallback: 'Checkbox label',
      }
    : {
        preview: '预览',
        checked: '已选中',
        unchecked: '未选中',
        labelProp: 'label',
        disabled: 'disabled',
        fallback: 'Checkbox label',
      };
  const effectiveLabel = label || labels.fallback;

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--checkbox">
        <VelvetCheckbox
          checked={checked}
          disabled={disabled}
          label={effectiveLabel}
          testId="checkbox-playground"
          onCheckedChange={setChecked}
        />
        <output aria-live="polite" className="velvet-component-playground__output">
          {checked ? labels.checked : labels.unchecked}
        </output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.labelProp}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <label className="velvet-component-playground__check">
          <input checked={disabled} type="checkbox" onChange={(event) => setDisabled(event.currentTarget.checked)} />
          <span>{labels.disabled}</span>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetCheckbox
  checked={${checked}}
  label="${effectiveLabel}"
  disabled={${disabled}}
  onCheckedChange={setChecked}
/>`}</code></pre>
    </div>
  );
}
