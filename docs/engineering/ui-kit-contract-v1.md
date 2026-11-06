# Velvet UI Kit contract v1

Status: Alpha host contract frozen in-repository. This document describes the
current framework-neutral surface used by the desktop UI bridge and the
host-owned renderer UI Kit. It is not a published third-party SDK and does not
select an iframe, child WebView or native renderer for future runtimes.

The frozen surface covers semantic tokens, theme negotiation, component
availability, semantic zones, origin checks and disposal/error behavior. It
does not freeze a production renderer, cross-platform accessibility
qualification or an external package ABI.

## Version surfaces

| Surface | Current version | Compatibility rule |
| --- | ---: | --- |
| UI bridge messages | `1` | A different bridge version fails the surface before activation. |
| Theme schema | `velvet.ui.theme/1` | Legacy snapshots are normalized; unknown schema versions fail closed. |
| Component contract | `1` | The host advertises version `1`; a surface may report one offered version and is failed and disposed when it is incompatible. |

The host owns layout, Activity Bar, panes, PreviewHost primary selection,
OperationsPanel, global overlays, focus restoration and global shortcuts. The
host renderer implements the primitives and composite components documented in
[the UI Kit component guide](ui-kit-component-guide.md). A
plugin surface receives projections and typed actions; it never receives React
elements, DOM nodes, host CSS classes, file paths or another View's
implementation.

## Theme snapshot

The host sends a read-only `velvet.ui.theme/1` snapshot:

```ts
interface UiThemeSnapshot {
  schema: 'velvet.ui.theme';
  schemaVersion: 1;
  scheme: 'light' | 'dark';
  density: 'compact' | 'default' | 'comfortable';
  contrast: 'normal' | 'high';
  reducedMotion: boolean;
  revision: number;
  tokens: Partial<Record<UiSemanticToken, string>>;
}
```

The public semantic token set is allow-listed in
`apps/desktop/src/workbench/plugins/ui-kit.ts`. Primitive colors, font files,
Tailwind classes and component implementation tokens remain host-private.
Unknown tokens, control characters and CSS values containing `url(`,
`expression(` or closing style tags are removed before the snapshot crosses
the surface boundary. A plugin cannot write a snapshot back to the host.

## Component availability

Plugin surfaces may describe or render token-compatible text, icon, button,
input, select, checkbox, tabs, menu, dialog body, toast, progress, loading,
empty, error, list, tree, table and grid content through the bridge contract.
The host retains ownership of Activity Bar, Navigation/Context/Operations
panes, global overlays, PreviewHost, OperationsPanel and global shortcuts.

Custom Views are registered through the host registry and remain inside their
declared semantic zone. A custom surface may request focus, resize, locale,
theme updates and registered command dispatch. It cannot create a second
layout owner, install a global key handler, inject arbitrary DOM/CSS or mutate
host state directly.

## Visual system and renderer boundary

The default host visual language is Eagle-informed rather than Eagle-compatible.
It uses the observed desktop workbench grammar as design input: dense
navigation/workspace/Inspector zones, near-flat charcoal surfaces, thin
separators, rounded selected rows, compact system-sans typography, one
saturated indigo/purple accent, icon-first toolbars with explicit search,
filter and zoom controls, and action-oriented empty states. The host may
support light, comfortable and high-contrast modes through the existing
semantic theme snapshot; those modes preserve the same component states and
ownership rules.

The visual system is expressed through Velvet semantic tokens and named
component variants. Primitive colors, Tailwind utility classes, DOM structure,
Eagle assets and exact pixel values are implementation details. A visual change
that changes keyboard behavior, state ownership or token meaning requires a UI
contract version decision; a palette or spacing refinement stays behind the
Velvet wrapper.

The public first-party renderer surface is the `Velvet*` React wrapper layer.
Wrappers expose semantic props, controlled state where the host owns selection
or overlay lifecycle, default state for local inputs, standard React events
and Velvet accessibility behavior. They do not expose Radix types, `asChild`,
Radix data attributes, Tailwind class names or host-private portals. Radix
Dialog and Context Menu remain internal primitives; additional Radix packages
are added one host use at a time after production bundle measurement.

The component set is split into three ownership groups:

1. **Primitives** — text, icon, button, icon button, input, select, checkbox,
   separator, stack, inline, tabs, progress, state and toast.
2. **Host composites** — Activity Bar, Pane, PreviewHost, AssetGrid,
   DataTable and CommandPalette. These retain host-owned context, focus,
   layout and projection state.
3. **Overlay wrappers** — Dialog and Context Menu today; Tooltip, Popover,
   DropdownMenu or ScrollArea only when a concrete host surface needs them.

Every group remains replaceable behind the semantic wrapper and the
framework-neutral plugin bridge. Isolated extensions receive component names,
theme tokens and typed overlay/command requests; they never receive React
elements or renderer implementation dependencies.

Story modules and the generated agent catalog are host development artifacts.
They verify the semantic wrapper behavior and document the supported states;
they do not expand the plugin bridge or turn story metadata into a runtime
permission surface.

## Surface and origin rules

The current host accepts only declared `http`, `https`, `tauri-webview`,
`asset`, `data`, `blob`, `wasm` and `builtin` entrypoint schemes. Browser
surfaces send messages to the exact network origin derived from the entrypoint
and discard messages from another origin; opaque schemes retain the source
window check. The iframe remains sandboxed. Tauri child WebViews continue to
use the existing explicit capability and lifecycle cleanup path.

## Failure and compatibility behavior

- An incompatible bridge, component contract or theme schema produces a typed
  host error and an isolated failed surface.
- Surface failure shows the host placeholder/retry path and does not change
  layout, Asset, Preview, Job or unrelated plugin state.
- Transport recovery disposes sessions, retains host descriptors and permits
  fresh sessions after authoritative resync.
- The contract is tested by `apps/desktop/test/ui-kit-contract.test.mjs`,
  `plugin-surface-adapter.test.mjs` and the existing surface lifecycle suites.

See the [plugin contract matrix](plugin-contract-matrix.md) for the complete
Rust/Tauri/frontend validation entry points and the [plugin developer
guide](plugin-developer-guide.md) for the plugin-facing UI boundary.

