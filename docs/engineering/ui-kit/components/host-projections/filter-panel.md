# VelvetFilterPanel

`VelvetFilterPanel` is a host-owned projection for composable query facets. The host owns the `rules` array; the component only renders controls and reports a new rule set through `onRuleChange`.

Supported facet kinds in this first slice:

- `text`: substring-style input (`contain` operator).
- `single`: one option (`equal` operator).
- `multi`: multiple options (`intersection` operator).
- `range`: host-defined range value (`between` operator).

Semantic facets may add a stable `semantic` id: `folder`, `tag`, `color`,
`rating` or `date`. The default operator mapping is `folder → equal`,
`tag (multi) → intersection`, `color → equal`, `rating (range) → between`
and `date (range) → within`. A facet may provide an explicit `operator` when
the host needs `before`, `after`, `not` or `or`; the component reports that
operator and never executes the query itself.

Rules use the typed operator set `contain`, `equal`, `intersection`, `between`,
`before`, `after`, `within`, `not` and `or`. The panel does not interpret those
operators; it reports the rule shape to the host. `resultCount` is an announced
host projection, `onApply` commits the current rules, and `onSave` persists a
named filter through the host.

The component does not read the database, execute a search, or infer result counts. Counts are supplied by the host on each option. `onReset`, `onSave`, and `onClose` are semantic host actions.

```tsx
const facets = [
  { id: 'type', label: 'Type', kind: 'multi', options: [{ id: 'image', label: 'Image', count: 12 }] },
  { id: 'annotation', label: 'Annotation', kind: 'text', placeholder: 'Contains…' },
  { id: 'folder', label: 'Folder', semantic: 'folder', kind: 'single', options: [{ id: 'reference', label: 'Reference' }] },
  { id: 'tag', label: 'Tag', semantic: 'tag', kind: 'multi', options: [{ id: 'illustration', label: 'Illustration' }] },
  { id: 'rating', label: 'Rating', semantic: 'rating', kind: 'range', placeholder: '0–5' },
  { id: 'date', label: 'Date', semantic: 'date', kind: 'range', placeholder: 'YYYY-MM-DD..YYYY-MM-DD' },
] as const;

<VelvetFilterPanel
  facets={facets}
  resultCount={12}
  rules={rules}
  onApply={(nextRules) => applyRules(nextRules)}
  onRuleChange={setRules}
  onReset={() => setRules([])}
  onSave={(nextRules) => saveFilter(nextRules)}
/>
```

The contract is in `apps/desktop/src/app/ui/contracts/host-projections/filter-panel.contract.ts`; executable scenarios are in `apps/desktop/src/app/ui/stories/host-projections/filter-panel.stories.tsx`.

The semantic names and operator families are grounded in the local shipped-app
research matrix: `docs/research/eagle/eagle-analysis/report.md` and
`docs/research/eagle/eagle-analysis/evidence-matrix.json`. The research remains
ignored product evidence; this component exposes only the stable Velvet schema.
