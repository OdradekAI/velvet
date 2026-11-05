# Plugin contract and integration validation matrix

Status: active alpha contract gate; observed on 2026-11-06.

This document is the issue 010 entry point for validating the plugin boundary
across the Rust workspace, the independent Tauri workspace, and the desktop
TypeScript/UI Bridge. It records repeatable fixtures and the limits of each
result. A green unit or contract test proves the named seam; it does not prove
that a production marketplace, package signature, or platform sandbox is
available.

## Result vocabulary

| Result | Meaning |
| --- | --- |
| `pass` | The named deterministic fixture or command ran and passed. |
| `pass (fixture)` | The seam is covered by a fake or local fixture; platform qualification is still separate. |
| `partial` | The host contract passes, but a target platform, release artifact, or production runtime gate is open. |
| `not run` | The command or environment was unavailable for the recorded run. It is not a pass. |
| `deferred` | The scenario is outside the current alpha scope, such as Marketplace operations or a physical power cut. |

## Three validation layers

Run these layers independently because the root and Tauri workspaces do not
share one Cargo workspace and the frontend has its own protocol mirror.

| Layer | Command | Covers | Current result |
| --- | --- | --- | --- |
| Root Rust workspace | `cargo fmt --all -- --check` | Formatting for the ten root crates | `pass` |
| Root Rust workspace | `cargo test --workspace` | Domain, protocol, manifest, registry, capability, Job, storage, diagnostics and ApplicationCore contracts | `pass` |
| Root Rust workspace | `cargo clippy --workspace --all-targets` | Cross-crate lint and public seam regressions | `pass` |
| Tauri workspace | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` | Tauri JSON mapping, command policy, native lifecycle and platform adapters | `pass` |
| Desktop frontend | `npm --prefix apps/desktop run typecheck` | TypeScript protocol mirror, Workbench, PluginClient, UI Bridge and renderer adapters | `pass` |
| Desktop frontend | `npm --prefix apps/desktop test` | Compiled renderer tests and the complete desktop fixture suite | `pass` |

The result values above are the local run recorded for this checkout on
2026-11-06. If a command cannot run on a target, record `not run` or
`deferred` with the exact environment reason rather than changing it to
`pass`.

## Cross-boundary contract matrix

| Contract | Rust owner and seam | Tauri/transport seam | Desktop/UI seam | Repeatable evidence | Result |
| --- | --- | --- | --- | --- | --- |
| Manifest schema and identifiers | `plugin-api::ExtensionManifest`, `validate_contract` | `PluginManifestDto` and Tauri JSON mapping | `apps/desktop/src/platform/protocol.ts` mirror | `crates/plugin-api/src/lib.rs` manifest round-trip/unknown-field tests; Asset Inspector fixture test; `plugin-manifest-v1.md` | `pass` |
| Plugin envelopes and session policy | `workbench-protocol::PluginRequestEnvelope` | `tauri-adapter` session, request outcome and event mapping | `apps/desktop/src/platform/tauri-transport.ts` | `crates/workbench-protocol/src/protocol_tests.rs`; `crates/tauri-adapter/src/transport_tests.rs`; `apps/desktop/test/tauri-transport.test.mjs` | `pass` |
| Registry lifecycle and contribution cleanup | `plugin-host::PluginHost`/`InMemoryPluginHost` | Plugin registry projection through `ApplicationCore` | `PluginClient`, `ViewRegistry`, command/menu registries and UI Bridge | `crates/plugin-host/src/host_tests.rs`; `apps/desktop/test/plugin-contribution-lifecycle.test.mjs`; `plugin-registry-v1.md` | `pass` |
| Capability and zone enforcement | `plugin-host` capability policy | Typed manifest/projection validation before dispatch | Renderer consumes already-filtered projections | `crates/plugin-host/src/capabilities.rs` and `host_tests.rs`; `apps/desktop/test/plugin-surface-adapter.test.mjs` | `pass` |
| Commands, menus and keybindings | Host-owned contribution descriptors and invocation validation | `invoke-plugin-command` CoreTransport path | `CommandRegistry`, `MenuRegistry`, keybinding adapter | `crates/plugin-host/src/host_tests.rs`; `apps/desktop/test/external-plugin-e2e.test.mjs`; `apps/desktop/test/workbench-dom-regression.test.mjs` | `pass` |
| Diagnostics, notifications and recovery | `PluginDiagnosticSink` and bounded ApplicationCore history | Typed diagnostic projection/export | Diagnostics service, recovery operations and notification seams | `crates/plugin-host/src/host_tests.rs`; `crates/application-core/src/composition_tests.rs`; `apps/desktop/test/diagnostics-service.test.mjs`; `apps/desktop/test/plugin-recovery-operations.test.mjs` | `pass` |
| Jobs, cancellation and provenance | `job-system::JobSystem` and `PluginJobInvocation` | Job request/result DTOs and cancellation commands | `JobClient` and Operations panel projections | `crates/job-system/src/lifecycle_tests.rs`; `crates/plugin-host/src/host_tests.rs`; `apps/desktop/test/job-operations.test.mjs` | `pass` |
| UI Kit theme and component negotiation | Host-owned UI bridge schema | Bridge version and origin policy | `PluginUiRegistry`, `PluginSurfaceHost`, token sanitizer | `apps/desktop/test/ui-kit-contract.test.mjs` (schema, token filtering, theme update, component fallback); `apps/desktop/test/plugin-surface-adapter.test.mjs` (origin and dispose); `apps/desktop/test/workbench-dom-regression.test.mjs` (ARIA labels and focus restoration); `ui-kit-contract-v1.md` | `pass (fixture)` |
| Asset Inspector developer path | Namespaced extension facet and Job owner | Manifest/projection/Job DTOs | Preview augmentation and Views | `examples/plugins/asset-inspector/manifest.json`; `examples/plugins/asset-inspector/src/asset-inspector.mjs`; `apps/desktop/test/asset-inspector-demo.test.mjs` | `pass (fixture)` |
| External sidecar failure isolation | `ProcessPluginRuntime` and per-extension failure domain | Runtime state, disable and recovery projection | UI surface removal and remount after recovery | `examples/plugins/echo-sidecar`; `crates/plugin-host/src/host_tests.rs`; `apps/desktop/test/external-plugin-e2e.test.mjs` | `pass (fixture)` |
| Extension storage migration | `PluginHost::ExtensionStorage` and migration participants | Manifest storage declaration and recovery actions | Recovery UI only consumes metadata projection | `crates/plugin-host/src/host_tests.rs`; `apps/desktop/test/asset-inspector-demo.test.mjs` | `pass` |
| Install, upgrade, rollback and uninstall | ApplicationCore package transaction and PluginHost lifecycle | Typed inspect/install/commit/rollback commands | Extensions dialog and recovery operations | `crates/application-core/src/composition_tests.rs`; `apps/desktop/test/plugin-manager-dialog.test.mjs`; `apps/desktop/test/plugin-package-picker.test.mjs` | `pass (fixture)` |
| Compatibility and migration surfaces | `plugin-api` schema/range types; PluginHost storage and facet owners | Manifest, runtime, storage and recovery DTOs | UI Bridge/component/theme negotiation | `crates/plugin-api/src/lib.rs`; `crates/plugin-host/src/host_tests.rs`; `crates/workbench-protocol/src/protocol_tests.rs`; `apps/desktop/test/ui-kit-contract.test.mjs`; Asset Inspector schema/migration fixture | `pass (fixture)` |

Facet schema is a namespaced owner contract rather than a second global schema
registry. The current fixture carries `schemaVersion: 1` and an input
fingerprint; a future incompatible facet schema must add an owner migration or
recomputation fixture before it is accepted. The host must keep the Asset
identity readable while the facet is stale, disabled or quarantined.

The matrix deliberately keeps `PluginHost` as the owner of extension lifecycle,
capabilities, diagnostics and extension storage. `CoreTransport` carries typed
projections and commands; it does not become a second plugin owner. The
frontend owns rendering and transient interaction state, while Rust remains the
authority for durable state and failure recovery.

## Fixture directory convention

Use the narrowest fixture location that owns the seam:

```text
examples/plugins/<extension-id>/
  manifest.json          # package-level manifest fixture
  README.md              # developer-facing contract and limitations
  README.zh.md           # Chinese companion when the fixture is public
  src/ or sidecar.mjs    # deterministic fixture implementation

crates/<crate>/src/*_tests.rs
crates/<crate>/tests/     # Rust contract or qualification fixtures
apps/desktop/test/        # frontend, transport and UI Bridge fixtures
tools/release-qualification/fixtures/
                            # release/platform fixtures only
```

Fixtures must be deterministic, local, bounded and independent of user Asset
data. A plugin fixture may use fake host handles, but it must not read a real
database, source path, Tauri state object, React component, or sibling View.
Runtime fixtures reserve stdout for protocol data and must bound stderr and
payload sizes. A fixture that models failure must also assert cleanup: removed
contributions, cancelled or interrupted Jobs, preserved core state and no
orphan runtime handle.

## Failure and recovery coverage

Every new cross-boundary contract must add at least one fixture for each of
these paths before it is considered complete:

1. valid round-trip or projection;
2. unknown schema, version, capability or contribution rejection;
3. cancellation, timeout or stale-result handling where the contract is
   asynchronous;
4. owner failure with unrelated extensions and core state still usable;
5. disable, upgrade, rollback or uninstall cleanup;
6. migration or recovery of extension-owned data without changing Asset,
   layout or Profile authority.

The negative path is part of the contract. A test that only asserts a success
projection is insufficient for plugin work.

## Qualification limits

The following remain explicit gates rather than hidden assumptions:

- `RuntimeKind::Wasm` and `RuntimeKind::Sidecar` describe a public contract;
  they do not make a sidecar a security sandbox or prove a production WASM
  runtime.
- Publisher signing, revocation, Marketplace publication, notarization and
  installer rollback require release qualification and are not proven by the
  local fixtures.
- macOS/Linux native lifecycle evidence exists in the release harness; Windows
  runtime, Job Object and installer qualification remain environment gates.
- A passing `npm test` proves the current desktop harness. It does not qualify
  every WebView, GPU, filesystem, power or network-volume combination.
- The current fixture `HostFacade` is a structural test seam, not a shipped SDK.
  A public SDK requires an accepted API/versioning ADR and a second consumer.

For the developer-facing walkthrough and the fixture-only HostFacade catalog,
see [plugin-developer-guide.md](plugin-developer-guide.md). For the individual
contracts, see [plugin-manifest-v1.md](plugin-manifest-v1.md),
[plugin-registry-v1.md](plugin-registry-v1.md) and
[ui-kit-contract-v1.md](ui-kit-contract-v1.md).
