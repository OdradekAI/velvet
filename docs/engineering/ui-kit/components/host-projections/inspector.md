# VelvetInspector

`VelvetInspector` is the host-owned complementary projection for the current
selection. It keeps the zero, one and many selection states in one semantic
surface and leaves selection, persistence and transport ownership with the
Workbench host.

## Contract

- **Props:** `title`, `selectedCount`, `preview`, `fields`, `sections`, `palette`, `emptyMessage`, `onFieldChange`, `onSave`, `onRetry`, `saveState`, `saveError`, `pinned`, `onPin`, `onOpen`, `onSectionToggle`, `children`
- **States:** empty, single, multiple, pinned, editing, saving, saved, error, cancelled, grouped, palette, preview-landscape, preview-portrait, no-preview
- **Actions:** pin, open, edit, save, retry, toggle-section
- **Slots:** preview, metadata, sections, palette, actions, children
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/inspector.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/inspector.stories.tsx`
- **Qualification:** both Node/Happy DOM and real browser

`selectedCount` is a host projection. The component does not inspect the Asset
store or derive selection itself. Multiple selection displays a compact selected
count. Editable fields are controlled through
`onFieldChange`; save state and validation messages come from the host. The
component does not persist edits or infer save success. `sections` supplies
labelled, host-expandable field groups, while `palette` renders precomputed
color ratios supplied by the host as a compact swatch pill; the renderer never
inspects files or calculates colors. The preview region has a 200px minimum height; its content is constrained to
a 200px maximum height and preserves the supplied asset aspect ratio. Landscape
and portrait assets use the same slot; assets without preview content omit the slot while
keeping metadata visible. The host may render a multi-selection summary without
preview content.

The semantic visual contract reserves a compact 250px Inspector column, 28px
editable controls and grouped sections on the panel surface. The host remains
the owner of selection, field values, persistence and save lifecycle.

## Evidence

The shape follows the shipped Inspector evidence: different field sets
appear for zero, one and many selected items, routes name/URL changes
through host handlers, and rejects an empty single-item name. Product-specific
edit persistence and plugin Inspector activation remain separate host contracts.
The evidence record for the shipped app is
`docs/research/eagle/eagle-analysis/component-style-evidence.md`; its source
matrix includes `inspector.html`, `inspector-information.html` and
`style/components/_inspector.scss`. The populated-library matrix also records
single selection, range selection and the Inspector's selected-count branch in
`docs/research/eagle/ui-kit/populated-evidence.md`.

The qualification fixture exposes four host-owned scenarios with stable test
IDs: `inspector-scenario-landscape`, `inspector-scenario-portrait`,
`inspector-scenario-no-preview` and `inspector-scenario-multiple`. The Playwright
and CUA browser runners assert selection mode, preview presence, aspect ratio,
section count, palette count and the selected-count text for each.

## Validation

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
