# ADR-0005: Plugin host runtime, permissions and lifecycle

Status: Proposed
Date: 2026-10-28

## Context

The product needs an ecosystem for format readers, previewers, inspector panels, feed connectors, AI actions and operational tools. Tauri plugins that expose host capabilities and user-installed product extensions have different trust and lifecycle requirements.

## Proposed decision

Keep two extension layers separate:

```text
Tauri host plugins      system and desktop capabilities
Product extensions      workbench, preview and asset capabilities
```

Product extensions are declared by a versioned manifest and activated lazily. A
manifest may contain Workbench-level, WorkbenchApp-level and reusable
capability contributions, each with its own target container and permission
set. The default runtime for third-party code is an isolated WASM module or
sidecar process. Built-in extensions may use an in-process adapter, but they
must use the same public contribution interfaces.

Permissions are capability-based and zone-aware:

```text
navigation:
  workspace.open, library.read, source.connect

contextual:
  asset.read, metadata.read, preview.render, metadata.write

operations:
  jobs.create, jobs.read, logs.read, external-process, network
```

An extension receives capability handles rather than unrestricted file paths. Installation, activation, update, disable, uninstall and rollback are owned by `PluginHost`. A crashed or incompatible extension must not take down the workbench.

In version one, extensions cannot own arbitrary Workbench Parts or bypass the
fixed semantic Zones. After the `WorkbenchApp` contribution contract is
accepted, an extension may contribute a top-level `WorkbenchApp`, an
ActivityBar ViewContainer-only tool, or a reusable capability. These entries
still use host-owned layout, permissions and lifecycle rules; they do not create
an independent shell or data store.

The right `AuxiliarySidebar` is reserved for host-approved global
`CompanionView` contributions, and the bottom `OperationsPanel` is reserved for the shared JobSystem
and operation history. An app add-on does not gain access to either surface
merely because it runs inside that app.

Every extension identifier, contribution identifier and persisted facet schema is globally unique and versioned.

The first runtime integration keeps the process boundary behind two host-owned
interfaces: `PluginRuntime` starts an extension with an explicit
`RuntimeLaunchSpec` and `RuntimeProcess` reports health, shutdown and command
invocation. The production adapter uses a child process with a versioned JSONL
stdin/stdout channel and passes only the extension id and the validated
permission grant as environment metadata. Built-in extensions use the same
lifecycle through a no-op process; WASM extensions provide their declared WASM
runner as the launch program. Invocation requests carry the extension id,
command id, opaque arguments and a context snapshot; responses carry the
runtime protocol version, success flag, result and optional error.

The host applies a bounded invocation timeout. A plugin-level rejection is
returned to the caller while the extension stays active. A closed or crashed
runtime, malformed response or timeout tears down only that process, marks the
extension failed and leaves it recoverable through the normal lifecycle path.

Permission sets are checked against the declared semantic zone before a
manifest is registered. Startup failure marks only that extension as failed;
health polling removes a crashed extension from active contributions, and an
explicit recovery call may create a fresh runtime. A crash or failed restart
does not disable the Workbench or unrelated extensions.

The native composition root scans the application data directory's `plugins`
folder for JSON manifests at startup. Discovery is deterministic by filename,
and each parse or registration failure is returned in a report while valid
manifests remain registered for activation. Package updates, signing and
marketplace policy remain outside this local discovery seam. Relative launch
arguments beginning with `./` or `../` resolve from the
manifest directory, which lets a sidecar package move as one directory without
embedding an absolute executable path.

The local install flow is a two-phase transaction. `inspect-plugin` parses the
manifest and returns the requested permissions, target app, runtime and host API
compatibility before any files are copied. `install-plugin` stages the package
under this directory; an existing extension is accepted only when its numeric
version increases. The host keeps the previous package as a hidden rollback
backup until the client has projected and activated the new extension, then
`commit-plugin-install` removes the backup. `rollback-plugin-install` removes a
new package or restores the previous package and runtime state. The native
Tauri composition root activates committed registered extensions during client
startup. It scans for abandoned `.staging-*` and `.rollback-*` directories left
by a client crash and holds affected extensions inactive until the host presents
resume, commit, rollback or discard actions. Other embedders may retain lazy
activation by calling the lifecycle commands explicitly.
Recovery failures are logged with an attempt number and remain retryable. Startup
removes only stale or invalid transaction directories that are not part of a
recoverable pair, records those cleanups, and exposes a bounded recovery log to
the native client.

## Alternatives considered

1. Load arbitrary third-party Rust dynamic libraries into the Tauri process. Rejected because ABI, memory safety and crash isolation cannot be controlled by the host.
2. Treat all extensions as frontend bundles. Rejected because decoders, indexing, filesystem access and long jobs need a controlled Rust-side capability model.
3. Give every plugin the same unrestricted permission set. Rejected because a metadata panel should not gain network or process permissions by default.

## Consequences

Positive:

- Plugin installation can be explained by zone and capability.
- Heavy decoders and untrusted code are isolated from the desktop host.
- The host can enforce compatibility and recover from extension failure.

Costs:

- Sidecar and WASM boundaries add serialization and packaging work.
- Permission prompts and compatibility checks become part of the product UX.

## Open questions

- Which extension packages may be signed, and who is trusted to sign them?
- Is there a marketplace in the first release or only local installation?
- What is the minimum supported host API version and deprecation window?
