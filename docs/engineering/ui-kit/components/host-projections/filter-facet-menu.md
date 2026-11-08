# VelvetFilterFacetMenu

`VelvetFilterFacetMenu` is a host projection for the two facet menus that need
their own interaction model: folder selection and color selection. It keeps the
folder hierarchy, search results, color matching and persistence outside the UI
Kit.

## Contract

- **Props:** `kind`, `open`, `options`/`swatches`, `query`, `rule`, `selectedColor`, `graySelected`, `hexValue`, `accuracy` and the corresponding host callbacks.
- **Folder geometry:** `360px` surface, `40px` header, `26px` option rows, OR/AND/EQUAL tabs and a `40px` footer.
- **Color geometry:** `202px` surface, `20px` swatches, `32px` hex input and accuracy range `5–40`.
- **Actions:** folder include/exclude, rule change, query change, color preset, hex, accuracy, clear and close.
- **Ownership:** the host owns filtered options, folder hierarchy, color matching, selected state and persistence.

Left-click on a folder option emits `include`; context-click emits `exclude`.
The component does not execute a query or infer a folder anchor. Color changes
only report the selected value or accuracy to the host.

## Stories

- `filter-facet-menu/folder-default`
- `filter-facet-menu/color-default`
- `filter-facet-menu/closed`

The contract and story runner cover the observed geometry, folder rule tabs,
include/exclude actions, color presets, hex input, accuracy range and closed
state. Browser qualification is intentionally still `node`; a later browser
pass will add portal, pointer geometry and native context-menu qualification.

## Evidence

See the ignored research record
`docs/research/eagle/eagle-analysis/component-style-evidence.md` for the
module-level SCSS, templates and behavior evidence used by this wrapper.

## Validation

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```
