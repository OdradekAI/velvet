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
cross-platform UI qualification.

## Findings

### P1: runtime identity is not bound at the plugin request entry point

`TauriTransport::bind_plugin_session` is exercised by tests but has no
production caller. The Tauri wire envelope accepts `callerExtensionId` and the
plugin request path dispatches it without deriving the identity from a host
owned runtime lease/package. This conflicts with the host-side identity and
capability boundary in ADR-0005 and can leave the endpoint unusable or open to
identity spoofing if exposed to an untrusted caller.

Follow-up: bind the extension identity when the host starts the runtime and
accept only the bound session at the Tauri/plugin request seam.

### P1: inbound UI Bridge messages are not fully validated at runtime

The surface adapter checks the message discriminator and then casts the
payload to `SurfaceToHostMessage`. Overlay, notification, progress and command
requests are forwarded without field schema, size, request-id or capability
validation. Malformed or oversized messages can therefore reach host actions.

Follow-up: validate every inbound message against the versioned UI Bridge
schema before dispatch, bound payload sizes and fail closed with an isolated
surface diagnostic.

### P1: dynamic capability revocation is not enforced on existing contributions

The capability broker revokes grants, but command invocation checks lifecycle
and registration without rechecking the contribution's required capabilities.
Revoking a command or asset capability can leave the contribution registered
and invokable until another lifecycle transition.

Follow-up: re-evaluate required capabilities for every contribution call and
remove or suspend affected projections on revocation.

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
