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

Product extensions are declared by a versioned manifest and activated lazily. The default runtime for third-party code is an isolated WASM module or sidecar process. Built-in extensions may use an in-process adapter, but they must use the same public contribution interfaces.

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

In version one, extensions cannot create arbitrary ActivityBar workspaces. They contribute to the fixed AssetManager and FeedWorkbench zones. A future top-level workspace contribution requires a separate manifest capability and review policy.

Every extension identifier, contribution identifier and persisted facet schema is globally unique and versioned.

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
