# Eagle directive behavior evidence

This is the versioned summary of the bounded Eagle source analysis used by the
Velvet UI Kit. It describes shipped-artifact evidence, not source compatibility
or runtime reachability for every static branch.

REA analyzed the extracted Eagle 4.0.0 artifact with exact selection of the
directive family below. The retained application Evidence is
`ev_16834b76767b31bdd19edc05c1a79910ce652466505ff131618514a865464a6f`.
Seven JavaScript sources were parsed; the remaining Eagle sources remain
outside the selected scope and explicitly unknown.

| Eagle source evidence | Observed behavior | Velvet consequence |
| --- | --- | --- |
| `app/js/directives/ng-grid-layout.js:539-566` | Grid/Square, List and Justified layout branches use different layout strategies and spacing. | `VelvetAssetGrid` exposes `grid`, `square`, `list` and `justified` without exposing InfiniteGrid. |
| `ng-grid-layout.js:78-101` | Items carry an aspect ratio, thumbnail, metadata labels, tags, rating and pin affordance. | Items accept `aspectRatio` and `pinned` as host-owned projection data. |
| `ng-grid-layout.js:53-55`, `80-81`, `208-212` | Thumbnail and item nodes are draggable; folder mode renders sortable helpers. | Drag/drop is optional and reports typed source/target ids through host callbacks. |
| `ng-grid-layout.js:696-729` | Selection, tagged and conditional pinned classes are projected from host mappings. | Selection and pin state stay controlled by the host. |
| `ng-grid-layout.js:600-610`, `792-807` | Layout completion registers lazy loading; removal unregisters before relayout. | Lazy loading and paging remain host data concerns, outside the component API. |
| `inspector.js:405-437` | Inspector fields differ for zero, one and many selected items. | Any future Inspector wrapper must consume host-owned selection state. |
| `inspector.js:162-205` | Name and URL changes route through host-owned change handlers; a blank single-item name is rejected before update. | Inspector editable fields are controlled by the host, with validation and save state projected back into the wrapper. |
| `quick-search-modal.js:15-47` | Open focuses/selects the input; close blurs and restores content focus. | Search overlays need explicit focus entry and restoration. |
| `quick-search-modal.js:49-120` | Search modes and bounded, deduplicated local history exist. | Eagle-specific modes/history stay out of the generic command contract until Velvet ownership is defined. |

The current correction includes AssetGrid layout/aspect-ratio/pinned/drag-drop
behavior and a controlled Inspector edit/save surface. Lazy-loading timing,
Inspector edit persistence and quick-search history ownership remain unknown and
require runtime or host-contract evidence before implementation.
