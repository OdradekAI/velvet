# VelvetQuickSearch

`VelvetQuickSearch` is the host projection for fast navigation across folders,
items, tags and smart folders. It is separate from `VelvetCommandPalette`
because its result model, tab scopes, alias matching and history are search
navigation concerns rather than command execution.

## Contract

- **Props:** `open`, `tabs`, `activeTab`, `results`, `query`, `history`, `historyLimit`, `onQueryChange`, `onTabChange`, `onHistorySelect`, `onSelect`, `onClose`, `restoreFocusRef`
- **Tabs:** `folders`, `items`, `tags`, `smart-folders`
- **Result aliases:** `searchAliases` is supplied by the host for pinyin and simplified/traditional variants. The component matches aliases after NFKC normalization and case folding.
- **History:** the host owns persistence and ordering; the component deduplicates normalized values and applies `historyLimit` for the rendered list. The default limit is 100.
- **Keyboard:** ArrowUp/ArrowDown, Home, End, Enter and Escape operate on the result list; native tab buttons remain keyboard reachable.
- **Focus:** opening enters the input; closing restores `restoreFocusRef` when supplied.
- **Contract:** `apps/desktop/src/app/ui/contracts/host-projections/quick-search.contract.ts`
- **Stories:** `apps/desktop/src/app/ui/stories/host-projections/quick-search.stories.tsx`

The component never reads a library index, writes history, performs transliteration
or calls CoreTransport. A host search adapter prepares result aliases and scores,
then owns query, tab, selection and persistence state.

## Evidence boundary

The local shipped-app research records four quick-search modes, pinyin and
simplified/traditional matching, scoring, bounded history and Arrow/Enter/Escape
behavior. It also records focus entry and restoration. Renderer timing and
history persistence ownership remain unverified, so this contract keeps those
responsibilities at the host boundary.

## Stories and qualification

- `quick-search/open` verifies pinyin alias matching, active descendant and keyboard selection.
- `quick-search/tabs-history` verifies the four-tab projection and deduplicated history selection.
- `quick-search/keyboard-focus` verifies input focus entry, Escape close and trigger focus restoration.

The real-browser fixture additionally verifies pinyin, traditional alias
matching, tab switching, bounded history, Arrow/Enter selection and restored
trigger focus.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
