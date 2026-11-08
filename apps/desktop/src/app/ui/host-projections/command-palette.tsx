import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, RefObject } from 'react';
import { VelvetInput, VelvetState } from '../primitives/index.js';
import type { UiCommonProps } from '../internal/types.js';

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
  readonly restoreFocusRef?: RefObject<HTMLElement | null>;
}

export function VelvetCommandPalette({
  open,
  commands,
  onSelect,
  onClose,
  placeholder = 'Search commands',
  restoreFocusRef,
}: VelvetCommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const previousOpenRef = useRef(open);
  const filtered = commands.filter((command) => `${command.label} ${command.description ?? ''}`.toLowerCase().includes(query.toLowerCase()));
  useEffect(() => setIndex(0), [query]);
  useEffect(() => {
    if (previousOpenRef.current && !open) {
      queueMicrotask(() => restoreFocusRef?.current?.focus());
    }
    previousOpenRef.current = open;
  }, [open, restoreFocusRef]);
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
            if (event.key === 'Home') { event.preventDefault(); setIndex(0); }
            if (event.key === 'End') { event.preventDefault(); setIndex(Math.max(0, filtered.length - 1)); }
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
