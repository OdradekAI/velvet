# ADR-0006: Host protocol and framework-independent UI SDK

Status: Proposed
Date: 2026-10-28

## Context

The built-in workbench may use Vue or React, but a third-party extension ecosystem must not depend on the current frontend framework. The Rust core also needs a small, testable interface instead of exposing internal services to every view.

## Proposed decision

Define a versioned `workbench-protocol` containing four categories:

```text
Commands  state changes and user actions
Queries   reads and projections
Events    state changes, progress and lifecycle notifications
Handles   opaque references to assets, views, jobs and plugin sessions
```

The protocol is transport-independent. Tauri IPC is the first transport adapter.

Protocol version `1` uses request and response envelopes with a request id.
Layout mutations may include an expected revision; Rust rejects stale writes
with a typed error and returns the authoritative revisioned snapshot. The
protocol defines layout and plugin lifecycle event shapes for the future Tauri
subscription, while the desktop TypeScript contract remains an explicit
adapter mapping until serialization and compatibility fixtures are finalized.

The UI SDK exposes host concepts rather than Vue or React component types:

```text
ViewDescriptor
ViewContainerDescriptor
CommandDescriptor
PreviewContext
InspectorSection
JobView
Notification
```

Trusted built-in extensions may use a native Vue/React adapter. External extensions use an isolated UI surface with host-provided theme, lifecycle, accessibility and message channels.

The host owns registries and lifecycle:

```text
LayoutService    sole owner of layout state
ViewRegistry     sole owner of view descriptors
PreviewResolver  sole owner of primary preview selection
PluginHost       sole owner of extension lifecycle
JobSystem        sole owner of asynchronous job state
```

The first desktop host slice also owns `CommandRegistry`, `MenuRegistry`,
`ContextKeyService` and `OverlayRegistry`. Commands and menus are namespace-
aware and resolve app-scoped entries only for the active WorkbenchApp. Context
predicates use a small declarative grammar; overlays carry host-managed modal,
escape and focus-restoration state while their body is rendered by a trusted
Velvet UI adapter or isolated UI bridge.

Layout actions use stable command identifiers (`workbench.toggle-part`,
`workbench.focus-part`, `workbench.resize-part`, `workbench.collapse-part`,
`workbench.maximize-part`, `workbench.move-view`,
`workbench.upsert-view-container`,
`workbench.set-view-container-visibility`, `workbench.reorder-view-container`,
`workbench.remove-view-container`, `workbench.reset-layout`,
`workbench.reset-app-layout` and `workbench.reset-view-layout`). Their typed
payloads are adapted to the Rust `LayoutService` rather than persisted by the
renderer.

No view calls another view's implementation directly. Cross-feature actions go through commands and typed context.

## Alternatives considered

1. Export internal React or Vue components to extensions. Rejected because a frontend migration would become an ecosystem migration.
2. Let every view invoke arbitrary Tauri commands. Rejected because permissions and state ownership would be invisible at call sites.
3. Use one untyped event bus for all communication. Rejected because payload compatibility and error handling would be implicit.

## Consequences

Positive:

- Protocol tests can run without a browser.
- Built-in and external extensions share the same conceptual model.
- UI framework changes remain local to the adapter layer.

Costs:

- The protocol needs explicit error, cancellation and version semantics.
- Isolated UI surfaces need a small host SDK for useful integration.

## Open questions

- JSON, MessagePack or another serialization format after performance measurement?
- Which protocol types are public in the first SDK release?
- How are incompatible events downgraded or ignored?
