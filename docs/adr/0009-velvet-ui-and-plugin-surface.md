# ADR-0009: Velvet UI and plugin surface

Status: Accepted
Date: 2026-10-28

## Context

Velvet needs a serious desktop visual language for the Workbench shell,
context menus, dialogs, resizable panels and product views. The host also needs
to support first-party paid extensions without forcing every third-party
extension to depend on React, shadcn or the host's Tailwind build.

The UI implementation and the plugin contract have different compatibility
needs. The host may evolve its React components more often than the external
protocol can evolve.

## Decision

Use React with TypeScript and Vite for the current desktop renderer. Build the
host UI from owned shadcn-style source components and Tailwind CSS tokens, but
publish only Velvet-owned wrappers and semantic tokens to first-party
consumers.

The UI surface is split into four layers:

```text
Velvet UI React       first-party components and host-owned interaction rules
Velvet UI tokens      framework-independent CSS variables and visual semantics
Plugin SDK            descriptors, commands, queries, events and capabilities
Plugin UI bridge      isolated UI mounting and host-provided overlays
```

First-party or explicitly trusted extensions may consume the stable subset of
the Velvet UI React layer. Third-party extensions consume the Plugin SDK and,
when they need a custom surface, use an isolated UI surface with a typed
message channel. The host owns layout, overlays, focus management, command
dispatch and capability checks.

No extension may import internal shadcn source files, Base UI/Radix internals,
host Tailwind class names or framework-specific Workbench components as part of
the stable plugin contract.

## Consequences

Positive:

- First-party extensions share one visual system and interaction behavior.
- External extensions can use React, Vue, Svelte or plain HTML without changing
  the host renderer.
- Host UI implementation can be upgraded behind a small Velvet-owned surface.
- Layout and overlay state keep one owner in the host instead of spreading into
  plugin components.

Costs:

- Velvet must maintain a stable wrapper layer and token vocabulary.
- First-party extensions need compatibility testing against the supported UI
  surface.
- Isolated third-party UI requires a host bridge, lifecycle handling and theme
  synchronization.

## Compatibility rules

- `hostApi` versions protocol and capability behavior.
- `uiApi` versions the first-party Velvet UI wrapper surface.
- Tokens use semantic names and are not a public Tailwind class contract.
- A plugin may contribute a `WorkbenchApp`, views, commands, previews,
  inspectors and jobs, but it cannot own Workbench layout or the global overlay
  layer. A `WorkbenchApp` owns only its central `AppSurface` and declared
  compatible ViewContainers; shared Asset and WorkspaceContext data remain
  host/core-owned.

The first UI Bridge implementation uses a versioned message channel with
explicit mount, activate, suspend, focus, resize, theme, command, error and
dispose messages. A failed surface transitions to an isolated failed state and
is cleaned up without disabling the Workbench or unrelated extensions.
