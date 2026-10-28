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
