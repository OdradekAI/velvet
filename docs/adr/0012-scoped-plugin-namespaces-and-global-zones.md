# ADR-0012: Scoped plugin namespaces and reserved global zones

Status: Accepted
Date: 2026-10-29

## Context

Velvet needs two different plugin relationships. A package may install a new
`WorkbenchApp` with its own central surface, or an add-on may extend an
existing `WorkbenchApp` with a context menu, dialog, toolbar action or
inspector. The latter must not become a second app and must not leak its
commands into unrelated apps.

The Workbench also has two areas whose meaning should remain global:

- the right `AuxiliarySidebar` is reserved for persistent companion views,
  such as an AI assistant, a restricted browser or a database tool;
- the bottom `OperationsPanel` is reserved for jobs, progress, logs, history
  and other global operations.

Without explicit namespaces and reserved-zone rules, an app add-on could
accidentally register a command globally, a feed plugin could occupy the
database companion slot, or each app could create a competing task system.

## Decision

Every installed package and every contribution has an explicit namespace:

```text
workbench                         global shell and cross-app namespace
workbench.app.<app-id>            one WorkbenchApp and its add-ons
capability.<extension-id>         reusable data and processing capability
```

An installed `WorkbenchApp` creates its own app namespace. An app add-on must
declare a target `app-id` and compatible app API range. It may register only
app-scoped commands, menus, dialogs, toolbar actions, inspector sections and
other small host surfaces permitted by that app. Its contributions are
resolved only while that app namespace is active and are not visible in other
app namespaces.

An app add-on cannot register another `WorkbenchApp`, replace the central
`AppSurface`, claim a global Part, or write another app's state. Cross-app
behavior uses explicit Workbench commands/events and capability grants.

The Workbench namespace owns the global containers:

```text
ActivityBar       WorkbenchApp entries and global tool entries
TitleBar          global actions and window-scoped tools
OverlayLayer      dialogs, popovers, notifications and command search
AuxiliarySidebar  persistent CompanionView contributions
OperationsPanel   global jobs, progress, logs and operation history
```

`CompanionView` is a global, host-managed view that follows the shared
`WorkspaceContext` but is independent of the active `WorkbenchApp`. A companion
may be toggled, resized and ordered like other host contributions, but it
cannot create a second global layout owner.

Examples:

- AI Assistant reads a permitted shared context and dispatches registered
  commands or jobs.
- Restricted Browser uses an isolated web surface and an explicit navigation,
  download and import capability; it has no ambient filesystem access.
- Database Companion receives a typed query capability and renders results; it
  does not open the application's SQLite files directly.

The `OperationsPanel` is global by default. App-specific progress can appear
inside an app surface or an app-scoped ViewContainer, while durable Job state,
cancellation, retry and logs remain owned by the shared `JobSystem`.

## Contribution resolution

The host resolves a contribution by:

1. package identity and manifest compatibility;
2. namespace and target app activation;
3. host container and contribution kind;
4. context anchor and enablement predicate;
5. capability grants and lifecycle state.

Registration in an app namespace does not grant the app add-on the permissions
of the app. Network, process, filesystem, metadata-write and model access are
separate capabilities.

The host registers an extension manifest before activation, validates its host
API and target WorkbenchApp API, then lazily registers its contributions when
the extension activates. App-scoped contributions are visible only while their
target app is active. Deactivation, crash isolation, incompatibility and
uninstall remove every contribution registered by that extension while leaving
layout, core data and other extensions usable.

The desktop and Rust skeletons derive namespaces from manifest identity:
`workbench`, `workbench.app.<app-id>` and `capability.<extension-id>`. A
manifest cannot claim another namespace, target more than one WorkbenchApp or
enter a reserved global container through an app scope. CompanionView and
OperationView remain global Workbench contributions only.

## Alternatives considered

1. Treat every app add-on as a full WorkbenchApp. Rejected because small
   interactions would fragment ActivityBar and central-surface ownership.
2. Make right and bottom panels available to every app. Rejected because
   persistent companions and global operations would lose stable meaning.
3. Use one global plugin namespace with naming conventions. Rejected because
   conventions do not enforce visibility, activation or permission isolation.

## Consequences

Positive:

- WorkbenchApp ecosystems can grow without leaking commands and menus across
  applications.
- The right and bottom regions retain stable product meaning.
- AI, browser and database companions can follow shared context without
  becoming part of every app's implementation.
- Job ownership stays centralized even when apps contribute job views.

Costs:

- The manifest and registries need namespace, target and compatibility fields.
- The host needs activation-aware lookup and cleanup for app add-ons.
- Companion surfaces need explicit isolation and capability policies.

The first lifecycle implementation is intentionally in-memory. Persistence,
lazy runtime startup and WASM/sidecar process supervision remain owned by the
plugin host milestone that introduces the external runtime.
