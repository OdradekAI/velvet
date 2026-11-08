import { useId } from 'react';
import type { ReactNode } from 'react';
import { VelvetButton, VelvetInput } from '../primitives/index.js';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetInspectorField {
  readonly id: string;
  readonly label: string;
  readonly value: ReactNode;
  readonly editable?: boolean;
  readonly inputValue?: string;
  readonly placeholder?: string;
  readonly error?: string;
}

export interface VelvetInspectorSection {
  readonly id: string;
  readonly label: string;
  readonly fields: readonly VelvetInspectorField[];
  readonly expanded?: boolean;
}

export interface VelvetInspectorPaletteEntry {
  readonly id: string;
  readonly color: string;
  readonly ratio: number;
  readonly label?: string;
}

export type VelvetInspectorSaveState = 'idle' | 'saving' | 'saved' | 'error' | 'cancelled';

export interface VelvetInspectorProps extends UiCommonProps {
  readonly title: string;
  readonly selectedCount: number;
  readonly preview?: ReactNode;
  readonly fields?: readonly VelvetInspectorField[];
  readonly sections?: readonly VelvetInspectorSection[];
  readonly palette?: readonly VelvetInspectorPaletteEntry[];
  readonly emptyMessage?: string;
  readonly onFieldChange?: (fieldId: string, value: string) => void;
  readonly onSave?: () => void;
  readonly onRetry?: () => void;
  readonly saveState?: VelvetInspectorSaveState;
  readonly saveError?: string;
  readonly pinned?: boolean;
  readonly onPin?: (pinned: boolean) => void;
  readonly onOpen?: () => void;
  readonly onSectionToggle?: (sectionId: string, expanded: boolean) => void;
  readonly children?: ReactNode;
  readonly className?: string;
}

export function VelvetInspector({
  title,
  selectedCount,
  preview,
  fields = [],
  sections = [],
  palette = [],
  emptyMessage = 'Select an asset to inspect it.',
  onFieldChange,
  onSave,
  onRetry,
  saveState = 'idle',
  saveError,
  pinned = false,
  onPin,
  onOpen,
  onSectionToggle,
  children,
  className = '',
  ...common
}: VelvetInspectorProps) {
  const headingId = useId();
  const selectionMode = selectedCount === 0 ? 'empty' : selectedCount === 1 ? 'single' : 'multiple';
  const groupedFields = sections.length > 0 ? sections.flatMap((section) => section.fields) : fields;
  const editableFields = groupedFields.filter((field) => field.editable);
  const renderFields = (nextFields: readonly VelvetInspectorField[]) => (
    <dl className="velvet-inspector__metadata">
      {nextFields.map((field) => (
        <div key={field.id}>
          <dt><label htmlFor={`velvet-inspector-field-${field.id}`}>{field.label}</label></dt>
          <dd>
            {field.editable ? (
              <VelvetInput
                ariaLabel={field.label}
                describedBy={field.error ? `velvet-inspector-error-${field.id}` : undefined}
                id={`velvet-inspector-field-${field.id}`}
                onChange={(event) => onFieldChange?.(field.id, event.target.value)}
                placeholder={field.placeholder}
                value={field.inputValue ?? (typeof field.value === 'string' ? field.value : '')}
              />
            ) : field.value}
            {field.error ? <small id={`velvet-inspector-error-${field.id}`} role="alert">{field.error}</small> : null}
          </dd>
        </div>
      ))}
    </dl>
  );
  return (
    <aside
      {...uiDataProps(common)}
      aria-label={title}
      aria-labelledby={common.ariaLabel ? undefined : headingId}
      className={`velvet-inspector velvet-inspector--${selectionMode} ${pinned ? 'velvet-inspector--pinned' : ''} ${className}`.trim()}
      data-inspector-pinned={pinned ? 'true' : undefined}
      data-inspector-selection={selectionMode}
      data-ui-kit-component="inspector"
    >
      <header className="velvet-inspector__header">
        <h2 id={headingId}>{title}</h2>
        {onPin ? (
          <VelvetButton
            ariaLabel={pinned ? 'Unpin inspector' : 'Pin inspector'}
            aria-pressed={pinned}
            className="velvet-inspector__pin"
            onClick={() => onPin(!pinned)}
            variant="quiet"
          >
            {pinned ? 'Pinned' : 'Pin'}
          </VelvetButton>
        ) : null}
      </header>
      {selectionMode === 'empty' ? (
        <p className="velvet-inspector__empty" role="status">{emptyMessage}</p>
      ) : (
        <>
          {preview ? <div className="velvet-inspector__preview">{preview}</div> : null}
          {selectionMode === 'multiple' ? <div className="velvet-inspector__selected-count" data-inspector-selected-count="true">Selected <strong>{selectedCount}</strong> assets</div> : null}
          {sections.length > 0 ? sections.map((section) => (
            <section aria-labelledby={`velvet-inspector-section-${section.id}`} className="velvet-inspector__section" data-inspector-section={section.id} key={section.id}>
              <button
                aria-controls={`velvet-inspector-section-content-${section.id}`}
                aria-expanded={section.expanded !== false}
                className="velvet-inspector__section-toggle"
                onClick={() => onSectionToggle?.(section.id, section.expanded === false)}
                type="button"
              >
                <h3 id={`velvet-inspector-section-${section.id}`}>{section.label}</h3>
                <span aria-hidden="true">{section.expanded === false ? '›' : '⌄'}</span>
              </button>
              {section.expanded === false ? null : <div id={`velvet-inspector-section-content-${section.id}`}>{renderFields(section.fields)}</div>}
            </section>
          )) : fields.length > 0 ? renderFields(fields) : null}
          {palette.length > 0 ? (
            <section aria-label="Color palette" className="velvet-inspector__palette" data-inspector-palette="true">
              <h3>Color palette</h3>
              <ul>
                {palette.map((entry) => (
                  <li data-palette-id={entry.id} key={entry.id} title={`${entry.label ?? entry.color} ${entry.ratio}%`}>
                    <span aria-label={`${entry.label ?? entry.color}, ${entry.ratio}%`} className="velvet-inspector__swatch" role="img" style={{ backgroundColor: entry.color }} />
                    <span className="velvet-inspector__palette-label">{entry.label ?? entry.color}</span>
                    <data value={String(entry.ratio)}>{entry.ratio}%</data>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
          {children}
          {editableFields.length > 0 && onSave ? (
            <div className="velvet-inspector__edit-actions">
              <VelvetButton
                busy={saveState === 'saving'}
                data-inspector-action="save"
                disabled={saveState === 'saving'}
                onClick={onSave}
              >
                Save changes
              </VelvetButton>
              {saveState === 'error' && onRetry ? <VelvetButton data-inspector-action="retry" onClick={onRetry} variant="quiet">Retry</VelvetButton> : null}
              {saveState === 'saved' ? <span role="status">Saved</span> : null}
              {saveState === 'cancelled' ? <span role="status">Save cancelled</span> : null}
              {saveError ? <span role="alert">{saveError}</span> : null}
            </div>
          ) : null}
          {onOpen ? <VelvetButton data-inspector-action="open" onClick={onOpen} variant="quiet">Open in preview</VelvetButton> : null}
        </>
      )}
    </aside>
  );
}
