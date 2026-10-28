# ADR-0004: Preview providers, intents and augmentations

Status: Proposed
Date: 2026-10-28

## Context

Different users need different levels of preview. A general user wants a clean, fast image or model preview. A photographer may need RAW metadata, RGB channels, curves, histograms and color profiles. A 3D user may need animation, materials, lighting and scene inspection.

Eagle demonstrates that these capabilities can arrive through separate extensions: 3D and RAW extensions provide format handling and preview, PNG metadata adds inspector data, and AI Actions add workflows. Spacedrive also separates preview renderers, inspectors, views, commands and jobs in its extension UI model.

## Proposed decision

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

The host owns loading, error, empty, disabled-extension and fallback states. Extensions return typed contributions and capability handles; they do not replace the host layout or inject arbitrary DOM.

Preview caches are keyed by content fingerprint, extension version, decoder version and relevant user settings. Expensive work runs through the shared job system and can be cancelled.

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
