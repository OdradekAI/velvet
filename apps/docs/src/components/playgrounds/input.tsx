import { useState } from 'react';

import { VelvetInput } from '@velvet-ui';
import type { PlaygroundProps } from './types';
import '../../css/ui-kit-showcase.css';

export function InputPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [value, setValue] = useState('sunset.raw');
  const [placeholder, setPlaceholder] = useState('Enter an asset name');
  const [inputType, setInputType] = useState<'text' | 'search' | 'email'>('text');
  const [ariaLabel, setAriaLabel] = useState('Asset name');
  const [busy, setBusy] = useState(false);
  const [disabled, setDisabled] = useState(false);
  const labels = locale === 'en'
    ? {
        preview: 'Preview',
        value: 'Current value',
        empty: 'empty',
        valueProp: 'value',
        placeholderProp: 'placeholder',
        typeProp: 'type',
        ariaLabelProp: 'ariaLabel',
        busy: 'busy',
        disabled: 'disabled',
      }
    : {
        preview: '预览',
        value: '当前值',
        empty: '空',
        valueProp: 'value',
        placeholderProp: 'placeholder',
        typeProp: 'type',
        ariaLabelProp: 'ariaLabel',
        busy: 'busy',
        disabled: 'disabled',
      };

  return (
    <div className="velvet-component-playground velvet-component-playground--wide">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--form">
        <label className="velvet-component-playground__field">
          <span>{labels.preview}</span>
          <VelvetInput
            ariaLabel={ariaLabel}
            ariaLive="polite"
            busy={busy}
            disabled={disabled}
            placeholder={placeholder}
            type={inputType}
            value={value}
            onChange={(event) => setValue(event.currentTarget.value)}
          />
        </label>
        <output aria-live="polite" className="velvet-component-playground__output">
          {labels.value}: {value || labels.empty}
        </output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--wide">
        <label>
          <span>{labels.valueProp}</span>
          <input value={value} onChange={(event) => setValue(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.placeholderProp}</span>
          <input value={placeholder} onChange={(event) => setPlaceholder(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.typeProp}</span>
          <select value={inputType} onChange={(event) => setInputType(event.currentTarget.value as typeof inputType)}>
            <option value="text">text</option>
            <option value="search">search</option>
            <option value="email">email</option>
          </select>
        </label>
        <label>
          <span>{labels.ariaLabelProp}</span>
          <input value={ariaLabel} onChange={(event) => setAriaLabel(event.currentTarget.value)} />
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
      <pre className="velvet-component-playground__code"><code>{`<VelvetInput
  type="${inputType}"
  value={value}
  placeholder="${placeholder}"
  busy={${busy}}
  disabled={${disabled}}
/>`}</code></pre>
    </div>
  );
}
