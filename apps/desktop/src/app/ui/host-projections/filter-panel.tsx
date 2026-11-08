import type { ChangeEvent } from 'react';
import { VelvetButton, VelvetCheckbox, VelvetInput, VelvetSelect } from '../primitives/index.js';
import type { UiCommonProps } from '../internal/types.js';

export type VelvetFilterFacetKind = 'text' | 'single' | 'multi' | 'range';
export type VelvetFilterOperator = 'contain' | 'equal' | 'intersection' | 'between' | 'before' | 'after' | 'within' | 'not' | 'or';
export type VelvetFilterRuleValue = string | number | readonly string[];

export interface VelvetFilterOption {
  readonly id: string;
  readonly label: string;
  readonly count?: number;
  readonly disabled?: boolean;
}

export interface VelvetFilterFacet {
  readonly id: string;
  readonly label: string;
  readonly kind: VelvetFilterFacetKind;
  readonly options?: readonly VelvetFilterOption[];
  readonly placeholder?: string;
}

export interface VelvetFilterRule {
  readonly facetId: string;
  readonly operator?: VelvetFilterOperator;
  readonly value: VelvetFilterRuleValue;
}

export interface VelvetFilterPanelProps extends UiCommonProps {
  readonly open?: boolean;
  readonly title?: string;
  readonly facets: readonly VelvetFilterFacet[];
  readonly rules: readonly VelvetFilterRule[];
  readonly resultCount?: number;
  readonly onRuleChange: (rules: readonly VelvetFilterRule[]) => void;
  readonly onApply?: (rules: readonly VelvetFilterRule[]) => void;
  readonly onReset?: () => void;
  readonly onSave?: (rules: readonly VelvetFilterRule[]) => void;
  readonly onClose?: () => void;
  readonly className?: string;
}

export function VelvetFilterPanel({
  open = true,
  title = 'Filters',
  facets,
  rules,
  resultCount,
  onRuleChange,
  onApply,
  onReset,
  onSave,
  onClose,
  className = '',
  ...common
}: VelvetFilterPanelProps) {
  if (!open) return null;

  const changeRule = (facet: VelvetFilterFacet, value: VelvetFilterRuleValue | null): void => {
    const nextRules = rules.filter((rule) => rule.facetId !== facet.id);
    if (value !== null && (!Array.isArray(value) || value.length > 0) && value !== '') {
      nextRules.push({ facetId: facet.id, operator: operatorFor(facet.kind), value });
    }
    onRuleChange(nextRules);
  };

  return (
    <section
      {...common}
      aria-label={common.ariaLabel ?? title}
      className={`velvet-filter-panel ${className}`.trim()}
      data-ui-kit-component="filter-panel"
      role="region"
    >
      <header className="velvet-filter-panel__header">
        <h2>{title}</h2>
        {resultCount === undefined ? null : <output aria-live="polite" data-filter-result-count="true">{resultCount} results</output>}
        {onClose ? <VelvetButton onClick={onClose} variant="quiet">Close</VelvetButton> : null}
      </header>
      <div className="velvet-filter-panel__facets">
        {facets.map((facet) => {
          const rule = rules.find((candidate) => candidate.facetId === facet.id);
          return (
            <fieldset className="velvet-filter-panel__facet" key={facet.id}>
              <legend>{facet.label}</legend>
              {facet.kind === 'text' ? (
                <VelvetInput
                  ariaLabel={facet.label}
                  onChange={(event: ChangeEvent<HTMLInputElement>) => changeRule(facet, event.currentTarget.value)}
                  placeholder={facet.placeholder}
                  value={typeof rule?.value === 'string' ? rule.value : ''}
                />
              ) : null}
              {facet.kind === 'single' ? (
                <VelvetSelect
                  ariaLabel={facet.label}
                  onChange={(event: ChangeEvent<HTMLSelectElement>) => changeRule(facet, event.currentTarget.value || null)}
                  value={typeof rule?.value === 'string' ? rule.value : ''}
                >
                  <option value="">All</option>
                  {(facet.options ?? []).map((option) => (
                    <option disabled={option.disabled} key={option.id} value={option.id}>
                      {option.label}{option.count === undefined ? '' : ` (${option.count})`}
                    </option>
                  ))}
                </VelvetSelect>
              ) : null}
              {facet.kind === 'multi' ? (
                <div className="velvet-filter-panel__options">
                  {(facet.options ?? []).map((option) => {
                    const selected = Array.isArray(rule?.value) && rule.value.includes(option.id);
                    const nextValue = Array.isArray(rule?.value) ? [...rule.value] : [];
                    if (selected) nextValue.splice(nextValue.indexOf(option.id), 1);
                    else nextValue.push(option.id);
                    return (
                      <VelvetCheckbox
                        checked={selected}
                        disabled={option.disabled}
                        key={option.id}
                        label={`${option.label}${option.count === undefined ? '' : ` (${option.count})`}`}
                        onCheckedChange={() => changeRule(facet, nextValue)}
                      />
                    );
                  })}
                </div>
              ) : null}
              {facet.kind === 'range' ? (
                <VelvetInput
                  ariaLabel={facet.label}
                  onChange={(event: ChangeEvent<HTMLInputElement>) => changeRule(facet, event.currentTarget.value)}
                  placeholder={facet.placeholder ?? 'Enter a range'}
                  value={typeof rule?.value === 'string' || typeof rule?.value === 'number' ? String(rule.value) : ''}
                />
              ) : null}
            </fieldset>
          );
        })}
      </div>
      <footer className="velvet-filter-panel__actions">
        {onReset ? <VelvetButton onClick={onReset} variant="quiet">Reset</VelvetButton> : null}
        {onSave ? <VelvetButton onClick={() => onSave(rules)}>Save filter</VelvetButton> : null}
        {onApply ? <VelvetButton data-filter-action="apply" onClick={() => onApply(rules)}>Apply</VelvetButton> : null}
      </footer>
    </section>
  );
}

function operatorFor(kind: VelvetFilterFacetKind): VelvetFilterOperator {
  if (kind === 'text') return 'contain';
  if (kind === 'multi') return 'intersection';
  if (kind === 'range') return 'between';
  return 'equal';
}
