# VelvetFilterPanel

`VelvetFilterPanel` is a host-owned projection for composable query facets. The host owns the `rules` array; the component only renders controls and reports a new rule set through `onRuleChange`.

Supported facet kinds in this first slice:

- `text`: substring-style input (`contain` operator).
- `single`: one option (`equal` operator).
- `multi`: multiple options (`intersection` operator).
- `range`: host-defined range value (`between` operator).

The component does not read the database, execute a search, or infer result counts. Counts are supplied by the host on each option. `onReset`, `onSave`, and `onClose` are semantic host actions.

```tsx
const facets = [
  { id: 'type', label: 'Type', kind: 'multi', options: [{ id: 'image', label: 'Image', count: 12 }] },
  { id: 'annotation', label: 'Annotation', kind: 'text', placeholder: 'Contains…' },
] as const;

<VelvetFilterPanel
  facets={facets}
  rules={rules}
  onRuleChange={setRules}
  onReset={() => setRules([])}
  onSave={(nextRules) => saveFilter(nextRules)}
/>
```

The contract is in `apps/desktop/src/app/ui/contracts/host-projections/filter-panel.contract.ts`; executable scenarios are in `apps/desktop/src/app/ui/stories/host-projections/filter-panel.stories.tsx`.
