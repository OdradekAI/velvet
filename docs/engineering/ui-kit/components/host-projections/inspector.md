# VelvetInspector

`VelvetInspector` is the host-owned complementary projection for the current
selection. It keeps the zero, one and many selection states in one semantic
surface and leaves selection, persistence and transport ownership with the
Workbench host.

## Contract

- **Props:** `title`, `selectedCount`, `preview`, `fields`, `emptyMessage`, `onFieldChange`, `onSave`, `onRetry`, `saveState`, `saveError`, `pinned`, `onPin`, `onOpen`, `children`
- **States:** empty, single, multiple, pinned, editing, saving, saved, error, cancelled
- **Actions:** pin, open, edit, save, retry
- **Slots:** preview, metadata, actions, children
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/inspector.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/inspector.stories.tsx`
- **Qualification:** both Node/Happy DOM and real browser

`selectedCount` is a host projection. The component does not inspect the Asset
store or derive selection itself. Editable fields are controlled through
`onFieldChange`; save state and validation messages come from the host. The
component does not persist edits or infer save success.

## Evidence

The shape follows the shipped Inspector evidence: different field sets
appear for zero, one and many selected items, routes name/URL changes
through host handlers, and rejects an empty single-item name. Product-specific
edit persistence and plugin Inspector activation remain separate host contracts.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
