# Eagle populated-library evidence

This record captures a bounded populated-library probe against the installed
Eagle 4.0.0 application. The probe used a temporary `Velvet Probe` folder in
the existing empty library and twelve locally generated SVG fixtures. The
temporary folder was removed after the probe with the user's explicit
authorization; no existing user assets were changed.

The current live REA native endpoint did not attach to the running process in
this pass (`target_unavailable`), so the interaction claims below are from the
controlled native CUA observation. Existing REA shipped-artifact evidence
remains the source for directive structure and static behavior; this record
does not turn CUA observations into source compatibility claims.

## Observed matrix

| Surface | Controlled observation | Velvet consequence | Confidence |
| --- | --- | --- | --- |
| Zoom step | The native `AXSlider` started at `180`. Repeated Increment actions moved it in 5-point steps (`155`, then `160`, `165`, `170`, `175`, `180`, and later `230`). | Keep `VelvetZoomControl` at step `5`; do not claim Eagle's full range. | observed |
| Grid geometry | At value `180`, the populated central area showed eight columns. At value `230`, the same window showed five columns with larger cells. The grid reflowed without changing the item aspect ratio (`320 × 240`). | Zoom is host-owned layout input. `AssetGrid` must accept controlled layout/column projections and must not own zoom persistence. | observed |
| Single selection | Clicking a cell added a blue selection outline and changed the persistent Inspector to the selected asset, including preview, name, tags/folder and file metadata. | `selected` remains controlled by the host; Inspector derives from the same selection owner. | observed |
| Range selection | Shift-clicking a second cell selected two cells and the Inspector announced `已选 2 个文件`. | `onSelect` reports a `range` modifier hint; the host owns the anchor and range expansion. | observed |
| Keyboard movement | After selecting a cell, `ArrowRight` moved the blue selection to the next visible cell. The accessibility tree continued to expose the HTML content container rather than a named cell focus target. | Keep keyboard navigation in the wrapper, but mark cell focus exposure as an Eagle unknown. Browser qualification remains the authoritative focus contract for Velvet. | observed / unknown |
| Format filter | Clicking `格式` opened a popover with a focused search field, a `svg` checkbox with count `12`, and explicit left-click select/right-click exclude guidance. Selecting it added an `svg` chip, a filter badge `1` and `搜索结果 (12)`. | Filters are extensible action projections. Do not hard-code format values into `AssetGrid`; expose host filter slots/actions. | observed |
| Content search | Filling the content search with `eagle-probe-12` changed the heading to `搜索结果 (1)` and left one matching cell. Pressing Escape cleared the query and returned focus to the content container. | Search state belongs to the host; a search wrapper needs clear and focus restoration coverage. | observed |
| Metadata | Each populated cell exposed type (`SVG`), filename, dimensions (`320 × 240`) and the Inspector exposed file size, format and dates. | `item.metadata` is the public projection slot; the grid does not invent Eagle-specific metadata fields. | observed |

## Contract decisions

- `VelvetAssetGrid` reports `onSelect(id, additive, range)` while preserving the
  existing host ownership rule. Shift is a hint; the component does not store
  an anchor or expand a range itself.
- Zoom remains a separate `VelvetZoomControl` host projection. The populated
  probe proves that zoom changes grid geometry, but it does not prove a stable
  formula or range, so the visual fixture may use an explicit host mapping
  without presenting it as Eagle compatibility.
- Format/search popovers remain host concerns. The current AssetGrid contract
  records selection and layout semantics but does not add an unobserved filter
  persistence API.

## Remaining unknowns

- Eagle's exact zoom range, persistence across restarts and layout formula.
- Whether `ArrowDown` follows a fixed row stride at every zoom value.
- Whether Command-click, Ctrl-click and Shift-click differ across macOS and
  Windows.
- Filter exclusion semantics after right-click, saved-filter persistence and
  interaction between content search and facet filters.
- Whether Eagle exposes a DOM/AX cell focus target in a different window mode.
