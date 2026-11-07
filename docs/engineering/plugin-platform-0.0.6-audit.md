# Plugin platform 0.0.6 audit

Audit range: `v0.0.5` (`c13753f`) through `HEAD` before the 0.0.6 version
iteration. The audit covered the 15 commits that introduced the Alpha plugin
platform, UI Kit, Asset Inspector fixture and final validation remediation.
No GitHub release was created.

## Result

The repository is internally consistent for the Alpha fixture gate:

- Rust workspace, independent Tauri workspace and desktop fixtures pass;
- the desktop platform/workbench dependency direction is explicit;
- diagnostics coverage is part of `npm test`;
- strict Clippy passes with one documented local exception on the internal
  `Request` dispatch envelope;
- package and development dependency versions are aligned to `0.0.6`;
- `npm audit` reports no vulnerabilities after updating the test-only
  `happy-dom` dependency to `21.0.4`.

This is an Alpha source iteration, not an ecosystem release. The repository
does not claim a public SDK, production WASM sandbox, Marketplace ABI or
cross-platform UI qualification. The three original P1 findings below have
since been remediated and are recorded as verified evidence against the
current HEAD; the remaining open findings are P2.

## Findings

### Resolved P1 evidence: runtime identity binding

The plugin request seam now requires a host-issued runtime lease nonce. The
nonce is published by `PluginHost`, projected through `ApplicationCore`, and
verified by `TauriTransport` before a transport session is bound. A missing,
mismatched or inactive lease fails closed; runtime shutdown removes the nonce
and persisted lease, so a stale request cannot be reused. The declared
`callerExtensionId` remains namespace metadata and is never treated as proof of
identity.

Evidence and checks:

- Implementation: `crates/plugin-host`, `crates/application-core`,
  `crates/tauri-adapter` runtime lease and transport binding seams.
- Contract fixtures: valid bound session, spoofed/missing nonce, shutdown
  cleanup and stale-session rejection.
- Verification: `cargo test -p velvet-workbench-protocol`,
  `cargo test -p velvet-tauri-adapter`, `cargo test -p velvet-plugin-host`,
  and the Tauri desktop test suite passed.
- Decision record: [Bind plugin runtime identity at the transport seam](../../.scratch/velvet-ui-kit-and-plugin-boundaries/issues/01-bind-plugin-runtime-identity.md).

Status: **verified on current HEAD**.

### Resolved P1 evidence: UI Bridge payload validation

Every inbound `SurfaceToHostMessage` now passes one host-owned validator before
dispatch. The validator rejects unknown fields, unsupported discriminators,
empty or oversized identifiers/text, invalid overlay/progress DTOs,
non-JSON command arguments and messages above the 64 KiB bridge budget. Source
and origin checks remain in the iframe and Tauri WebView adapters. Invalid
messages fail closed through the bounded `surface.error` path and isolate the
affected surface.

Evidence and checks:

- Implementation: the shared UI Bridge validator used by the surface adapters
  and in-memory endpoint.
- Contract fixtures: valid messages, unknown fields, oversized command
  payloads and origin spoofing.
- Verification: desktop typecheck, test compilation,
  `plugin-surface-adapter.test.mjs` and `ui-kit-contract.test.mjs` passed.
- Decision record: [Validate inbound UI Bridge messages before host dispatch](../../.scratch/velvet-ui-kit-and-plugin-boundaries/issues/02-validate-ui-bridge-messages.md).

Status: **verified on current HEAD**.

### Resolved P1 evidence: dynamic capability revocation

`InMemoryPluginHost` now treats contribution requirements as a live boundary.
Revoking a capability removes only affected command, view, surface or service
registrations, while unrelated contributions and the extension runtime remain
available. Every invocation rechecks the current capability snapshot, so a
stale handle cannot bypass revocation. Regranting restores eligible manifest
contributions through the normal registry validation path and preserves typed
denial reasons.

Evidence and checks:

- Implementation: `crates/plugin-host` capability broker, contribution
  registries and invocation guard.
- Contract fixtures: revoke-before-invoke, revoke-while-active, regrant and
  unaffected-extension isolation.
- Verification: `cargo fmt --all -- --check` and
  `cargo test -p velvet-plugin-host` passed.
- Decision record: [Enforce capability revocation on active plugin contributions](../../.scratch/velvet-ui-kit-and-plugin-boundaries/issues/03-enforce-capability-revocation.md).

Status: **verified on current HEAD**.

### P2: manifest discovery is unbounded and too broad

Manifest discovery reads JSON files without a manifest-size limit and treats
every JSON file beneath a plugin root as a candidate. Declared resources such
as localization JSON can produce false manifest failures, while a large file
can cause an avoidable allocation before validation.

Follow-up: discover package-root `manifest.json` entries only and enforce a
bounded read before deserialization.

### P2: lazy activation and some declared host boundaries remain fixture-only

`activation_candidates` and `activate_for_event` exist, but application and
transport event sources do not invoke them. Feed ownership, event topics/
sequence/backpressure and configuration read/write/migration contracts are
declared or documented but not wired as complete host capabilities.

Follow-up: keep these surfaces deferred until a real consumer defines the
owner, persistence and compatibility requirements.

## Scope interpretation

The original `.scratch/velvet-plugin-platform/spec.md` described a planning
stage that did not modify implementation code. Subsequent user instructions
explicitly authorized implementation of the plugin platform phases, so the
large source diff is intentional scope evolution rather than an unnoticed
change. The current Alpha freeze and ecosystem qualification documents make
that transition explicit.

## Verification

- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed: 338 tests plus doc-test harnesses.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` passed: 78
  desktop tests and 5 launcher tests.
- `npm --prefix apps/desktop run typecheck` passed.
- `npm --prefix apps/desktop test` passed, including diagnostics, shortcut,
  UI Kit, Demo and updater fixtures.
- `node tools/release-qualification/test-updater-release.mjs` passed.
- `npm audit` passed with zero vulnerabilities.
