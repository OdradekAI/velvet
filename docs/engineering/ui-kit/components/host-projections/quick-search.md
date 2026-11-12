# VelvetQuickSearch

`VelvetQuickSearch` is the host projection for fast navigation across folders,
items, tags and smart folders. It is separate from `VelvetCommandPalette`
because its result model and tab scopes are navigation projections rather than
command execution.

## Contract

- **Props:** `open`, `tabs`, `activeTab`, `results`, `query`, `history`, `onQueryChange`, `onTabChange`, `onHistorySelect`, `onSelect`, `onClose`, `restoreFocusRef`
- **Tabs:** `folders`, `items`, `tags`, `smart-folders`
- **Results:** `results` is already scoped, filtered and ordered by the host adapter. The component renders the list and reports selection.
- **History:** `history` is already ordered, deduplicated and bounded by the host. The component renders entries and reports history selection.
- **Keyboard:** ArrowUp/ArrowDown, Home, End, Enter and Escape operate on the result list; native tab buttons remain keyboard reachable.
- **Focus:** opening enters the input; closing restores `restoreFocusRef` when supplied.
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/quick-search.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/quick-search.stories.tsx`

The component never reads a library index, parses query syntax, performs
transliteration, ranks results, deduplicates history, writes history or calls
CoreTransport. A Workbench adapter prepares the result and history projections,
then owns query, tab, selection and persistence state. If `query` is omitted,
the component keeps a local input draft and still reports changes through
`onQueryChange`; supplying `query` makes the value host-controlled.

## Evidence boundary

The local shipped-app research records four quick-search modes, pinyin and
simplified/traditional matching, scoring, bounded history and Arrow/Enter/Escape
behavior. It also records focus entry and restoration. This UI contract keeps
the observed visual and keyboard surface while leaving search semantics,
ranking, indexing and history persistence at the Workbench boundary.

## Stories and qualification

- `quick-search/open` verifies host-provided results, active descendant and keyboard selection.
- `quick-search/tabs-history` verifies the four-tab projection and host-provided history selection.
- `quick-search/keyboard-focus` verifies input focus entry, Escape close and trigger focus restoration.

The real-browser fixture additionally verifies host result replacement, tab
switching, history rendering/selection, Arrow/Enter selection and restored
trigger focus.

## Validation

Run from `apps/desktop`:

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
