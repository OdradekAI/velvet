import { useState } from 'react';

import { VelvetSelect } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function SelectPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [value, setValue] = useState<string | string[]>('inspect');
  const [multiple, setMultiple] = useState(false);
  const [busy, setBusy] = useState(false);
  const [disabled, setDisabled] = useState(false);
  const [ariaLabel, setAriaLabel] = useState('Preview mode');
  const options = [
    { value: 'quick', label: locale === 'en' ? 'Quick' : '快速' },
    { value: 'inspect', label: locale === 'en' ? 'Inspect' : '检查' },
    { value: 'analyze', label: locale === 'en' ? 'Analyze' : '分析' },
  ];
  const labels = locale === 'en'
    ? {
        preview: 'Preview',
        value: 'Selected value',
        empty: 'none',
        ariaLabelProp: 'ariaLabel',
        multiple: 'multiple',
        busy: 'busy',
        disabled: 'disabled',
      }
    : {
        preview: '预览',
        value: '当前选中',
        empty: '无',
        ariaLabelProp: 'ariaLabel',
        multiple: 'multiple',
        busy: 'busy',
        disabled: 'disabled',
      };
  const renderedValue = multiple
    ? (Array.isArray(value) ? value : [value])
    : (Array.isArray(value) ? value[0] ?? 'inspect' : value);
  const selectedValues = Array.isArray(renderedValue) ? renderedValue : [renderedValue];

  const handleMultipleChange = (nextMultiple: boolean) => {
    setMultiple(nextMultiple);
    setValue(nextMultiple
      ? (Array.isArray(value) ? value : [value])
      : (Array.isArray(value) ? value[0] ?? 'inspect' : value));
  };

  return (
    <div className="velvet-component-playground velvet-component-playground--wide">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--form">
        <label className="velvet-component-playground__field">
          <span>{labels.preview}</span>
          <VelvetSelect
            ariaLabel={ariaLabel}
            busy={busy}
            disabled={disabled}
            multiple={multiple}
            value={renderedValue}
            onChange={(event) => {
              setValue(multiple
                ? Array.from(event.currentTarget.selectedOptions, (option) => option.value)
                : event.currentTarget.value);
            }}
          >
            {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </VelvetSelect>
        </label>
        <output aria-live="polite" className="velvet-component-playground__output">
          {labels.value}: {selectedValues.join(', ') || labels.empty}
        </output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--wide">
        <label>
          <span>{labels.ariaLabelProp}</span>
          <input value={ariaLabel} onChange={(event) => setAriaLabel(event.currentTarget.value)} />
        </label>
        <label className="velvet-component-playground__check">
          <input checked={multiple} type="checkbox" onChange={(event) => handleMultipleChange(event.currentTarget.checked)} />
          <span>{labels.multiple}</span>
        </label>
        <label className="velvet-component-playground__check">
          <input checked={busy} type="checkbox" onChange={(event) => setBusy(event.currentTarget.checked)} />
          <span>{labels.busy}</span>
        </label>
        <label className="velvet-component-playground__check">
          <input checked={disabled} type="checkbox" onChange={(event) => setDisabled(event.currentTarget.checked)} />
          <span>{labels.disabled}</span>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetSelect
  value={value}
  multiple={${multiple}}
  busy={${busy}}
  disabled={${disabled}}
/>`}</code></pre>
    </div>
  );
}
