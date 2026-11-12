---
id: capabilities
title: Capability catalog
---

# Capability catalog

Use this catalog to find an existing seam. **Alpha** describes its compatibility promise; **fixture**, **host implementation** and **release gate** describe different implementation evidence. None of these entries is a published SDK package.

| Task | Declaration or call | Owner and required boundary | Current path |
| --- | --- | --- | --- |
| Run an action | `command`, `menu`, `context-menu`, `keybinding`; `invoke-command` | PluginHost and command registry validate identity, context and permissions | [Commands and Views](../guides/commands-and-views); Echo sidecar fixture |
| Add a View | `view-container`, `view`, `companion-view`, `operation-view` | Workbench owns container, scope and zone | Echo manifest and [UI Bridge](./ui-bridge) |
| Add a product app | `workbenchApp`, `app-surface`, `activity-bar-entry` | WorkbenchApp registry owns activation and surface lifecycle | Host contracts and lifecycle tests; no standalone SDK starter |
| Provide a preview | `preview-provider`, `preview-augmentation`, `inspector` | PreviewHost owns selection and fallback; permission policy remains host-owned | Echo preview fixture and [Asset analysis](../guides/asset-analysis) |
| Run background work | `job`; `submit-job`, `query-job`, `cancel-job` | Rust JobSystem; caller ownership and capability checks on each request | [Host services](/reference/plugin-api-index); local contract tests |
| Read Asset data or write a facet | Fixture `assets.readProjection`, `facets.read`, `facets.write` | Asset identity stays core-owned; writes require namespace, schema and current fingerprint | Asset Inspector fixture only; these method names are not shipped SDK APIs |
| Store extension data | Manifest `storage`; fixture `storage.read/write/migrate` | PluginHost ExtensionStorage, quota and transactional migration | Host storage implementation; convenience facade remains fixture-only |
| Read configuration | Fixture `configuration.version()` | Profile/configuration owner; no arbitrary configuration mutation API | Asset Inspector fixture only |
| Translate an extension | Manifest `localization`; `host.locale` | Host bundle loader and locale projection; bounded package resources | Host localization implementation and Bridge contract |
| Report diagnostics or request interaction | `log`, `notify`, `confirm`; Bridge notification/overlay messages | Host bounds/redacts data and owns UI; confirmation denies without an adapter | Host request contracts and fixture evidence |
| Render custom UI | Contribution `ui`; `surface.*` / `host.*` messages | Host owns origin policy, focus, layout and disposal | [Isolated UI](../guides/isolated-ui); Alpha adapters |

Manifest declarations request capabilities; they do not grant them. Find the exact permission and failure behavior in the [API index](/reference/plugin-api-index) and [security guide](./security). Runtime enum membership does not establish a security sandbox.

For schemas and callable host messages, use the API reference. For `HostFacade` methods above, use the explicitly labeled [fixture catalog](/reference/plugin-developer-guide#fixture-hostfacade-api-catalog). Production SDK publication remains an [ecosystem gate](/reference/plugin-ecosystem-qualification).
