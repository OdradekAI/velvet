import * as ContextMenu from '@radix-ui/react-context-menu';
import * as Dialog from '@radix-ui/react-dialog';
import {
  createContext,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react';
import type {
  ButtonHTMLAttributes,
  HTMLAttributes,
  InputHTMLAttributes,
  KeyboardEvent,
  ReactNode,
  SelectHTMLAttributes,
} from 'react';

export const UI_KIT_API_VERSION = '1.0' as const;

export type UiButtonVariant = 'default' | 'quiet' | 'danger';

export interface UiCommonProps {
  readonly apiVersion?: string;
  readonly testId?: string;
  readonly ariaLabel?: string;
  readonly describedBy?: string;
  readonly disabled?: boolean;
  readonly busy?: boolean;
}

function uiDataProps(props: UiCommonProps): Record<string, string | undefined> {
  return {
    'data-ui-kit-api-version': props.apiVersion ?? UI_KIT_API_VERSION,
    'data-ui-test-id': props.testId,
    'aria-label': props.ariaLabel,
    'aria-describedby': props.describedBy,
    'aria-busy': props.busy ? 'true' : undefined,
  };
}

export interface VelvetTextProps extends UiCommonProps {
  readonly as?: 'p' | 'span' | 'strong' | 'small' | 'h1' | 'h2' | 'h3';
  readonly tone?: 'primary' | 'secondary' | 'muted' | 'danger' | 'success' | 'warning';
  readonly children: ReactNode;
  readonly className?: string;
}

export function VelvetText({
  as = 'span',
  tone = 'primary',
  children,
  className = '',
  ...common
}: VelvetTextProps) {
  const Tag = as;
  return (
    <Tag
      {...uiDataProps(common)}
      className={`velvet-text velvet-text--${tone} ${className}`.trim()}
    >
      {children}
    </Tag>
  );
}

export interface VelvetStackProps extends UiCommonProps {
  readonly children: ReactNode;
  readonly className?: string;
  readonly gap?: 'none' | 'xs' | 'sm' | 'md' | 'lg';
  readonly direction?: 'column' | 'row';
}

export function VelvetStack({
  children,
  className = '',
  gap = 'md',
  direction = 'column',
  ...common
}: VelvetStackProps) {
  return (
    <div
      {...uiDataProps(common)}
      className={`velvet-stack velvet-stack--${direction} velvet-stack--gap-${gap} ${className}`.trim()}
    >
      {children}
    </div>
  );
}

export interface VelvetInlineProps extends Omit<VelvetStackProps, 'direction'> {}

export function VelvetInline(props: VelvetInlineProps) {
  return <VelvetStack {...props} direction="row" />;
}

export interface VelvetButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children' | 'disabled' | 'aria-label' | 'aria-describedby'>,
    UiCommonProps {
  readonly children?: ReactNode;
  readonly label?: string;
  readonly variant?: UiButtonVariant;
}

export function VelvetButton({
  children,
  label,
  variant = 'default',
  className = '',
  disabled,
  busy,
  apiVersion,
  testId,
  ariaLabel,
  describedBy,
  ...props
}: VelvetButtonProps) {
  return (
    <button
      {...props}
      {...uiDataProps({ apiVersion, testId, ariaLabel, describedBy, disabled, busy })}
      className={`velvet-button velvet-button--${variant} ${className}`.trim()}
      data-ui-kit-component="button"
      disabled={disabled || busy}
      type={props.type ?? 'button'}
    >
      {busy ? <span aria-hidden="true" className="velvet-button__spinner" /> : null}
      {children ?? label}
    </button>
  );
}

export interface VelvetIconButtonProps
  extends Omit<VelvetButtonProps, 'children' | 'label' | 'ariaLabel'> {
  readonly icon: ReactNode;
  readonly label: string;
}

export function VelvetIconButton({ icon, label, className = '', ...props }: VelvetIconButtonProps) {
  return (
    <VelvetButton
      {...props}
      ariaLabel={label}
      className={`velvet-icon-button ${className}`.trim()}
    >
      <span aria-hidden="true">{icon}</span>
    </VelvetButton>
  );
}

export interface VelvetInputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'disabled' | 'aria-label' | 'aria-describedby'>,
    UiCommonProps {}

export function VelvetInput({ className = '', disabled, busy, ...props }: VelvetInputProps) {
  const { apiVersion, testId, ariaLabel, describedBy, ...inputProps } = props;
  return (
    <input
      {...inputProps}
      {...uiDataProps({ apiVersion, testId, ariaLabel, describedBy, disabled, busy })}
      className={`velvet-input ${className}`.trim()}
      data-ui-kit-component="input"
      disabled={disabled || busy}
    />
  );
}

export interface VelvetSelectProps
  extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'disabled' | 'aria-label' | 'aria-describedby'>,
    UiCommonProps {}

export function VelvetSelect({ className = '', disabled, busy, ...props }: VelvetSelectProps) {
  const { apiVersion, testId, ariaLabel, describedBy, ...selectProps } = props;
  return (
    <select
      {...selectProps}
      {...uiDataProps({ apiVersion, testId, ariaLabel, describedBy, disabled, busy })}
      className={`velvet-select ${className}`.trim()}
      data-ui-kit-component="select"
      disabled={disabled || busy}
    />
  );
}

export interface VelvetCheckboxProps extends UiCommonProps {
  readonly checked: boolean;
  readonly label: string;
  readonly onCheckedChange: (checked: boolean) => void;
  readonly className?: string;
}

export function VelvetCheckbox({
  checked,
  label,
  onCheckedChange,
  className = '',
  disabled,
  ...common
}: VelvetCheckboxProps) {
  const id = useId();
  return (
    <label {...uiDataProps(common)} className={`velvet-checkbox ${className}`.trim()} htmlFor={id}>
      <input
        checked={checked}
        disabled={disabled}
        id={id}
        onChange={(event) => onCheckedChange(event.currentTarget.checked)}
        type="checkbox"
      />
      <span>{label}</span>
    </label>
  );
}

export function VelvetSeparator({ orientation = 'horizontal' }: { readonly orientation?: 'horizontal' | 'vertical' }) {
  return <div aria-orientation={orientation} className={`velvet-separator velvet-separator--${orientation}`} role="separator" />;
}

export interface VelvetPanelProps {
  readonly title?: string;
  readonly children: ReactNode;
  readonly className?: string;
  readonly ariaLabel?: string;
  readonly id?: string;
  readonly surface?: 'panel' | 'raised' | 'overlay';
}

export function VelvetPanel({
  title,
  children,
  className = '',
  ariaLabel,
  id,
  surface = 'panel',
}: VelvetPanelProps) {
  return (
    <section
      aria-label={ariaLabel}
      className={`velvet-panel velvet-panel--surface-${surface} ${className}`.trim()}
      data-ui-kit-component="panel"
      id={id}
    >
      {title ? <header className="velvet-panel__header">{title}</header> : null}
      <div className="velvet-panel__body">{children}</div>
    </section>
  );
}

export interface VelvetDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly title: string;
  readonly description?: string;
  readonly children: ReactNode;
  readonly actions?: readonly VelvetDialogAction[];
  readonly severity?: 'normal' | 'warning' | 'danger';
  readonly closeLabel?: string;
}

export interface VelvetDialogAction {
  readonly id: string;
  readonly label: string;
  readonly variant?: UiButtonVariant;
  readonly primary?: boolean;
  readonly disabled?: boolean;
  readonly busy?: boolean;
  readonly onSelect: () => void;
}

export function VelvetDialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  actions = [],
  severity = 'normal',
  closeLabel = 'Close',
}: VelvetDialogProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="velvet-dialog__overlay" />
        <Dialog.Content className={`velvet-dialog__content velvet-dialog__content--${severity}`}>
          <Dialog.Title className="velvet-dialog__title">{title}</Dialog.Title>
          {description ? (
            <Dialog.Description className="velvet-dialog__description">
              {description}
            </Dialog.Description>
          ) : null}
          <div className="velvet-dialog__body">{children}</div>
          <div className="velvet-dialog__actions">
            {actions.map((action) => (
              <VelvetButton
                busy={action.busy}
                className={action.primary ? 'velvet-dialog__action--primary' : ''}
                disabled={action.disabled}
                key={action.id}
                onClick={action.onSelect}
                variant={action.variant ?? (action.primary ? 'default' : 'quiet')}
              >
                {action.label}
              </VelvetButton>
            ))}
            <Dialog.Close className="velvet-button velvet-button--quiet">
              {closeLabel}
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export interface VelvetContextMenuAction {
  readonly id: string;
  readonly label: string;
  readonly disabled?: boolean;
  readonly checked?: boolean;
  readonly shortcutHint?: string;
  readonly separatorBefore?: boolean;
  readonly group?: string;
  readonly onSelect?: () => void;
}

export interface VelvetContextMenuProps {
  readonly children: ReactNode;
  readonly actions: readonly VelvetContextMenuAction[];
}

export function VelvetContextMenu({ children, actions }: VelvetContextMenuProps) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger className="velvet-context-target">
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="velvet-context-menu">
          {actions.map((action) => (
            <span key={action.id}>
              {action.separatorBefore ? <ContextMenu.Separator className="velvet-context-menu__separator" /> : null}
              <ContextMenu.Item
                aria-checked={action.checked}
                className="velvet-context-menu__item"
                data-checked={action.checked ? 'true' : undefined}
                disabled={action.disabled}
                onSelect={action.onSelect}
                role={action.checked === undefined ? undefined : 'menuitemcheckbox'}
              >
                <span>{action.label}</span>
                {action.shortcutHint ? <kbd>{action.shortcutHint}</kbd> : null}
              </ContextMenu.Item>
            </span>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

export interface VelvetTab {
  readonly id: string;
  readonly label: string;
  readonly disabled?: boolean;
  readonly badge?: string | number;
}

export interface VelvetTabsProps extends UiCommonProps {
  readonly tabs: readonly VelvetTab[];
  readonly selectedId: string;
  readonly onSelect: (id: string) => void;
  readonly className?: string;
}

export function VelvetTabs({
  tabs,
  selectedId,
  onSelect,
  className = '',
  ...common
}: VelvetTabsProps) {
  const selectRelative = (start: number, direction: 1 | -1): void => {
    if (tabs.length === 0) return;
    for (let offset = 1; offset <= tabs.length; offset += 1) {
      const candidate = tabs[(start + offset * direction + tabs.length) % tabs.length];
      if (candidate && !candidate.disabled) {
        onSelect(candidate.id);
        return;
      }
    }
  };

  return (
    <div {...uiDataProps(common)} className={`velvet-tabs ${className}`.trim()}>
      <div aria-label={common.ariaLabel ?? 'Tabs'} className="velvet-tabs__list" role="tablist">
        {tabs.map((tab, index) => {
          const selected = tab.id === selectedId;
          return (
            <button
              aria-controls={`velvet-tab-panel-${tab.id}`}
              aria-selected={selected}
              className="velvet-tabs__tab"
              disabled={tab.disabled}
              id={`velvet-tab-${tab.id}`}
              key={tab.id}
              onClick={() => onSelect(tab.id)}
              onKeyDown={(event) => {
                if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
                  event.preventDefault();
                  selectRelative(index, 1);
                } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
                  event.preventDefault();
                  selectRelative(index, -1);
                } else if (event.key === 'Home') {
                  event.preventDefault();
                  const first = tabs.find((candidate) => !candidate.disabled);
                  if (first) onSelect(first.id);
                } else if (event.key === 'End') {
                  event.preventDefault();
                  const last = [...tabs].reverse().find((candidate) => !candidate.disabled);
                  if (last) onSelect(last.id);
                }
              }}
              role="tab"
              tabIndex={selected ? 0 : -1}
              type="button"
            >
              {tab.label}
              {tab.badge !== undefined ? <span className="velvet-tabs__badge">{tab.badge}</span> : null}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export interface VelvetProgressProps extends UiCommonProps {
  readonly value?: number;
  readonly label?: string;
  readonly tone?: 'accent' | 'success' | 'danger';
  readonly className?: string;
}

export function VelvetProgress({
  value,
  label,
  tone = 'accent',
  className = '',
  ...common
}: VelvetProgressProps) {
  const determinate = value !== undefined;
  const bounded = determinate ? Math.max(0, Math.min(100, value)) : undefined;
  return (
    <div {...uiDataProps(common)} className={`velvet-progress velvet-progress--${tone} ${className}`.trim()}>
      {label ? <span className="velvet-progress__label">{label}</span> : null}
      <progress aria-label={label ?? 'Progress'} max={100} value={bounded} />
      {determinate ? <span className="velvet-progress__value">{bounded}%</span> : null}
    </div>
  );
}

export type VelvetStateKind = 'loading' | 'empty' | 'error';

export interface VelvetStateProps extends UiCommonProps {
  readonly kind: VelvetStateKind;
  readonly title: string;
  readonly description?: string;
  readonly retryLabel?: string;
  readonly onRetry?: () => void;
  readonly action?: ReactNode;
  readonly className?: string;
}

export function VelvetState({
  kind,
  title,
  description,
  retryLabel = 'Retry',
  onRetry,
  action,
  className = '',
  ...common
}: VelvetStateProps) {
  return (
    <section
      {...uiDataProps(common)}
      aria-busy={kind === 'loading' ? 'true' : undefined}
      aria-live={kind === 'error' ? 'assertive' : 'polite'}
      className={`velvet-state velvet-state--${kind} ${className}`.trim()}
      role={kind === 'error' ? 'alert' : 'status'}
    >
      <strong>{title}</strong>
      {description ? <p>{description}</p> : null}
      {kind === 'error' && onRetry ? <VelvetButton onClick={onRetry}>{retryLabel}</VelvetButton> : null}
      {action}
    </section>
  );
}

export interface VelvetToastItem {
  readonly id: string;
  readonly message: string;
  readonly title?: string;
  readonly severity?: 'info' | 'success' | 'warning' | 'error';
  readonly action?: ReactNode;
}

export interface VelvetToastHostProps {
  readonly items: readonly VelvetToastItem[];
  readonly onDismiss: (id: string) => void;
  readonly className?: string;
}

export function VelvetToastHost({ items, onDismiss, className = '' }: VelvetToastHostProps) {
  return (
    <div aria-label="Notifications" className={`velvet-toast-host ${className}`.trim()} role="region">
      {items.map((item) => (
        <article
          aria-live={item.severity === 'error' ? 'assertive' : 'polite'}
          className={`velvet-toast velvet-toast--${item.severity ?? 'info'}`}
          data-toast-id={item.id}
          key={item.id}
          role={item.severity === 'error' ? 'alert' : 'status'}
        >
          <div className="velvet-toast__content">
            {item.title ? <strong>{item.title}</strong> : null}
            <span>{item.message}</span>
            {item.action}
          </div>
          <VelvetIconButton icon="×" label="Dismiss notification" onClick={() => onDismiss(item.id)} />
        </article>
      ))}
    </div>
  );
}

interface VelvetToastContextValue {
  readonly push: (item: Omit<VelvetToastItem, 'id'>) => string;
  readonly dismiss: (id: string) => void;
}

const VelvetToastContext = createContext<VelvetToastContextValue | undefined>(undefined);

export function VelvetToastProvider({ children }: { readonly children: ReactNode }) {
  const [items, setItems] = useState<readonly VelvetToastItem[]>([]);
  const nextId = useRef(0);
  const dismiss = (id: string) => setItems((current) => current.filter((item) => item.id !== id));
  const push = (item: Omit<VelvetToastItem, 'id'>): string => {
    nextId.current += 1;
    const id = `toast-${nextId.current}`;
    setItems((current) => [...current, { ...item, id }]);
    return id;
  };
  const value = useMemo(() => ({ push, dismiss }), []);
  return (
    <VelvetToastContext.Provider value={value}>
      {children}
      <VelvetToastHost items={items} onDismiss={dismiss} />
    </VelvetToastContext.Provider>
  );
}

export function useVelvetToast(): VelvetToastContextValue {
  const context = useContext(VelvetToastContext);
  if (!context) {
    throw new Error('useVelvetToast must be used inside VelvetToastProvider');
  }
  return context;
}

export interface VelvetActivityBarItem {
  readonly id: string;
  readonly label: string;
  readonly icon: ReactNode;
  readonly badge?: string | number;
  readonly active?: boolean;
  readonly disabled?: boolean;
}

export interface VelvetActivityBarProps extends UiCommonProps {
  readonly items: readonly VelvetActivityBarItem[];
  readonly onSelect: (id: string) => void;
  readonly className?: string;
}

export function VelvetActivityBar({ items, onSelect, className = '', ...common }: VelvetActivityBarProps) {
  const activeIndex = Math.max(0, items.findIndex((item) => item.active));
  const move = (index: number, direction: 1 | -1): void => {
    for (let offset = 1; offset <= items.length; offset += 1) {
      const candidate = items[(index + offset * direction + items.length) % items.length];
      if (candidate && !candidate.disabled) {
        onSelect(candidate.id);
        return;
      }
    }
  };
  return (
    <nav {...uiDataProps(common)} aria-label={common.ariaLabel ?? 'Workspaces'} className={`velvet-activity-bar ${className}`.trim()}>
      {items.map((item, index) => (
        <button
          aria-current={item.active ? 'page' : undefined}
          aria-label={item.label}
          aria-pressed={item.active}
          className="velvet-activity-bar__item"
          disabled={item.disabled}
          key={item.id}
          onClick={() => onSelect(item.id)}
          onKeyDown={(event) => {
            if (event.key === 'ArrowDown' || event.key === 'ArrowRight') {
              event.preventDefault();
              move(index, 1);
            } else if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') {
              event.preventDefault();
              move(index, -1);
            } else if (event.key === 'Home') {
              event.preventDefault();
              const first = items.find((candidate) => !candidate.disabled);
              if (first) onSelect(first.id);
            } else if (event.key === 'End') {
              event.preventDefault();
              const last = [...items].reverse().find((candidate) => !candidate.disabled);
              if (last) onSelect(last.id);
            }
          }}
          tabIndex={index === activeIndex ? 0 : -1}
          type="button"
        >
          <span aria-hidden="true">{item.icon}</span>
          {item.badge !== undefined ? <span className="velvet-activity-bar__badge">{item.badge}</span> : null}
        </button>
      ))}
    </nav>
  );
}

export interface VelvetPaneProps {
  readonly title: string;
  readonly region: 'navigation' | 'context' | 'operations' | 'preview';
  readonly children: ReactNode;
  readonly className?: string;
}

export function VelvetPane({ title, region, children, className = '' }: VelvetPaneProps) {
  return (
    <section aria-label={title} className={`velvet-pane velvet-pane--${region} ${className}`.trim()} data-ui-kit-region={region}>
      <header className="velvet-pane__header">{title}</header>
      <div className="velvet-pane__body">{children}</div>
    </section>
  );
}

export interface VelvetPreviewHostProps {
  readonly state: 'loading' | 'ready' | 'empty' | 'error' | 'disabled';
  readonly children?: ReactNode;
  readonly message?: string;
  readonly onRetry?: () => void;
}

export function VelvetPreviewHost({ state, children, message, onRetry }: VelvetPreviewHostProps) {
  if (state === 'ready') {
    return <section aria-label="Preview" className="velvet-preview-host" data-preview-state={state}>{children}</section>;
  }
  const kind: VelvetStateKind = state === 'loading' ? 'loading' : state === 'error' ? 'error' : 'empty';
  return (
    <section aria-label="Preview" className="velvet-preview-host" data-preview-state={state}>
      <VelvetState
        description={message}
        kind={kind}
        onRetry={state === 'error' ? onRetry : undefined}
        title={state === 'disabled' ? 'Preview unavailable' : state === 'loading' ? 'Loading preview' : state === 'error' ? 'Preview failed' : 'No preview'}
      />
    </section>
  );
}

export interface VelvetAssetGridItem {
  readonly id: string;
  readonly label: string;
  readonly secondary?: string;
  readonly thumbnailUrl?: string;
  readonly selected?: boolean;
  readonly disabled?: boolean;
}

export interface VelvetAssetGridProps extends UiCommonProps {
  readonly items: readonly VelvetAssetGridItem[];
  readonly onSelect: (id: string, additive: boolean) => void;
  readonly onOpen?: (id: string) => void;
  readonly columns?: number;
  readonly className?: string;
}

export function VelvetAssetGrid({
  items,
  onSelect,
  onOpen,
  columns = 4,
  className = '',
  ...common
}: VelvetAssetGridProps) {
  const current = Math.max(0, items.findIndex((item) => item.selected));
  const move = (index: number, delta: number): void => {
    const next = Math.max(0, Math.min(items.length - 1, index + delta));
    const item = items[next];
    if (item && !item.disabled) onSelect(item.id, false);
  };
  return (
    <div
      {...uiDataProps(common)}
      aria-label={common.ariaLabel ?? 'Assets'}
      className={`velvet-asset-grid ${className}`.trim()}
      role="grid"
      style={{ '--velvet-grid-columns': columns } as HTMLAttributes<HTMLDivElement>['style']}
    >
      {items.map((item, index) => (
        <button
          aria-label={item.label}
          aria-selected={item.selected}
          className="velvet-asset-grid__item"
          disabled={item.disabled}
          key={item.id}
          onClick={(event) => onSelect(item.id, event.metaKey || event.ctrlKey)}
          onDoubleClick={() => onOpen?.(item.id)}
          onKeyDown={(event) => {
            if (event.key === 'ArrowRight') { event.preventDefault(); move(index, 1); }
            if (event.key === 'ArrowLeft') { event.preventDefault(); move(index, -1); }
            if (event.key === 'ArrowDown') { event.preventDefault(); move(index, columns); }
            if (event.key === 'ArrowUp') { event.preventDefault(); move(index, -columns); }
            if (event.key === 'Enter') { event.preventDefault(); onOpen?.(item.id); }
          }}
          role="gridcell"
          tabIndex={index === current ? 0 : -1}
          type="button"
        >
          {item.thumbnailUrl ? <img alt="" src={item.thumbnailUrl} /> : <span aria-hidden="true" className="velvet-asset-grid__placeholder" />}
          <span className="velvet-asset-grid__label">{item.label}</span>
          {item.secondary ? <span className="velvet-asset-grid__secondary">{item.secondary}</span> : null}
        </button>
      ))}
    </div>
  );
}

export interface VelvetDataTableColumn<Row> {
  readonly id: string;
  readonly label: string;
  readonly render: (row: Row) => ReactNode;
  readonly sortable?: boolean;
}

export interface VelvetDataTableProps<Row> extends UiCommonProps {
  readonly rows: readonly Row[];
  readonly columns: readonly VelvetDataTableColumn<Row>[];
  readonly rowId: (row: Row) => string;
  readonly selectedId?: string;
  readonly onSelect?: (id: string) => void;
  readonly onSort?: (columnId: string) => void;
  readonly className?: string;
}

export function VelvetDataTable<Row>({
  rows,
  columns,
  rowId,
  selectedId,
  onSelect,
  onSort,
  className = '',
  ...common
}: VelvetDataTableProps<Row>) {
  return (
    <div {...uiDataProps(common)} className={`velvet-data-table ${className}`.trim()} role="region">
      <table aria-label={common.ariaLabel ?? 'Data table'}>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.id} scope="col">
                {column.sortable ? <button className="velvet-data-table__sort" onClick={() => onSort?.(column.id)} type="button">{column.label}</button> : column.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const id = rowId(row);
            return (
              <tr aria-selected={selectedId === id} key={id} onClick={() => onSelect?.(id)} tabIndex={0}>
                {columns.map((column) => <td key={column.id}>{column.render(row)}</td>)}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export interface VelvetCommandPaletteCommand {
  readonly id: string;
  readonly label: string;
  readonly description?: string;
  readonly shortcutHint?: string;
  readonly disabled?: boolean;
}

export interface VelvetCommandPaletteProps extends UiCommonProps {
  readonly open: boolean;
  readonly commands: readonly VelvetCommandPaletteCommand[];
  readonly onSelect: (id: string) => void;
  readonly onClose: () => void;
  readonly placeholder?: string;
}

export function VelvetCommandPalette({
  open,
  commands,
  onSelect,
  onClose,
  placeholder = 'Search commands',
}: VelvetCommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const filtered = commands.filter((command) => `${command.label} ${command.description ?? ''}`.toLowerCase().includes(query.toLowerCase()));
  useEffect(() => setIndex(0), [query]);
  if (!open) return null;
  const selectCurrent = () => {
    const command = filtered[index];
    if (command && !command.disabled) onSelect(command.id);
  };
  return (
    <div className="velvet-command-palette__overlay" onMouseDown={onClose}>
      <section aria-label="Command Palette" className="velvet-command-palette" onMouseDown={(event) => event.stopPropagation()} role="dialog">
        <VelvetInput
          ariaLabel={placeholder}
          autoFocus
          onChange={(event) => setQuery(event.currentTarget.value)}
          onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
            if (event.key === 'Escape') onClose();
            if (event.key === 'ArrowDown') { event.preventDefault(); setIndex((value) => Math.min(filtered.length - 1, value + 1)); }
            if (event.key === 'ArrowUp') { event.preventDefault(); setIndex((value) => Math.max(0, value - 1)); }
            if (event.key === 'Enter') { event.preventDefault(); selectCurrent(); }
          }}
          placeholder={placeholder}
          value={query}
        />
        <div className="velvet-command-palette__list" role="listbox">
          {filtered.length === 0 ? <VelvetState kind="empty" title="No commands found" /> : null}
          {filtered.map((command, commandIndex) => (
            <button
              aria-selected={commandIndex === index}
              className="velvet-command-palette__item"
              disabled={command.disabled}
              key={command.id}
              onClick={() => onSelect(command.id)}
              role="option"
              type="button"
            >
              <span><strong>{command.label}</strong>{command.description ? <small>{command.description}</small> : null}</span>
              {command.shortcutHint ? <kbd>{command.shortcutHint}</kbd> : null}
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}
