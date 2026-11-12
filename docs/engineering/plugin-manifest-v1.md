---
id: plugin-manifest-v1
title: Plugin manifest and transport contract (schema 1)
---

# Plugin manifest and transport contract (schema 1)

<span id="velvet-plugin-manifest-and-coretransport-contract-schema-1" className="velvet-doc-anchor" />

Status: **Alpha contract maintained in-repository**, not a published external SDK.

<span id="ownership-and-boundaries" className="velvet-doc-anchor" />

Owner: `crates/plugin-api/src/lib.rs` (`ExtensionManifest` / `RawExtensionManifest`). Consumers: PluginHost admission and transport adapters. The host alone owns permission grants, activation and package state.

## Manifest schema 1

JSON field names are camelCase. `required` means required by deserialization; `optional` means the field may be absent or null. Runtime and contribution validation can impose additional requirements. Required arrays must be present, even when empty.

{/* manifest-fields:start */}
| Field | Presence | Meaning |
| --- | --- | --- |
| `manifestSchema` | `required` | Numeric schema, currently `1`. |
| `id` | `required` | Stable extension identity. |
| `version` | `required` | Numeric package version; preserve identity across upgrades. |
| `hostApi` | `required` | Host API range. |
| `bridgeApi` | `required` | UI Bridge API range. |
| `runtimeProtocol` | `required` | External runtime protocol range. |
| `runtime` | `required` | `built-in`, `wasm` or `sidecar`; declaration does not qualify a runtime. |
| `extensionKind` | `required` | Array of `ui`, `workspace`, `capability`. |
| `launch` | `optional` | Launch descriptor; runtime admission may require it. |
| `activationEvents` | `required` | Activation selectors; local command/View references must exist. |
| `extensionDependencies` | `required` | Array of stable extension IDs. |
| `targetApp` | `optional` | WorkbenchApp target and API range. |
| `support` | `required` | Trust, virtual-workspace and Profile support declarations. |
| `requiredCapabilities` | `required` | Required capability names; grants remain host policy. |
| `optionalCapabilities` | `required` | Optional capability names; an array may be empty. |
| `localization` | `optional` | Package-relative localization declaration. |
| `storage` | `optional` | Extension storage schema and quota. |
| `workbenchApp` | `optional` | Top-level product app descriptor. |
| `contributions` | `required` | Contribution descriptors with unique local IDs. |
| `permissionSets` | `required` | Zone-scoped permission sets; `permissions` is nested here. |

{/* manifest-fields:end */}

`ui` is a field on a contribution descriptor, not a top-level manifest field. `permissionSets[].permissions` is distinct from capability-name arrays. Copy a complete [Echo manifest](https://github.com/OdradekAI/velvet/blob/main/examples/plugins/echo-sidecar/manifest.json) rather than treating this table as a complete package.

Unknown fields are rejected except top-level `x-*` metadata. The host does not interpret that metadata as capabilities. `launch` can be optional in JSON while still required by runtime admission. `built-in` is a trusted composition path, not a third-party escape hatch.

## Identifier and upgrade rules

- Extension and contribution IDs match `[A-Za-z0-9][A-Za-z0-9._-]*` and are at most 128 bytes.
- Contribution IDs are unique within the extension. Namespace authority is host-derived; a package cannot claim another owner's namespace.
- Upgrades preserve the extension ID and increase the numeric version. A new ID is a different installation.
- Duplicate dependencies, missing activation references, malformed ranges, unknown capabilities and duplicate capability declarations are rejected.
- Alpha API ranges accept `*`, an exact numeric version or a caret major such as `^1`.
- Scope, zone, target app and effective grants remain PluginHost policy.

## Plugin CoreTransport envelope

The host-facing `PluginRequestEnvelope` contains `protocolVersion`, `sessionId`, `callerExtensionId`, `runtimeNonce`, `requestId` and `message`. The runtime adapter binds caller identity and a host-issued lease nonce. This is distinct from the sidecar JSONL payload used by the quick-start examples.

```json
{
  "protocolVersion": 1,
  "sessionId": "opaque-host-session",
  "callerExtensionId": "publisher.reader",
  "runtimeNonce": "host-issued-runtime-lease-nonce",
  "requestId": 7,
  "message": {
    "kind": "query-projection",
    "extensionId": "publisher.reader"
  }
}
```

These placeholder values cannot authorize a request. The adapter validates the active lease and session; the target extension must equal the bound caller. `requestId` must be nonzero. See the [API index](plugin-api-index.md) for all current host request kinds and their response variants.

Invocation is not replayable by request ID. Projection queries are repeatable. A transport failure can leave a mutation outcome unknown; reconcile authoritative state before retry. No envelope grants access to a database, raw path, Tauri state object or arbitrary process.

## Compatibility policy

`manifestSchema`, `hostApi`, `bridgeApi` and `runtimeProtocol` are negotiated independently. The current host accepts schema/API major `1`; unsupported ranges fail closed. Optional additive changes can retain a major version. Required fields, message meaning and ownership changes require a compatibility decision and fixtures.

A reconnect creates a new session. Old events and side effects are not replayed. ADR status describes the design decision; implementation and platform qualification are tracked separately in the [contract matrix](plugin-contract-matrix.md).

## Contract fixtures

- Manifest parsing and validation: `crates/plugin-api/src/lib.rs`.
- Envelopes and request conversion: `crates/workbench-protocol/src/protocol_tests.rs`.
- Lease/session validation: `crates/tauri-adapter/src/transport_tests.rs`.
- Tauri mapping: `apps/desktop/src-tauri/src/lib.rs` tests.
- Desktop mirror: `apps/desktop/src/platform/protocol.ts`.

These checks prove the named seam, not a production WASM runtime, marketplace or SDK package. See the [developer walkthrough](plugin-developer-guide.md) and [ecosystem gate](plugin-ecosystem-qualification.md).
