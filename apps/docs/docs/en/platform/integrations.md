---
id: integrations
title: UI integration choices
---

# UI integration choices

<span id="integrations" className="velvet-doc-anchor" />

Use host Commands, menus, notifications and progress when they meet the task. A custom surface is optional.

Trusted first-party UI uses the [host React wrappers](../ui-kit). Third-party surfaces render their own content and consume the framework-neutral [Bridge](./ui-bridge). React, Vue, Svelte and plain HTML are possible rendering choices, not separately shipped Velvet SDKs.

The repository has iframe and Tauri child-WebView adapters. Their Alpha and native qualification limits are described in the [isolated UI guide](../guides/isolated-ui). Host Tailwind classes, Radix types, private DOM and internal React components are outside the third-party contract.
