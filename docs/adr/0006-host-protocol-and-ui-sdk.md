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
subscription. Plugin manifests cross this boundary as explicit
`PluginManifestDto`, `PluginProjectionDto` and `PluginInstallPreviewDto`
mirrors of the Rust `plugin-api` types; the desktop TypeScript contract and
Tauri wire types map those DTOs explicitly rather than importing the domain
Manifest as a transport owner.

The lifecycle slice also exposes a versioned `RecoveryProjection` through a
`query-recovery` query and `recovery-changed` event. The desktop
`RecoveryClient` owns that projection and subscribes through `CoreTransport`;
it never reads the lifecycle journal or persistence files. Recovery actions
remain host policy and are not exposed as arbitrary renderer commands.

The canonical envelope also carries a negotiated feature set, `sessionId`,
message kind (`command`, `query`, `response`, `event`, `handle`, `error` or
`cancel`), method, typed payload, revision/sequence and deadline. Request ids
are idempotent across retry; duplicate events are ignored, sequence gaps cause
bounded snapshot resync, and reconnect reports the last acknowledged sequence
and scope revisions. The transport retains no unbounded event log. A missing,
corrupt or incompatible state document is handled by its Rust owner with a
versioned migration, safe default or typed recovery error; the renderer never
repairs persistence directly.

Protocol compatibility has three independent dimensions. A major protocol
version defines message semantics and must match; a minor version may add
optional fields or features without changing existing required meaning. Each
durable or projected payload carries its own schema version, and feature ids
are negotiated separately. The alpha `protocolVersion: 1` is major 1/minor 0
until an explicit major/minor wire shape is introduced.

Each new transport begins with a typed handshake that advertises supported
protocol versions, payload schemas, client build identity and features. The
core returns the selected version, accepted features, a fresh `sessionId`,
authoritative scope revisions and the current event sequence. Mutating
messages are rejected until the handshake succeeds. Unknown optional fields
may be ignored; unknown required fields, unsupported kinds and incompatible
schemas return typed errors such as `protocol-incompatible`,
`schema-incompatible`, `feature-unavailable`, `session-expired`,
`sequence-gap`, `stale-revision`, `request-unknown-outcome` and
`transport-unavailable`.

`requestId` is the logical idempotency key retained across a retry only for a
command whose contract declares idempotency. The core keeps a bounded
recent-result window scoped to the command owner and retention policy. If a
mutation's outcome is unknown after a disconnect, the client first queries
authoritative state and retries with the same key only when the command
contract permits it; a request id does not make an external side effect
idempotent, and such effects are never replayed blindly.

The desktop allocates request ids monotonically for each client. When the
bounded window evicts an older command outcome, the transport retains a
per-client consumed floor and rejects command ids at or below that floor;
queries remain repeatable and may still report the evicted outcome as unknown.
This keeps bounded memory from turning an old request id into a new mutation
key.

Reconnect creates a fresh session, retains the last valid read-only projection
while degraded, performs bounded backoff and then requests a complete
authoritative snapshot resynchronization. Events and handles from the old
session are ignored. Plugin and UI Bridge sessions are recreated from
Rust-owned projections, and the transport never requires an unbounded event
replay log.

The desktop mapping is exposed by `TauriCoreTransport`. It sends a
`velvet_core_request` envelope with a monotonically allocated request id and
subscribes to `velvet_core_event` only while there are active listeners. The
adapter rejects protocol-version or request-id mismatches and normalizes native
invoke failures into `CoreTransportError`.

The Tauri application boundary implements the matching `velvet_core_request`
command and explicit JSON wire mapping. Successful layout mutations emit
`velvet_core_event` snapshots through the Rust adapter's event sink. The
native global shortcut plugin feeds `velvet_keybinding` events into the same
renderer `KeybindingRegistry`; it does not own command resolution or layout
state.

The current protocol also carries Rust-owned Preview source registration and
removal commands plus an AssetId-only Preview query. The source path is an
input to the application boundary and is validated and persisted by Rust; it
is never included in a Preview projection. TypeScript and Tauri keep explicit
mirrors for these requests, while source failures use a typed, path-free error
that remains local to the affected asset.

Plugin lifecycle uses the same versioned request envelope. The protocol exposes
manifest registration plus activate, disable, health-check and recovery
commands, each returning the authoritative Rust `ExtensionSummary`. The
desktop `PluginClient` keeps this runtime projection separate from its local
contribution registries; UI registration and isolated surface mounting remain
explicit renderer actions. A plugin inventory query is best-effort during
native startup so a runtime or transport failure cannot block the Workbench
shell.

The plugin runtime envelope is shared by built-in, WASM and sidecar adapters.
It carries `protocolVersion`, `sessionId`, `requestId`, a typed message kind
(`request`, `response`, `event`, `cancel` or `health`), method, typed payload,
deadline and sequence. Requests use idempotent request ids; deadlines produce
bounded cancellation and non-cooperative runtimes are terminated after a
grace period. Reconnect creates a fresh session and restores the authoritative
capability/context snapshot without replaying side-effecting commands. Typed
runtime errors, malformed DTOs, crash state and recovery remain scoped to the
extension that owns the session.

Runtime events also carry the extension package version, trust/runtime kind,
exit code or WASM trap, budget decision, and (when applicable) Job id/attempt.
The host-owned diagnostic sink accepts bounded structured logs and redaction
hints; it is the only path into a `DiagnosticBundle`. Extension storage,
backup/migration participants and a future read-only entitlement handle use
the same versioned envelope, but none can publish a core recovery pointer or
invoke a generic Tauri command.

Active extension projections can also be queried with their declared
ViewContainer/View metadata and isolated UI Bridge descriptors. The desktop
adapter validates the returned namespace and reuses `ViewRegistry` and
`PluginUiRegistry` for registration and cleanup. The projection is filtered by
the Rust lifecycle state, so a disabled or failed extension cannot leave active
views or surface sessions behind; recovery reuses the same projection path.
The same projection carries command, context-menu and dialog metadata. The
desktop adapter registers those entries through `CommandRegistry`,
`MenuRegistry` and `OverlayRegistry`. A projected command is sent back through
the versioned CoreTransport as an `invoke-plugin-command` request; Rust checks
that the command belongs to the active extension and forwards it through the
runtime's versioned JSONL channel. Local trusted handlers remain an adapter for
built-in integrations, while external execution never bypasses the host.
If invocation fails because the isolated runtime closes, crashes or times out,
the Tauri adapter emits a `plugin-disabled` crash event so the renderer removes
the failed projection and can recover it through the normal lifecycle command.

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

### Optional text-language capability

The reusable Workbench foundation is editorless. It does not require an
editor document model, a language provider registry or an LSP adapter. A
future text-oriented `WorkbenchApp` may opt into a separate text-language
capability pack; the first known need is limited to syntax-highlighting
resources.

If that pack is introduced, core languages reserve stable short `LanguageId`
values such as `json` and `markdown`. A plugin-defined language receives an
extension-qualified identity (`extensionId/localId`); a plugin that augments an
existing language targets that identity explicitly. Provider, grammar, snippet
and contribution ids are owned by the contributing extension, while
human-readable aliases are localized labels. Language identity, file
associations and the document's content survive extension disable or removal;
only the extension's optional highlighting resources are removed. The resource
format, selector grammar and JSON/Markdown fixture are deferred. Diagnostics,
completion, hover, definition, formatting, provider matching, document
versions, snippets, schema validation, semantic tokens and any LSP adapter are
also deferred and are not Workbench dependencies.

### Plugin localization

Extension localization is a P0 plugin capability, separate from Workbench
display-language packs and from the optional text-language capability. A
manifest that contributes localized text may declare a strict, versioned
`localization` object with `defaultLocale`, BCP-47 `supportedLocales`, a
package-relative `resourceRoot` (default `l10n` when opted in) and a
`bundleSchema`. Bundles live at `<resourceRoot>/<locale>.json`; the default
bundle is required, other locales are optional, and traversal outside the
package is rejected. Unknown fields are rejected except reserved `x-*`
metadata. The host validates locale tags, duplicate ids, message syntax and
argument shapes before activation. Workbench display-language packs are a
separate extension category and do not change this contract.

Message ids are stable and namespaced as `<extensionId>.<localMessageId>`.
Manifest-owned labels (commands, Views, menus, settings, Activity Bar,
Preview, Job and Overlay labels) carry a message id and a human-readable
`defaultMessage`; the host resolves them before projection. Runtime strings use
the same conceptual API:

```ts
interface L10n {
  t<T extends MessageArgs>(
    messageId: string,
    defaultMessage: string,
    args?: T,
  ): LocalizedString;
}
```

The host verifies namespace ownership and argument compatibility. Trusted
surfaces receive a scoped host `L10n` handle. Isolated surfaces receive the
same semantic service through the UI Bridge as a locale/bundle snapshot or
message-resolution channel carrying the bundle schema, session id and locale
revision. Neither path exposes another extension's bundle, arbitrary paths,
HTML, components or framework objects.

The effective locale is owned by Profile/Session state. Selection and change
are host commands; active surfaces observe a scoped locale revision. Resolution
uses the deterministic chain `requested exact locale → requested language
subtag → extension defaultLocale → declared defaultMessage`. A missing or
invalid translation records an extension/locale/message diagnostic and keeps
the contribution visible with fallback text.

The initial message format supports named arguments, validated number/date
formatting and `plural`/`select` branches using a constrained ICU-like syntax.
Bundle entries and `defaultMessage` must agree on argument names and branch
types; invalid entries fall back per message and rendered values are text-only.
Bundles load lazily and are cached by extension id, package version, schema
and locale. A locale change re-resolves projections and sends bounded updates
to active Views, AppSurfaces, Previews, Overlays and Job UI. Duplicate or late
updates from an old session/revision are discarded. Bad JSON, schema mismatch,
invalid locale, path escape, duplicate id or bridge delivery failure isolates
the affected bundle/message; a valid extension and the Workbench remain usable.
Disable, upgrade and uninstall remove that extension's bundle cache and
projections without touching core layout, Asset identity or other extensions.

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

The host also owns `KeybindingRegistry` and `FocusService`. Keybindings use
normalized multi-chord sequences, allow context-qualified alternatives, prefer
the active WorkbenchApp namespace and execute through `CommandRegistry`.
Focus state is a framework-neutral projection for Parts, Views and overlays;
the renderer adapts it to DOM focus while the host retains restoration order.

The public command seam uses typed request/response DTOs with a request id,
typed arguments and results, a context snapshot, cancellation and typed
diagnostics. Command errors distinguish missing/inactive, disabled,
permission-denied, cancelled, timeout and handler failure. `MenuRegistry` may
reference only registered commands. Context expressions carry a grammar
version; the first grammar supports boolean operators, parentheses, equality,
inequality and finite-set membership, and rejects unknown syntax closed with a
diagnostic. Command Palette and Quick Input remain host-owned surfaces.

The host Overlay contract covers dialogs, popovers, context menus, command
search, quick input, toast, alert and NotificationCenter. The host owns the
overlay stack, modality, placement, escape/outside-click, timeout, focus trap,
focus restoration, live-region announcement and cleanup. A trusted or
isolated body is only a renderer adapter. View providers use the same host
lifecycle for loading, empty, welcome, badge, toolbar, failure, retry and
state-restore projections; their namespaced state is separate from Rust layout
persistence. View/Container movement and drag-and-drop remain typed
`LayoutService` commands.

Layout actions use stable command identifiers (`workbench.toggle-part`,
`workbench.focus-part`, `workbench.resize-part`, `workbench.collapse-part`,
`workbench.maximize-part`, `workbench.move-view`,
`workbench.upsert-view-container`,
`workbench.upsert-view`, `workbench.set-view-container-visibility`,
`workbench.reorder-view-container`,
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

## Proposed hardening clarification (2026-11-02)

This ADR remains `Proposed`; the clarification is not an implicit promotion to
`Accepted`. The host envelope is typed, while plugin command/action arguments,
Job input and reserved extension metadata remain schema-owned JSON payloads.
Each such payload must carry a bounded owner schema id/version and be validated
by `PluginHost` or `JobSystem` before activation or execution. The transport
must not present these fields as arbitrary native capability entry points.

`clientId` is a stable local correlation identity and `sessionId` is a fresh
session authority for the current Core process. Reconnect retires the old
session, rejects old handles and mutations, and resynchronizes an authoritative
snapshot. These values are not authentication or remote write grants. A future
CLI, daemon or remote adapter requires a separate authenticated handshake with
profile scope, capability grants, authority epoch and key rotation; v1 does not
share projections across remote clients.

The transport may carry narrow versioned environment facts needed by an owner,
including storage availability and power pressure as optional extensions to the
Job environment schema. Wall-clock trust and proxy credentials do not belong in
that payload: Job deadlines and migration ordering use monotonic time, while
endpoint/proxy failures remain owner-specific typed retryable errors.

The acceptance gate is a Rust/Tauri/TypeScript fixture for schema ids and
bounded payloads, stale sessions, sequence gaps, unknown outcomes and the
negative case where an untyped path/URL/process command is rejected. Until
those fixtures and non-Tauri adoption exist, the proposed protocol remains a
host contract rather than a public SDK compatibility promise.
