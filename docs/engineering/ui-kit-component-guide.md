# Velvet UI Kit component guide

Status: host implementation for UI Kit v1.

The renderer UI Kit is implemented in `apps/desktop/src/app/ui/index.ts`, the
layered modules under `primitives/`, `overlays/` and `host-projections/`, and
`apps/desktop/src/app/ui/theme.ts`. `velvet-ui.tsx` is a compatibility barrel;
new first-party imports use `ui/index.ts`. Its public behavior is semantic and
versioned; the React implementation and CSS class names remain host-private.

## Visual baseline

The default host theme follows an Eagle-informed desktop workbench grammar:
dense navigation/workspace/Inspector zones, charcoal near-flat surfaces, thin
separators, rounded selected rows, compact system-sans typography, a restrained
indigo/purple accent, icon-first toolbars and action-oriented empty states.
This is design input for Velvet, not pixel compatibility or permission to copy
Eagle assets. Light, comfortable and high-contrast modes reuse the same
semantic states and accessibility rules.

Use Velvet semantic tokens and component variants to style a surface. Tailwind
classes, CSS variable names, DOM structure and Radix data attributes are
implementation details and must not become extension or plugin contracts.

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
`VelvetCommandPalette` and `VelvetInspector` are host-owned renderer components. `VelvetPanel`,
`VelvetDialog` and `VelvetContextMenu` are the compatibility wrappers used by
existing Workbench surfaces and now use the same token, focus and state rules.

Primitive components provide semantic props and standard React events. The host
owns controlled selection, overlay and projection state; local form controls may
use default state. Dialog and Context Menu keep Radix behind the Velvet wrapper.
Future Radix primitives are admitted one host use at a time after bundle and
interaction qualification.

## Module map

The first migration wave gives every reusable primitive its own module:

```text
apps/desktop/src/app/ui/
  internal/                 # common props, data attributes and contract types
  primitives/<component>.tsx
  contracts/primitives/<component>.contract.ts
  stories/primitives/<component>.stories.tsx
  index.ts                  # formal first-party barrel
  velvet-ui.tsx             # compatibility re-export barrel
```

`overlays/` owns the Dialog, Context Menu and Toast implementations, and
`host-projections/` owns Panel, Activity Bar, Pane, PreviewHost, Inspector,
AssetGrid, DataTable and CommandPalette. Radix dependencies stay behind the Velvet
overlay wrappers. `legacy-components.tsx` is now a compatibility re-export
barrel only.

## Stories, contract tests and agent catalog

Each component owns a `*.stories.tsx` module with typed metadata and named
stories. Velvet maintains the story registry and runner; the first runner uses
the current TypeScript compile path, Node and Happy DOM rather than adding a
Storybook runtime dependency. Stories are executable fixtures, not screenshots
that can drift away from the component API.

Interactive stories may attach a typed lifecycle scenario with
`setup → actions → checkpoints → cleanup`. The runner provides a fake host
adapter that records semantic calls and an `act` boundary for React updates.
Checkpoints can assert DOM/ARIA, focus, emitted action payloads and recovery
state. Browser-only surfaces keep the same scenario shape but run through the
real-browser qualification scripts when portals, geometry or native focus
behavior are involved.

The unified UI Kit acceptance gate is `npm run verify:ui-kit` from
`apps/desktop`. It runs typed compile/catalog checks, the Node/Happy DOM
contract and story suite, the production build, and both real-browser
qualification layers. The browser layer expects the Vite fixture server at
`http://127.0.0.1:5173` (start it with `npm run dev` in another terminal).

The fake host adapter also exposes typed deferred operations for asynchronous
stories. They record `requested → pending → committed` for success and the
terminal `cancelled`, `timed-out` or `failed` states. `retry()` starts a fresh
pending attempt after a terminal failure. This keeps success, cancellation,
timeout, failure and retry deterministic without connecting a story to
`CoreTransport`.

Every interactive component covers its meaningful state matrix, keyboard
behavior and accessibility contract: default, selected/open, focus, disabled,
loading/error and empty states when applicable. Static layout components cover
their semantic structure and representative variants without inventing
interaction states. A story may use a fake host projection, but it must not
create a second state owner or bypass CoreTransport semantics.

The source of truth for discovery is a typed `ComponentMeta` record paired with
the component's typed `ComponentContract`. The build generates a JSON catalog
for developers and agents. Catalog entries describe the stable component id,
category, ownership layer, contract id, documentation path, semantic purpose,
props and events, supported states, keyboard/a11y guarantees, story names and
usage examples.
They do not publish CSS class names, Radix types, private DOM structure or
plugin-internal implementation details. The catalog is versioned with the UI
Kit contract and generated in CI so metadata cannot silently diverge from the
stories.

The current catalog inputs live in `apps/desktop/src/app/ui/component-meta.ts`,
`contracts/`, `story-registry.ts` and the independent `stories/` modules. The
generated output is `ui-kit-catalog.json`. Run
`npm run generate:ui-kit-catalog` from `apps/desktop` after changing metadata or
story names; CI must fail if generation produces a diff.

Host-owned surfaces include Activity Bar, Navigation, Context, Inspector,
Operations, PreviewHost, global overlays and global shortcuts. Plugins can contribute to
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
- Inspector consumes host-owned selection count and exposes controlled metadata
  edits with projected save, validation, cancellation and retry states.
- Dialogs restore focus through Radix; controlled dialogs with an external
  trigger pass `restoreFocusRef` so the Velvet wrapper can restore focus after
  Escape or outside dismissal. Actions stay in a host-owned footer.
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
