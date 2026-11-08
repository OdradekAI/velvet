import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent, RefObject } from 'react';
import { VelvetInput, VelvetState } from '../primitives/index.js';
import type { UiCommonProps } from '../internal/types.js';

export interface VelvetCommandPaletteCommand {
  readonly id: string;
  readonly label: string;
  readonly description?: string;
  readonly shortcutHint?: string;
  readonly disabled?: boolean;
  readonly scope?: VelvetCommandPaletteScope;
}

export type VelvetCommandPaletteScope = 'all' | 'current';

export interface VelvetCommandPaletteScopeOption {
  readonly id: VelvetCommandPaletteScope;
  readonly label: string;
}

export interface VelvetCommandPaletteProps extends UiCommonProps {
  readonly open: boolean;
  /** Commands are already scoped and filtered by the host adapter. */
  readonly commands: readonly VelvetCommandPaletteCommand[];
  readonly onSelect: (id: string) => void;
  readonly onClose: () => void;
  readonly scope?: VelvetCommandPaletteScope;
  readonly scopeOptions?: readonly VelvetCommandPaletteScopeOption[];
  readonly onScopeChange?: (scope: VelvetCommandPaletteScope) => void;
  readonly query?: string;
  readonly onQueryChange?: (query: string) => void;
  readonly placeholder?: string;
  readonly restoreFocusRef?: RefObject<HTMLElement | null>;
}

export function VelvetCommandPalette({
  open,
  commands,
  onSelect,
  onClose,
  scope = 'all',
  scopeOptions,
  onScopeChange,
  query,
  onQueryChange,
  placeholder = 'Search commands',
  restoreFocusRef,
}: VelvetCommandPaletteProps) {
  const [internalQuery, setInternalQuery] = useState('');
  const [index, setIndex] = useState(0);
  const previousOpenRef = useRef(open);
  const listId = useId();
  const resolvedQuery = query ?? internalQuery;

  useEffect(() => setIndex(0), [resolvedQuery, scope, commands]);
  useEffect(() => {
    if (previousOpenRef.current && !open) queueMicrotask(() => restoreFocusRef?.current?.focus());
    previousOpenRef.current = open;
  }, [open, restoreFocusRef]);

  if (!open) return null;

  const selectCurrent = (): void => {
    const command = commands[index];
    if (command && !command.disabled) onSelect(command.id);
  };
  const updateQuery = (nextQuery: string): void => {
    if (query === undefined) setInternalQuery(nextQuery);
    onQueryChange?.(nextQuery);
  };

  return (
    <div className="velvet-command-palette__overlay" onMouseDown={onClose}>
      <section aria-label="Command Palette" className="velvet-command-palette" onMouseDown={(event) => event.stopPropagation()} role="dialog">
        <VelvetInput
          aria-activedescendant={commands[index] ? `${listId}-${commands[index].id}` : undefined}
          ariaLabel={placeholder}
          aria-controls={listId}
          autoFocus
          data-command-results="host"
          onChange={(event) => updateQuery(event.currentTarget.value)}
          onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
            if (event.key === 'Escape') onClose();
            if (event.key === 'ArrowDown') { event.preventDefault(); setIndex((value) => Math.min(commands.length - 1, value + 1)); }
            if (event.key === 'ArrowUp') { event.preventDefault(); setIndex((value) => Math.max(0, value - 1)); }
            if (event.key === 'Home') { event.preventDefault(); setIndex(0); }
            if (event.key === 'End') { event.preventDefault(); setIndex(Math.max(0, commands.length - 1)); }
            if (event.key === 'Enter') { event.preventDefault(); selectCurrent(); }
          }}
          placeholder={placeholder}
          value={resolvedQuery}
        />
        {scopeOptions && scopeOptions.length > 0 ? (
          <div aria-label="Command scope" className="velvet-command-palette__scopes" role="tablist">
            {scopeOptions.map((option) => (
              <button aria-label={option.label} aria-selected={scope === option.id} className="velvet-command-palette__scope" key={option.id} onClick={() => onScopeChange?.(option.id)} role="tab" type="button">{option.label}</button>
            ))}
          </div>
        ) : null}
        <div className="velvet-command-palette__list" id={listId} role="listbox">
          {commands.length === 0 ? <VelvetState kind="empty" title="No commands found" /> : null}
          {commands.map((command, commandIndex) => (
            <button
              aria-selected={commandIndex === index}
              id={`${listId}-${command.id}`}
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
