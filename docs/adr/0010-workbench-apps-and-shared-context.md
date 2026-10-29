# ADR-0010: Workbench apps and shared context

Status: Accepted
Date: 2026-10-29

## Context

Velvet's central `WorkspaceArea` is not required to behave like a VS Code
editor group. The product needs business-oriented sub-applications such as a
resource manager, feed workbench, research surface or paid extension. A user
should be able to choose one from the Activity Bar, while the selected
sub-application can own its central surface and declare supporting sidebars.

These sub-applications must share the same Asset, Collection, Source, facet
and job data. If each app creates its own record model or persistence owner,
selection, permissions, indexing and extension data will diverge.

The existing term `Workspace` is therefore overloaded: it describes a user's
working context in the domain, while the central surface is a selectable
product experience.

## Decision

Introduce `WorkbenchApp` as the top-level selectable experience inside a
Workbench:

```text
Workbench
├── ActivityBar
│   └── WorkbenchApp entries
├── WorkspaceArea
│   └── active WorkbenchApp AppSurface
├── PrimarySidebar / AuxiliarySidebar / OperationsPanel
│   └── ViewContainers and Views declared by the active app or host
```

A `WorkbenchApp` may be built in or contributed by a product extension. It may
declare:

- an Activity Bar entry with stable identity, title, icon and ordering;
- one main `AppSurface` for the `WorkspaceArea`;
- compatible ViewContainers and Views in the declared semantic Zones;
- commands, menus, preview intents, jobs and capability requirements.

An extension may also contribute only an Activity Bar `ViewContainer` entry
without providing a central `AppSurface`. This supports focused tools that
need a dedicated sidebar but do not need to become a full sub-application.
Conversely, a full `WorkbenchApp` may declare both its central surface and its
supporting ViewContainers.

The first host API treats built-in and first-party applications as trusted
adapters. Third-party applications use an isolated UI bridge with a declared
bridge version. Both paths receive the same framework-neutral host contracts;
neither path receives host React component types or direct DOM access.

An installed WorkbenchApp establishes an app namespace. Smaller app add-ons
may register dialogs, context-menu actions, toolbar actions or inspector
sections inside that namespace without becoming another WorkbenchApp. They do
not register into unrelated apps or the global shell unless they declare a
separate approved Workbench contribution.

The technical id `asset-manager` remains stable for the current protocol and
migrations; its product-facing title is `Resource Manager`.

The canonical transport now calls the active-app field `activeApp` and carries
it in the revisioned layout snapshot. Desktop migration code accepts the older
`activeWorkspace` spelling so persisted state and older clients can be upgraded
without changing the stable `asset-manager` identifier.

A `WorkbenchApp` may not own Workbench Parts, global overlays, layout
persistence, Asset identity, Source identity or the shared data stores. The
host owns activation, focus, layout, lifecycle, permissions and failure
isolation.

`WorkspaceArea` hosts one active `WorkbenchApp` surface at a time. The host
does not assume editor tabs as the central navigation model. An app may later
define its own internal tabs, panes or navigation, but those remain inside the
app surface and use app-owned state behind a host-controlled lifecycle.

Expose a read-only `WorkspaceContext` projection and typed commands/events for
shared context such as the active data scope, selected Asset identifiers,
current Collection, query, filters and handoff intents. The first projection
contains stable typed references, not application-specific record schemas. An
app may request a handoff to another app, but app activation remains a
host-owned command. Apps do not call one another's implementation or access
persistence directly; cross-app workflows go through core queries, commands
and events.

The desktop host derives a contributed app's stable id from an extension-owned
prefix (for example, `sample.extension.research`) and rejects ids owned by
another extension. `PluginClient` registers the descriptor and its isolated
AppSurface bridge in `WorkbenchAppRegistry` as one activation unit. The
registry projects the app into the Activity Bar, lazily creates a bridge
session when the app becomes active, suspends it when another app is selected,
and disposes it before extension deactivation removes the Activity Bar entry.
`WorkbenchAppSurfaceHost` renders the active session in the WorkspaceArea and
forwards measured geometry and context updates without taking over lifecycle
ownership. Late adapter binding retries mount; a disposed session can be
created again from the retained descriptor and adapter, which is the restart
recovery path for an isolated app surface.

Keep these ownership layers separate:

```text
Core data       Asset, Source, content identity, Collection, facets and Jobs
Workbench       Parts, layout, activation, selection context and commands
WorkbenchApp    central surface, app preferences and saved app views
Extension       namespaced facets/models, declared views and capability use
```

Borrow Spacedrive's separation of a library control plane, source records,
stable content identity and extension-owned durable data, but do not copy its
filesystem or distributed-storage schema into Velvet. The data model remains
the subject of a later decision ticket.

## Alternatives considered

1. Treat every Activity Bar entry as a VS Code-style editor container. Rejected
   because Velvet's central experiences are business applications with their
   own navigation and composition model.
2. Let every sub-application own a private Asset database. Rejected because
   shared selection, search, permissions, deduplication and extension facets
   would diverge.
3. Let extensions replace the whole Workbench shell. Rejected because Parts,
   layout, overlays, focus and recovery need one host owner.

## Consequences

Positive:

- Resource Manager, Feed Workbench and paid extensions can share one data model.
- Activity Bar contributions are first-class without turning plugins into
  unrestricted layout owners.
- The central surface can evolve beyond editor tabs without weakening the
  framework-independent Workbench seam.
- Spacedrive's useful identity and durability principles can inform the core
  without coupling Velvet to a VDFS implementation.

Costs:

- Velvet needs an explicit `WorkbenchAppRegistry` and app activation protocol.
- Shared context needs versioned projections and event semantics.
- App state, layout state and durable domain data need separate persistence
  owners and migration rules.

## Follow-up decisions

- App-local selection, query and View state is restored from Workspace scope;
  profile scope stores only global shell and companion layout, while Session
  scope stores window restoration hints.
- `WorkbenchAppRegistry` registers descriptors before activation and binds a
  surface factory lazily. A missing or failed factory leaves the Workbench
  usable and does not invalidate layout or core data.
- A plugin WorkbenchApp uses an extension-prefixed id and an isolated
  `AppSurface` descriptor. Registry activation and deactivation are tested as
  one lifecycle: Activity Bar projection appears only while active, the
  bridge session is mounted on selection, and disposal removes the app,
  surface descriptor and namespace projections together.
- The later Asset/Source/Collection model will define the concrete data-scope
  handles and materialization semantics; the first context API remains
  reference-based so that decision does not leak into the shell.
