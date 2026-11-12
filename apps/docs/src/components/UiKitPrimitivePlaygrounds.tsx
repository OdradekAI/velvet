import { useRef, useState } from 'react';

import { VelvetBadge } from '@velvet-desktop/app/ui/primitives/badge';
import { VelvetButton } from '@velvet-desktop/app/ui/primitives/button';
import { VelvetCheckbox } from '@velvet-desktop/app/ui/primitives/checkbox';
import { VelvetIcon, type VelvetIconName } from '@velvet-desktop/app/ui/primitives/icon';
import { VelvetIconButton } from '@velvet-desktop/app/ui/primitives/icon-button';
import { VelvetInput } from '@velvet-desktop/app/ui/primitives/input';
import { VelvetProgress } from '@velvet-desktop/app/ui/primitives/progress';
import { VelvetSelect } from '@velvet-desktop/app/ui/primitives/select';
import { VelvetSeparator } from '@velvet-desktop/app/ui/primitives/separator';
import { VelvetInline } from '@velvet-desktop/app/ui/primitives/inline';
import { VelvetStack } from '@velvet-desktop/app/ui/primitives/stack';
import { VelvetState } from '@velvet-desktop/app/ui/primitives/state';
import { VelvetTabs, type VelvetTab } from '@velvet-desktop/app/ui/primitives/tabs';
import { VelvetText } from '@velvet-desktop/app/ui/primitives/text';
import { VelvetContextMenu, type VelvetContextMenuAction } from '@velvet-desktop/app/ui/overlays/context-menu';
import { VelvetDialog, type VelvetDialogAction } from '@velvet-desktop/app/ui/overlays/dialog';
import { VelvetToastHost, type VelvetToastItem } from '@velvet-desktop/app/ui/overlays/toast';

import '../css/ui-kit-showcase.css';

type PlaygroundLocale = 'zh-Hans' | 'en';

type PlaygroundProps = {
  readonly locale?: PlaygroundLocale;
};

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

export function BadgePlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [label, setLabel] = useState(locale === 'en' ? 'qualified' : '已通过');
  const [tone, setTone] = useState<'neutral' | 'info' | 'success' | 'warning' | 'danger'>('success');
  const labels = locale === 'en'
    ? {preview: 'Preview', label: 'label', tone: 'tone', status: 'Rendered tone'}
    : {preview: '预览', label: 'label', tone: 'tone', status: '当前 tone'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetBadge tone={tone}>{label || 'Badge'}</VelvetBadge>
        <output className="velvet-component-playground__output">{labels.status}: {tone}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.tone}</span>
          <select value={tone} onChange={(event) => setTone(event.currentTarget.value as typeof tone)}>
            <option value="neutral">neutral</option>
            <option value="info">info</option>
            <option value="success">success</option>
            <option value="warning">warning</option>
            <option value="danger">danger</option>
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetBadge tone="${tone}">${label || 'Badge'}</VelvetBadge>`}</code></pre>
    </div>
  );
}

export function IconButtonPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [label, setLabel] = useState(locale === 'en' ? 'Add asset' : '添加 Asset');
  const [busy, setBusy] = useState(false);
  const [disabled, setDisabled] = useState(false);
  const [status, setStatus] = useState(locale === 'en' ? 'No action yet' : '尚未触发 action');
  const labels = locale === 'en'
    ? {preview: 'Preview', label: 'label', busy: 'busy', disabled: 'disabled'}
    : {preview: '预览', label: 'label', busy: 'busy', disabled: 'disabled'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetIconButton
          busy={busy}
          disabled={disabled}
          icon={<VelvetIcon name="add" />}
          label={label || 'Icon button'}
          onClick={() => setStatus(locale === 'en' ? 'button.activate committed' : 'button.activate 已提交')}
        />
        <output className="velvet-component-playground__output">{status}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--wide">
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
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
      <pre className="velvet-component-playground__code"><code>{`<VelvetIconButton
  icon={<VelvetIcon name="add" />}
  label="${label || 'Icon button'}"
  busy={${busy}}
  disabled={${disabled}}
/>`}</code></pre>
    </div>
  );
}

export function ProgressPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [value, setValue] = useState('64');
  const [tone, setTone] = useState<'accent' | 'success' | 'danger'>('accent');
  const [label, setLabel] = useState(locale === 'en' ? 'Indexing assets' : '正在索引 Asset');
  const parsedValue = Number(value);
  const numericValue = value.trim() === '' || Number.isNaN(parsedValue) ? undefined : parsedValue;
  const labels = locale === 'en'
    ? {label: 'label', value: 'value', tone: 'tone', indeterminate: 'empty value = indeterminate'}
    : {label: 'label', value: 'value', tone: 'tone', indeterminate: '留空表示 indeterminate'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetProgress label={label || undefined} tone={tone} value={numericValue} />
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--wide">
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.value}</span>
          <input inputMode="decimal" value={value} onChange={(event) => setValue(event.currentTarget.value)} />
        </label>
        <label>
          <span>{labels.tone}</span>
          <select value={tone} onChange={(event) => setTone(event.currentTarget.value as typeof tone)}>
            <option value="accent">accent</option>
            <option value="success">success</option>
            <option value="danger">danger</option>
          </select>
        </label>
        <span className="velvet-component-playground__hint">{labels.indeterminate}</span>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetProgress
  value={${numericValue === undefined ? 'undefined' : numericValue}}
  label="${label}"
  tone="${tone}"
/>`}</code></pre>
    </div>
  );
}

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

export function IconPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [name, setName] = useState<VelvetIconName>('spark');
  const [label, setLabel] = useState('');
  const labels = locale === 'en'
    ? {name: 'name', label: 'label', decorative: 'empty label = decorative', status: 'Accessible mode'}
    : {name: 'name', label: 'label', decorative: 'label 为空表示装饰性图标', status: '无障碍模式'};
  const names: VelvetIconName[] = ['add', 'check', 'search', 'settings', 'spark', 'warning'];

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetIcon label={label || undefined} name={name} />
        <output className="velvet-component-playground__output">{labels.status}: {label ? 'labelled image' : 'decorative'}</output>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.name}</span>
          <select value={name} onChange={(event) => setName(event.currentTarget.value as VelvetIconName)}>
            {names.map((iconName) => <option key={iconName} value={iconName}>{iconName}</option>)}
          </select>
        </label>
        <label>
          <span>{labels.label}</span>
          <input value={label} onChange={(event) => setLabel(event.currentTarget.value)} />
        </label>
        <span className="velvet-component-playground__hint">{labels.decorative}</span>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetIcon name="${name}"${label ? ` label="${label}"` : ''} />`}</code></pre>
    </div>
  );
}

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

export function StackPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [direction, setDirection] = useState<'column' | 'row'>('column');
  const [gap, setGap] = useState<'none' | 'xs' | 'sm' | 'md' | 'lg'>('md');
  const labels = locale === 'en' ? {direction: 'direction', gap: 'gap'} : {direction: 'direction', gap: 'gap'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetStack className="velvet-playground-stack" direction={direction} gap={gap}>
          <VelvetText tone="primary">First item</VelvetText>
          <VelvetText tone="secondary">Second item</VelvetText>
          <VelvetText tone="muted">Third item</VelvetText>
        </VelvetStack>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.direction}</span>
          <select value={direction} onChange={(event) => setDirection(event.currentTarget.value as typeof direction)}>
            <option value="column">column</option>
            <option value="row">row</option>
          </select>
        </label>
        <label>
          <span>{labels.gap}</span>
          <select value={gap} onChange={(event) => setGap(event.currentTarget.value as typeof gap)}>
            {['none', 'xs', 'sm', 'md', 'lg'].map((gapName) => <option key={gapName} value={gapName}>{gapName}</option>)}
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetStack direction="${direction}" gap="${gap}">...</VelvetStack>`}</code></pre>
    </div>
  );
}

export function InlinePlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [gap, setGap] = useState<'none' | 'xs' | 'sm' | 'md' | 'lg'>('md');
  const labels = locale === 'en' ? {gap: 'gap'} : {gap: 'gap'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--status">
        <VelvetInline className="velvet-playground-stack" gap={gap}>
          <VelvetBadge tone="info">View</VelvetBadge>
          <VelvetBadge tone="success">Ready</VelvetBadge>
          <VelvetText tone="muted">Inline content</VelvetText>
        </VelvetInline>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.gap}</span>
          <select value={gap} onChange={(event) => setGap(event.currentTarget.value as typeof gap)}>
            {['none', 'xs', 'sm', 'md', 'lg'].map((gapName) => <option key={gapName} value={gapName}>{gapName}</option>)}
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetInline gap="${gap}">...</VelvetInline>`}</code></pre>
    </div>
  );
}

export function SeparatorPlayground({ locale = 'zh-Hans' }: PlaygroundProps) {
  const [orientation, setOrientation] = useState<'horizontal' | 'vertical'>('horizontal');
  const labels = locale === 'en' ? {orientation: 'orientation'} : {orientation: 'orientation'};

  return (
    <div className="velvet-component-playground">
      <div className="velvet-component-playground__preview velvet-component-playground__preview--separator">
        <VelvetText>Before</VelvetText>
        <VelvetSeparator orientation={orientation} />
        <VelvetText>After</VelvetText>
      </div>
      <div className="velvet-component-playground__controls velvet-component-playground__controls--two">
        <label>
          <span>{labels.orientation}</span>
          <select value={orientation} onChange={(event) => setOrientation(event.currentTarget.value as typeof orientation)}>
            <option value="horizontal">horizontal</option>
            <option value="vertical">vertical</option>
          </select>
        </label>
      </div>
      <pre className="velvet-component-playground__code"><code>{`<VelvetSeparator orientation="${orientation}" />`}</code></pre>
    </div>
  );
}

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
