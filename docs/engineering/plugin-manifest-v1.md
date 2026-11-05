# Velvet plugin manifest and CoreTransport contract (schema 1)

This document records the narrow contract implemented by issue 002. It is a
wire and validation reference, not an acceptance of the proposed runtime in
ADR-0006. `RuntimeKind` is an existing declaration in `plugin-api`; this
document does not make WASM or sidecar execution available to packages.

## Ownership and boundaries

- `crates/plugin-api/src/lib.rs` owns manifest meaning, identifier validation,
  contribution descriptors and permission declarations.
- `crates/workbench-protocol/src/protocol.rs` owns the transport mirror and the
  plugin-only request, response and event envelopes.
- `crates/tauri-adapter/src/transport.rs` applies session, request-outcome and
  event policy before dispatching to `ApplicationCore`.
- `apps/desktop/src-tauri/src/lib.rs` maps Tauri JSON into the typed contract;
  command handlers do not own manifest or runtime state.
- `apps/desktop/src/platform/protocol.ts` is the explicit desktop mirror. It
  does not read package files or call a plugin runtime directly.

## Manifest schema 1

The canonical JSON names are camelCase. The following fields are required by
`ExtensionManifest` deserialization:

| Field | Meaning |
| --- | --- |
| `manifestSchema` | Numeric manifest schema. Current value: `1`. |
| `id` | Stable extension identifier. It remains stable across upgrades. |
| `version` | Package version selected by the host. |
| `hostApi` | Host API compatibility range. |
| `bridgeApi` | UI bridge compatibility range. |
| `runtimeProtocol` | Runtime message protocol range. |
| `runtime` | Declared runtime form (`built-in`, `wasm`, or `sidecar`). The declaration is not proof that the runtime exists. |
| `extensionKind` | One or more of `ui`, `workspace`, `capability`. |
| `launch` | Runtime launch descriptor when required by host policy. |
| `activationEvents` | Lazy activation selectors. `onView` and `onCommand` selectors must name a contribution in the same manifest. |
| `extensionDependencies` | Stable extension IDs that must be present before activation. |
| `targetApp` | Optional WorkbenchApp target and API range. |
| `support` | Trust, virtual-workspace and profile support declarations. |
| `requiredCapabilities` / `optionalCapabilities` | Declarations only; effective grants remain host policy. |
| `localization` | Optional package-relative localization declaration. |
| `workbenchApp` | Optional WorkbenchApp descriptor. |
| `contributions` | Contribution descriptors with stable local IDs. |
| `permissionSets` | Zone-scoped permission declarations. |

`storage` is optional. Contribution fields such as `namespace`, `zone`,
`container`, labels, UI and Preview metadata are optional and are validated by
the host according to contribution kind and scope.

Unknown fields are rejected. The only forward-extension escape hatch is a
top-level field whose name begins with `x-`; the host must not interpret those
fields as capabilities.

## Identifier and upgrade rules

- Extension IDs and contribution IDs use the bounded identifier grammar
  `[A-Za-z0-9][A-Za-z0-9._-]*` and are limited to 128 bytes.
- Contribution IDs are unique within one manifest. The host derives the
  authoritative namespace from the extension ID and contribution scope; a
  package cannot override that namespace through the optional echo field.
- An upgrade keeps the extension ID and changes the package version. A new
  extension ID is a different installation and does not inherit the old
  extension's state automatically.
- Duplicate dependency IDs and activation references to missing contributions
  are rejected by `ExtensionManifest::validate_contract`.
- Zone, container, permission and target-app rules remain `PluginHost` policy;
  they are not silently moved into the transport crate.

## Plugin CoreTransport envelope

`PluginRequestEnvelope` carries:

```json
{
  "protocolVersion": 1,
  "sessionId": "opaque-host-session",
  "requestId": 7,
  "message": {
    "kind": "invoke-command",
    "extensionId": "publisher.reader",
    "commandId": "reader.inspect",
    "args": {},
    "context": {}
  }
}
```

The public message set is deliberately small:

- `invoke-command` forwards a declared command through the existing
  `Command::InvokePluginCommand` path;
- `query-projection` reads a Rust-owned `PluginProjectionDto`;
- `PluginResponseEnvelope` returns `accepted` or a plugin projection;
- `PluginEventEnvelope` carries only plugin-disabled and plugin-diagnostic
  events.

Invocation is not replayable by request ID. Projection queries are repeatable.
Session validation, unknown mutation outcomes and snapshot resynchronization
remain the general `CoreTransport`/`TauriTransport` policy. A plugin envelope
cannot address a database, file path, Tauri state object or runtime process.

## Compatibility policy

- A plugin protocol major mismatch fails before dispatch.
- Minor protocol evolution may add optional fields; required field or message
  meaning changes require a new major version.
- `manifestSchema`, `hostApi`, `bridgeApi` and `runtimeProtocol` are negotiated
  independently. The current host accepts schema `1`, host API `1`, bridge API
  `1` and runtime protocol `1`; unsupported ranges fail closed in
  `PluginHost`. Adding another supported range requires a compatibility fixture
  and an explicit host decision.
- Unknown required fields fail closed. Optional future metadata must use the
  reserved `x-*` namespace until a later schema explicitly defines it.
- A reconnect creates a new transport session. Old plugin sessions and events
  are not replayed; callers query authoritative state and resync through the
  existing snapshot contract.

## Contract fixtures

- Rust manifest and envelope fixtures: `crates/plugin-api/src/lib.rs` and
  `crates/workbench-protocol/src/protocol_tests.rs`.
- Session and idempotency path: `crates/tauri-adapter/src/transport_tests.rs`.
- Tauri JSON mapping and unknown-field rejection:
  `apps/desktop/src-tauri/src/lib.rs` tests.
- Desktop mirror and conversion helper:
  `apps/desktop/src/platform/protocol.ts`.

The fixture set intentionally proves the public seam only. It does not prove
that a production WASM/sidecar runtime, marketplace, signing policy or SDK
package exists.

For the end-to-end evidence map and result vocabulary, see
[plugin-contract-matrix.md](plugin-contract-matrix.md). For the developer
walkthrough and the fixture-only HostFacade catalog, see
[plugin-developer-guide.md](plugin-developer-guide.md).
