import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent, RefObject } from 'react';
import { VelvetInput, VelvetState } from '../primitives/index.js';
import type { UiCommonProps } from '../internal/types.js';

type VelvetCommandQueryToken =
  | { readonly kind: 'term' | 'phrase'; readonly value: string }
  | { readonly kind: 'and' | 'or' | 'not' | 'lparen' | 'rparen' };

type VelvetCommandQueryNode =
  | { readonly kind: 'term'; readonly value: string; readonly phrase: boolean }
  | { readonly kind: 'not'; readonly child: VelvetCommandQueryNode }
  | { readonly kind: 'and' | 'or'; readonly children: readonly VelvetCommandQueryNode[] };

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
  const scopedCommands = commands.filter((command) => scope === 'all' || command.scope === undefined || command.scope === scope);
  const resolvedQuery = query ?? internalQuery;
  const queryExpression = parseVelvetCommandQuery(resolvedQuery);
  const filtered = scopedCommands.filter((command) => matchesVelvetCommandQuery(queryExpression, `${command.label} ${command.description ?? ''}`));
  useEffect(() => setIndex(0), [resolvedQuery, scope]);
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
          data-command-query-semantics="boolean"
          aria-activedescendant={filtered[index] ? `${listId}-${filtered[index].id}` : undefined}
          ariaLabel={placeholder}
          aria-controls={listId}
          autoFocus
          onChange={(event) => {
            const nextQuery = event.currentTarget.value;
            if (query === undefined) setInternalQuery(nextQuery);
            onQueryChange?.(nextQuery);
          }}
          onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
            if (event.key === 'Escape') onClose();
            if (event.key === 'ArrowDown') { event.preventDefault(); setIndex((value) => Math.min(filtered.length - 1, value + 1)); }
            if (event.key === 'ArrowUp') { event.preventDefault(); setIndex((value) => Math.max(0, value - 1)); }
            if (event.key === 'Home') { event.preventDefault(); setIndex(0); }
            if (event.key === 'End') { event.preventDefault(); setIndex(Math.max(0, filtered.length - 1)); }
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
          {filtered.length === 0 ? <VelvetState kind="empty" title="No commands found" /> : null}
          {filtered.map((command, commandIndex) => (
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

function parseVelvetCommandQuery(query: string): VelvetCommandQueryNode | null {
  const tokens = tokenizeVelvetCommandQuery(query);
  let cursor = 0;

  const isUnaryStart = (token: VelvetCommandQueryToken | undefined): boolean => token?.kind === 'term' || token?.kind === 'phrase' || token?.kind === 'lparen' || token?.kind === 'not';
  const parsePrimary = (): VelvetCommandQueryNode | null => {
    const token = tokens[cursor];
    if (!token) return null;
    if (token.kind === 'term' || token.kind === 'phrase') {
      cursor += 1;
      return { kind: 'term', phrase: token.kind === 'phrase', value: token.value };
    }
    if (token.kind === 'lparen') {
      cursor += 1;
      const nested = parseOr();
      if (tokens[cursor]?.kind === 'rparen') cursor += 1;
      return nested;
    }
    return null;
  };
  const parseUnary = (): VelvetCommandQueryNode | null => {
    if (tokens[cursor]?.kind === 'not') {
      cursor += 1;
      const child = parseUnary();
      return child ? { kind: 'not', child } : null;
    }
    return parsePrimary();
  };
  const parseAnd = (): VelvetCommandQueryNode | null => {
    const children: VelvetCommandQueryNode[] = [];
    const first = parseUnary();
    if (first) children.push(first);
    while (cursor < tokens.length) {
      if (tokens[cursor]?.kind === 'and') {
        cursor += 1;
        const next = parseUnary();
        if (next) children.push(next);
        continue;
      }
      if (!isUnaryStart(tokens[cursor])) break;
      const next = parseUnary();
      if (!next) break;
      children.push(next);
    }
    if (children.length === 0) return null;
    return children.length === 1 ? children[0] ?? null : { kind: 'and', children };
  };
  function parseOr(): VelvetCommandQueryNode | null {
    const children: VelvetCommandQueryNode[] = [];
    const first = parseAnd();
    if (first) children.push(first);
    while (tokens[cursor]?.kind === 'or') {
      cursor += 1;
      const next = parseAnd();
      if (next) children.push(next);
    }
    if (children.length === 0) return null;
    return children.length === 1 ? children[0] ?? null : { kind: 'or', children };
  }

  return parseOr();
}

function tokenizeVelvetCommandQuery(query: string): readonly VelvetCommandQueryToken[] {
  const tokens: VelvetCommandQueryToken[] = [];
  let cursor = 0;
  while (cursor < query.length) {
    const character = query[cursor];
    if (!character || /\s/.test(character)) {
      cursor += 1;
      continue;
    }
    if (character === '(') { tokens.push({ kind: 'lparen' }); cursor += 1; continue; }
    if (character === ')') { tokens.push({ kind: 'rparen' }); cursor += 1; continue; }
    if (character === '-' && (cursor === 0 || /\s|\(/.test(query[cursor - 1] ?? ''))) { tokens.push({ kind: 'not' }); cursor += 1; continue; }
    if (character === '|' && query[cursor + 1] === '|') { tokens.push({ kind: 'or' }); cursor += 2; continue; }
    if (character === '"') {
      const end = query.indexOf('"', cursor + 1);
      const value = query.slice(cursor + 1, end === -1 ? query.length : end).trim();
      if (value) tokens.push({ kind: 'phrase', value });
      cursor = end === -1 ? query.length : end + 1;
      continue;
    }
    const start = cursor;
    while (cursor < query.length && !/\s|[()|]/.test(query[cursor] ?? '')) cursor += 1;
    const value = query.slice(start, cursor);
    const operator = value.toUpperCase();
    if (operator === 'AND') tokens.push({ kind: 'and' });
    else if (operator === 'OR') tokens.push({ kind: 'or' });
    else if (operator === 'NOT') tokens.push({ kind: 'not' });
    else if (value) tokens.push({ kind: 'term', value });
  }
  return tokens;
}

function matchesVelvetCommandQuery(query: VelvetCommandQueryNode | null, text: string): boolean {
  if (!query) return true;
  const normalizedText = text.normalize('NFKC').toLocaleLowerCase();
  if (query.kind === 'term') return normalizedText.includes(query.value.normalize('NFKC').toLocaleLowerCase());
  if (query.kind === 'not') return !matchesVelvetCommandQuery(query.child, text);
  if (query.kind === 'and') return query.children.every((child) => matchesVelvetCommandQuery(child, text));
  return query.children.some((child) => matchesVelvetCommandQuery(child, text));
}
