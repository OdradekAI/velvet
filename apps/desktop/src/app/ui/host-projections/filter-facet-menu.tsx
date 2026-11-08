import { useEffect, useRef, type ChangeEvent, type KeyboardEvent, type MouseEvent, type RefObject } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export type VelvetFilterFacetMenuKind = 'folder' | 'color';
export type VelvetFilterFolderRule = 'OR' | 'AND' | 'EQUAL';

export interface VelvetFilterFolderOption {
  readonly id: string;
  readonly label: string;
  readonly count?: number;
  readonly depth?: number;
  readonly state?: 'idle' | 'included' | 'excluded';
}

export interface VelvetFilterColorSwatch {
  readonly id: string;
  readonly color: string;
  readonly label?: string;
  readonly selected?: boolean;
}

interface VelvetFilterFacetMenuBaseProps extends UiCommonProps {
  readonly open?: boolean;
  readonly title?: string;
  readonly query?: string;
  readonly onQueryChange?: (query: string) => void;
  readonly onClear?: () => void;
  readonly onClose?: () => void;
  readonly className?: string;
}

export interface VelvetFolderFilterFacetMenuProps extends VelvetFilterFacetMenuBaseProps {
  readonly kind: 'folder';
  readonly options: readonly VelvetFilterFolderOption[];
  readonly rule?: VelvetFilterFolderRule;
  readonly onRuleChange?: (rule: VelvetFilterFolderRule) => void;
  readonly onOptionAction?: (id: string, action: 'include' | 'exclude') => void;
}

export interface VelvetColorFilterFacetMenuProps extends VelvetFilterFacetMenuBaseProps {
  readonly kind: 'color';
  readonly swatches: readonly VelvetFilterColorSwatch[];
  readonly selectedColor?: string;
  readonly graySelected?: boolean;
  readonly hexValue?: string;
  readonly accuracy?: number;
  readonly onPresetChange?: (value: string | 'gray' | undefined) => void;
  readonly onHexChange?: (value: string) => void;
  readonly onAccuracyChange?: (value: number) => void;
}

export type VelvetFilterFacetMenuProps = VelvetFolderFilterFacetMenuProps | VelvetColorFilterFacetMenuProps;

export function VelvetFilterFacetMenu(props: VelvetFilterFacetMenuProps) {
  if (props.open === false) return null;
  const { kind, title = kind === 'folder' ? 'Folders' : 'Color', className = '', ...common } = props;
  const folderSearchRef = useRef<HTMLInputElement>(null);
  const colorHexRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    (kind === 'folder' ? folderSearchRef.current : colorHexRef.current)?.focus();
  }, [kind]);
  return (
    <section
      {...uiDataProps(common)}
      aria-label={common.ariaLabel ?? title}
      className={`velvet-filter-facet-menu velvet-filter-facet-menu--${kind} ${className}`.trim()}
      data-filter-facet-kind={kind}
      data-ui-kit-component="filter-facet-menu"
      onKeyDown={(event: KeyboardEvent<HTMLElement>) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          props.onClose?.();
        }
      }}
      role="dialog"
    >
      {kind === 'folder' ? <FolderMenu {...props} searchRef={folderSearchRef} title={title} /> : <ColorMenu {...props} hexRef={colorHexRef} title={title} />}
    </section>
  );
}

function FolderMenu({
  options,
  query = '',
  rule = 'OR',
  title,
  onClear,
  onClose,
  onOptionAction,
  onQueryChange,
  onRuleChange,
  searchRef,
}: VelvetFolderFilterFacetMenuProps & { readonly searchRef: RefObject<HTMLInputElement | null>; readonly title: string }) {
  return (
    <>
      <header className="velvet-filter-facet-menu__header">
        <input
          aria-label={`${title} search`}
          className="velvet-filter-facet-menu__search"
          onChange={(event: ChangeEvent<HTMLInputElement>) => onQueryChange?.(event.currentTarget.value)}
          placeholder="Search"
          ref={searchRef}
          type="search"
          value={query}
        />
        <span className="velvet-filter-facet-menu__rule-label">Rule</span>
        <div aria-label="Folder filter rule" className="velvet-filter-facet-menu__rule-tabs" role="tablist">
          {(['OR', 'AND', 'EQUAL'] as const).map((candidate) => (
            <button
              aria-selected={rule === candidate}
              className="velvet-filter-facet-menu__rule-tab"
              data-filter-rule={candidate}
              key={candidate}
              onClick={() => onRuleChange?.(candidate)}
              role="tab"
              type="button"
            >
              {candidate === 'OR' ? 'Any' : candidate === 'AND' ? 'All' : 'Equal'}
            </button>
          ))}
        </div>
      </header>
      <div className="velvet-filter-facet-menu__content">
        <div className="velvet-filter-facet-menu__options" role="listbox">
          {options.map((option) => (
            <button
              aria-pressed={option.state === 'included'}
              className="velvet-filter-facet-menu__option"
              data-filter-option={option.id}
              data-filter-state={option.state ?? 'idle'}
              key={option.id}
              onClick={() => onOptionAction?.(option.id, 'include')}
              onContextMenu={(event: MouseEvent<HTMLButtonElement>) => {
                event.preventDefault();
                onOptionAction?.(option.id, 'exclude');
              }}
              role="option"
              style={{ paddingLeft: `${6 + (option.depth ?? 0) * 20}px` }}
              type="button"
            >
              <span aria-hidden="true" className="velvet-filter-facet-menu__check" />
              <span className="velvet-filter-facet-menu__option-label">{option.label}</span>
              {option.count === undefined ? null : <span className="velvet-filter-facet-menu__count">{option.count}</span>}
            </button>
          ))}
          {options.length > 0 ? null : <div className="velvet-filter-facet-menu__empty">No folders</div>}
        </div>
      </div>
      <FacetMenuFooter onClear={onClear} onClose={onClose} primaryHint="click include" secondaryHint="right-click exclude" />
    </>
  );
}

function ColorMenu({
  accuracy = 20,
  graySelected = false,
  hexValue = '#FFFFFF',
  onAccuracyChange,
  onClear,
  onClose,
  onHexChange,
  onPresetChange,
  selectedColor,
  swatches,
  title,
  hexRef,
}: VelvetColorFilterFacetMenuProps & { readonly hexRef: RefObject<HTMLInputElement | null>; readonly title: string }) {
  return (
    <>
      <header className="velvet-filter-facet-menu__compact-header"><span>{title}</span>{onClose ? <button aria-label="Close color filter" onClick={onClose} type="button">×</button> : null}</header>
      <div className="velvet-filter-facet-menu__content velvet-filter-facet-menu__color-content">
        <div aria-label="Color presets" className="velvet-filter-facet-menu__swatches" role="listbox">
          <button aria-pressed={!selectedColor && !graySelected} className="velvet-filter-facet-menu__swatch velvet-filter-facet-menu__swatch--none" data-filter-color="none" onClick={() => onPresetChange?.(undefined)} type="button" />
          <button aria-pressed={graySelected} className="velvet-filter-facet-menu__swatch velvet-filter-facet-menu__swatch--gray" data-filter-color="gray" onClick={() => onPresetChange?.('gray')} type="button" />
          {swatches.map((swatch) => (
            <button
              aria-label={swatch.label ?? swatch.id}
              aria-pressed={swatch.selected ?? selectedColor === swatch.color}
              className="velvet-filter-facet-menu__swatch"
              data-filter-color={swatch.id}
              key={swatch.id}
              onClick={() => onPresetChange?.(swatch.color)}
              style={{ backgroundColor: swatch.color }}
              type="button"
            />
          ))}
        </div>
        <div className="velvet-filter-facet-menu__hex-field">
          <input aria-label="Hex color" onChange={(event: ChangeEvent<HTMLInputElement>) => onHexChange?.(event.currentTarget.value)} ref={hexRef} value={hexValue} />
          <span aria-hidden="true" className="velvet-filter-facet-menu__color-preview" style={{ backgroundColor: hexValue }} />
        </div>
        <label className="velvet-filter-facet-menu__accuracy">
          <span>Accuracy</span>
          <input aria-label="Color accuracy" max={40} min={5} onChange={(event: ChangeEvent<HTMLInputElement>) => onAccuracyChange?.(Number(event.currentTarget.value))} step={1} type="range" value={accuracy} />
        </label>
      </div>
      <FacetMenuFooter onClear={onClear} onClose={onClose} primaryHint="click a color to apply" />
    </>
  );
}

function FacetMenuFooter({
  onClear,
  onClose,
  primaryHint,
  secondaryHint,
}: {
  readonly onClear?: () => void;
  readonly onClose?: () => void;
  readonly primaryHint: string;
  readonly secondaryHint?: string;
}) {
  return (
    <footer className="velvet-filter-facet-menu__footer">
      <span>{primaryHint}</span>
      {secondaryHint ? <span>{secondaryHint}</span> : null}
      {onClear ? <button data-filter-action="clear" onClick={onClear} type="button">Clear</button> : null}
      {onClose ? <button data-filter-action="close" onClick={onClose} type="button">Close</button> : null}
    </footer>
  );
}
