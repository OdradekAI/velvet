# ADR-0004: Preview providers, intents and augmentations

Status: Accepted
Date: 2026-10-28

## Context

Different users need different levels of preview. A general user wants a clean, fast image or model preview. A photographer may need RAW metadata, RGB channels, curves, histograms and color profiles. A 3D user may need animation, materials, lighting and scene inspection.

Eagle demonstrates that these capabilities can arrive through separate extensions: 3D and RAW extensions provide format handling and preview, PNG metadata adds inspector data, and AI Actions add workflows. Spacedrive also separates preview renderers, inspectors, views, commands and jobs in its extension UI model.

## Decision

Use one host-managed `PreviewHost` with three stable user intents:

```rust
enum PreviewIntent {
    Quick,
    Inspect,
    Analyze,
}
```

Separate the extension interfaces:

```text
ContentKindProvider   recognizes and describes a content kind
PreviewProvider       owns the primary preview surface
PreviewAugmentation   adds overlays or controls
InspectorContribution adds metadata and analysis sections
PreviewTool           adds commands and toolbar operations
AnalysisJob           performs expensive asynchronous analysis
```

For one asset, intent and surface, the resolver chooses one primary `PreviewProvider`; multiple augmentations may attach. Resolution order is explicit user choice, user profile default, provider priority and built-in fallback.

The first Workbench host maps `Quick` to a host-owned preview overlay when a
popup is appropriate; `Inspect` and `Analyze` may render through an
`AppSurface` region, inspector or another declared preview surface. The frame,
loading/error/disabled/fallback states, focus, theme and accessibility remain
host-owned. Built-in and product-plugin providers use the same public seam;
plugins extend declared augmentation, inspector and tool slots without
replacing the host frame.

The host owns loading, error, empty, disabled-extension and fallback states. Extensions return typed contributions and capability handles; they do not replace the host layout or inject arbitrary DOM.

Preview caches are keyed by content fingerprint, extension version, decoder version and relevant user settings. Expensive work runs through the shared job system and can be cancelled.

The host/provider boundary is explicit. `PreviewHost` drives the lifecycle
`requested → resolving → loading → ready ↔ suspended`, with typed failed,
retrying, disabled, unsupported and disposed states. A `PreviewContext` carries
an opaque Asset handle, ContentKind, intent, target surface, WorkspaceContext
projection, theme, locale, measured size, focus, granted capabilities,
session id, cancellation and deadline. It never exposes a database connection,
unrestricted path or another extension session.

Providers return a framework-neutral `PreviewSessionDescriptor` describing the
render adapter/UI Bridge version, supported commands and augmentation slots.
Trusted and isolated providers use the same descriptor and lifecycle; only the
renderer transport differs. The host owns the Quick overlay/AppSurface frame,
placement, focus, accessibility, theme, loading/error/fallback and disposal.
Providers never return React/Vue components, inject arbitrary DOM or replace
the frame. Late events are ignored after session disposal or a session-id
change.

The v1 runtime seam carries `method: "preview.open"` over the existing
isolated JSONL invocation channel. Rust validates the capability-scoped
`PreviewProvider`, requires `preview-render`, sorts matching candidates and
parses the provider result into the session descriptor. Invocation timeout,
runtime failure or malformed descriptors skip that provider and leave the
built-in generic fallback available. The desktop `PreviewRegistry` mirrors
provider and augmentation declarations and removes them with the extension
lifecycle; it does not execute provider code or mount arbitrary DOM. A
fallback response carries the first non-fatal provider diagnostic so the host
can show a usable preview and a supportable explanation at the same time.

`PreviewResolver` selects one Primary provider from Asset, ContentKind, intent,
surface and Workspace/Profile context. The deterministic order is explicit
user choice, Profile/Workspace default, selector match, provider priority,
provider id and the built-in generic fallback. Equal-priority ambiguity is a
diagnostic decision. A provider cannot bypass the resolver or claim the host
directly.

Augmentations, Inspector sections and tools register only declared slots.
Augmentations do not replace the Primary provider; their loading/error/disabled
state is independent. Inspector and toolbar actions consume typed projections
and dispatch host Commands. Decode, parse, thumbnail, histogram and analysis
work goes through the Rust `JobSystem`, and provider/capability caches are
owned by their producer and invalidated by content, provider, decoder, schema
or settings changes. A stale cache is never reported as a current result.

Editing is a separate `Editor` concern. Preview providers may expose non-destructive tools, but mutations go through registered commands and transactions.

## Examples

```text
3D extension:
  ContentKindProvider + PreviewProvider + ThumbnailProvider
  + InspectorContribution + PreviewTool + AnalysisJob

RAW extension:
  ContentKindProvider + PreviewProvider + MetadataProvider
  + HistogramAnalyzer + InspectorContribution

PNG metadata extension:
  InspectorContribution only

AI action extension:
  AssetAction + Command + Job
```

## Alternatives considered

1. Give every format one complete viewer page. Rejected because clean preview and technical analysis would be duplicated and plugin composition would be impossible.
2. Let every extension replace the entire preview host. Rejected because lifecycle, accessibility, theming, error handling and fallback would become inconsistent.
3. Treat RAW analysis as synchronous renderer work. Rejected because decoding and analysis can block the UI and consume substantial memory.

## Consequences

Positive:

- The same RAW or 3D provider can serve different user profiles.
- Metadata-only extensions can coexist with built-in renderers.
- A disabled plugin leaves a usable generic or built-in preview.

Costs:

- Resolver conflicts and contribution ordering need contract tests.
- The host needs a stable preview context and capability model.

## References

- [Eagle plugin center](https://community-cn.eagle.cool/plugins)
- [Eagle 3D format extension](https://community-cn.eagle.cool/plugin/89fb801f-7761-4de6-b7ce-3785a084edaf)
- [Eagle RAW format extension](https://community-cn.eagle.cool/plugin/eagle-extension-raw)
- [Eagle PNG metadata extension](https://community-cn.eagle.cool/plugin/c4079626-c5e9-44c2-bab0-9f90f3bb4fe5)
- [VS Code custom editors](https://code.visualstudio.com/api/extension-guides/custom-editors)
- [Spacedrive UI integration](https://github.com/spacedriveapp/spacedrive/blob/main/docs/extensions/ui-integration.mdx)
