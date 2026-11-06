# Velvet UI Kit component guide

Status: host implementation for UI Kit v1.

The renderer UI Kit is implemented in `apps/desktop/src/app/ui/velvet-ui.tsx`
and `apps/desktop/src/app/ui/theme.ts`. Its public behavior is semantic and
versioned; the React implementation and CSS class names remain host-private.

## Version surfaces

- `UI_KIT_API_VERSION` is `1.0` for host component behavior.
- `velvet.ui.theme/1` is the read-only semantic token snapshot sent to plugin
  surfaces.
- UI bridge component contract `1` is negotiated before a surface becomes
  active. An incompatible surface is failed and disposed in isolation.

Additive fields and optional tokens are compatible. A changed token meaning,
keyboard model or state owner requires a new major contract version and a
fixture for the old version. The host may replace React, Radix or CSS
internals without changing the contract.

## Host components

`VelvetButton`, `VelvetIconButton`, `VelvetInput`, `VelvetSelect`,
`VelvetCheckbox`, `VelvetText`, `VelvetStack`, `VelvetInline`,
`VelvetSeparator`, `VelvetTabs`, `VelvetProgress`, `VelvetState`,
`VelvetToastProvider`, `VelvetToastHost`, `VelvetActivityBar`, `VelvetPane`,
`VelvetPreviewHost`, `VelvetAssetGrid`, `VelvetDataTable` and
`VelvetCommandPalette` are host-owned renderer components. `VelvetPanel`,
`VelvetDialog` and `VelvetContextMenu` are the compatibility wrappers used by
existing Workbench surfaces and now use the same token, focus and state rules.

Host-owned surfaces include Activity Bar, Navigation, Context, Operations,
PreviewHost, global overlays and global shortcuts. Plugins can contribute to
those surfaces through registries and bridge requests, but cannot import the
React components, change the Workbench grid, create a global portal or attach a
document-level keyboard listener.

## State and accessibility rules

- Buttons dispatch host commands or local surface actions; command visibility
  and capabilities are checked by the host before execution.
- Tabs use roving tab focus with Arrow, Home and End navigation.
- Activity Bar uses roving tab focus with Arrow, Home and End navigation.
- Asset Grid exposes `role=grid`, selection and Arrow/Enter behavior. Core
  selection remains owned by the host projection.
- Data Table exposes a labelled table, sortable headers and focusable rows.
- Dialogs restore focus through Radix and keep actions in a host-owned footer.
- Loading, empty and error states use `status`/`alert` semantics; errors offer
  an explicit retry callback when available.
- Progress is a projection of host Job state. Rendering progress cannot imply
  Job completion.
- Toasts use a live region and explicit dismissal; notifications do not mutate
  core state.
- `reducedMotion`, density and contrast are carried by the theme snapshot and
  reflected in host CSS variables.

## Plugin adapter boundary

An isolated surface receives only the semantic token snapshot, locale, bounded
context, size, focus and lifecycle messages. It may send a typed command,
overlay request, notification or progress projection through the UI Bridge.
The host adapter decides whether to render a Context Menu, Dialog, Toast or
Progress surface and rechecks command, context and capability state before
acting. A surface failure or plugin disable removes its sessions and leaves the
Workbench, Asset, Preview and Job owners usable.

The contract fixture is `apps/desktop/test/ui-kit-contract.test.mjs`; renderer
component and keyboard fixtures are in
`apps/desktop/test/ui-kit-components.test.mjs`. Run the full desktop suite with
`npm test` from `apps/desktop`.
