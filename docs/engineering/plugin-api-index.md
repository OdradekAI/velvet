---
id: plugin-api-index
title: API and protocol index
---

# API and protocol index

Audience: product-extension authors and host maintainers. Status: **in-repository Alpha**. This index maps existing contract owners; it does not expose every Rust or desktop type as a public SDK.

## Contract map

| Surface | Owner / source | Reference and use |
| --- | --- | --- |
| Manifest and contribution descriptors | `plugin-api` / `ExtensionManifest`, `ContributionDescriptor` | [Manifest](plugin-manifest-v1.md); stable IDs, scope, zone and requested capabilities |
| Host requests, responses and events | `workbench-protocol` / `PluginRequest`, `PluginResponse`, `PluginEvent` | Host services below; [envelope](plugin-manifest-v1.md#plugin-coretransport-envelope) |
| Isolated UI messages | Desktop `workbench/plugins/ui-bridge.ts` | [Bridge protocol](ui-bridge-protocol-v1.md); surface lifecycle and typed projections |
| Semantic theme / component negotiation | Desktop `workbench/plugins/ui-kit.ts` | [UI contract](ui-kit-contract-v1.md); not host React props |
| Registry implementation | `plugin-host` | [Maintainer registry record](plugin-registry-v1.md); not a plugin-owned mutable map |
| Fixture convenience handles | `examples/plugins/asset-inspector/src/fixture-host.mjs` | [Fixture HostFacade](plugin-developer-guide.md#fixture-hostfacade-api-catalog); not shipped SDK methods |

## Host services

Every request targets the bound caller extension. The adapter checks session and runtime lease; the host enforces lifecycle, capability and ownership policy. Request-specific DTOs live in `crates/workbench-protocol/src/protocol.rs`; errors use the host transport error path.

| Request kind | Additional input | Response kind | Authority and failure boundary |
| --- | --- | --- | --- |
| `invoke-command` | `commandId`, `args`, `context` | `accepted` | Declared command and host authorization; acceptance is not permission to replay |
| `query-projection` | — | `projection` | Read-only plugin projection |
| `query-capabilities` | — | `capabilities` | Effective grants; requested capabilities are not necessarily granted |
| `log` | `request: PluginLogRequestDto` | `accepted` | Active owner, schema, size, redaction and rate limits |
| `notify` | `request: PluginNotificationRequestDto` | `accepted` | Host-owned notification surface and bounded data |
| `confirm` | `request: PluginConfirmationRequestDto` | `confirmation` | Returns `accepted: boolean`; headless policy defaults to deny |
| `submit-job` | `jobKind`, `input`, `provenance`, `policy`, `budget` | `job` | Active declared Job and `jobs.create`; host overwrites provenance owner |
| `cancel-job` | `jobId` | `job` | Owned Job and active capability policy; runtime cancellation is cooperative |
| `query-job` | `jobId` | `job` | Owner-filtered snapshot and capability policy |

Events are `disabled` and `diagnostic`, carried with session/sequence metadata. On disconnect, query authoritative state and start a fresh session. An unknown mutation outcome must be reconciled before retrying; request IDs do not imply idempotency.

## How to use this reference

Choose a task in the [capability catalog](/platform/capabilities), then follow its runnable example. Check caller, capability, version, errors, cancellation and disposal before using a seam. The [contract matrix](plugin-contract-matrix.md) links owners to tests; [ecosystem qualification](plugin-ecosystem-qualification.md) records what is still required for an external SDK.
