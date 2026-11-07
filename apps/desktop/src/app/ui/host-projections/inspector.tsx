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

export type VelvetInspectorSaveState = 'idle' | 'saving' | 'saved' | 'error' | 'cancelled';

export interface VelvetInspectorProps extends UiCommonProps {
  readonly title: string;
  readonly selectedCount: number;
  readonly preview?: ReactNode;
  readonly fields?: readonly VelvetInspectorField[];
  readonly emptyMessage?: string;
  readonly onFieldChange?: (fieldId: string, value: string) => void;
  readonly onSave?: () => void;
  readonly onRetry?: () => void;
  readonly saveState?: VelvetInspectorSaveState;
  readonly saveError?: string;
  readonly pinned?: boolean;
  readonly onPin?: (pinned: boolean) => void;
  readonly onOpen?: () => void;
  readonly children?: ReactNode;
  readonly className?: string;
}

export function VelvetInspector({
  title,
  selectedCount,
  preview,
  fields = [],
  emptyMessage = 'Select an asset to inspect it.',
  onFieldChange,
  onSave,
  onRetry,
  saveState = 'idle',
  saveError,
  pinned = false,
  onPin,
  onOpen,
  children,
  className = '',
  ...common
}: VelvetInspectorProps) {
  const selectionMode = selectedCount === 0 ? 'empty' : selectedCount === 1 ? 'single' : 'multiple';
  const editableFields = fields.filter((field) => field.editable);
  return (
    <aside
      {...uiDataProps(common)}
      aria-label={title}
      className={`velvet-inspector velvet-inspector--${selectionMode} ${pinned ? 'velvet-inspector--pinned' : ''} ${className}`.trim()}
      data-inspector-pinned={pinned ? 'true' : undefined}
      data-inspector-selection={selectionMode}
      data-ui-kit-component="inspector"
    >
      <header className="velvet-inspector__header">
        <h2>{title}</h2>
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
          {fields.length > 0 ? (
            <dl className="velvet-inspector__metadata">
              {fields.map((field) => (
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
