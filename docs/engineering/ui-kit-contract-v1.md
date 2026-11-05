# Velvet UI Kit contract v1

Status: host contract slice. This document describes the current framework-neutral
surface used by the desktop UI bridge. It is not a published third-party SDK and
does not select an iframe, child WebView or native renderer for future runtimes.

## Version surfaces

| Surface | Current version | Compatibility rule |
| --- | ---: | --- |
| UI bridge messages | `1` | A different bridge version fails the surface before activation. |
| Theme schema | `velvet.ui.theme/1` | Legacy snapshots are normalized; unknown schema versions fail closed. |
| Component contract | `1` | The host advertises version `1`; a surface may report one offered version and is failed and disposed when it is incompatible. |

The host owns layout, Activity Bar, panes, PreviewHost primary selection,
OperationsPanel, global overlays, focus restoration and global shortcuts. A
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

