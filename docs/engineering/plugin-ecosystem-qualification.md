# Plugin ecosystem qualification gate

Status: deferred ecosystem gate; observed on 2026-11-06.

This document defines the evidence required before the repository-owned Alpha
host contract can become an external SDK contract. It is a qualification plan,
not an implementation of a Marketplace, remote deployment service or production
WASM/sidecar runtime. Results use the same vocabulary as the
[plugin contract matrix](plugin-contract-matrix.md): `pass`, `pass (fixture)`,
`partial`, `not run` and `deferred`.

## Promotion rule

The external SDK gate remains `deferred` until every row below is `pass` or an
explicitly accepted release qualification result, there is no unresolved P1,
and a second real consumer has completed the independent developer path. The
Alpha contract and its compatibility fixtures remain the only frozen contract
until that promotion decision is recorded in an accepted ADR.

## Evidence matrix

| Gate | Owner | Required input | Evidence seam | Current result | Pass condition |
| --- | --- | --- | --- | --- | --- |
| Second real consumer | Developer experience + Workbench | Independent extension using only published docs | Separate package repository or isolated workspace; install, upgrade, disable, uninstall and recovery fixtures | `deferred` | A consumer completes the full lifecycle without internal imports or host-specific test doubles |
| Runtime containment | PluginHost + JobSystem + platform release | WASM/sidecar package and resource policy | Runtime budget tests, cooperative cancel/timeout, crash and malformed-response injection, orphan reconciliation, watchdog evidence on target OSes | `partial` | Resource limits, cancellation, crash isolation, orphan cleanup and watchdog behavior pass on every supported target |
| Package trust and rollback | PluginHost + release engineering | Signed package, publisher key and incident documents | Digest/identity binding, publisher trust, revocation, anti-downgrade, offline cache, staged install and rollback fixtures | `partial` | Signed packages and revocation/rollback behavior pass online and offline, including key rotation and stale metadata |
| UI Kit platform qualification | Desktop UI + accessibility | Supported OS/WebView matrix and UI Kit test packages | Theme/token, origin, keyboard, focus transfer, screen reader, reduced motion and accessibility probes; plugin surface dispose/retry | `deferred` | Every supported OS/WebView passes the host/plugin boundary without arbitrary DOM or layout ownership |
| Transport and state authority | Protocol + ApplicationCore | Session interruption and unknown-outcome scenarios | CoreTransport reconnect/resync, stale session rejection, Job provenance, cancellation and plugin failure-isolation fixtures | `pass (fixture)` | Fixtures pass and target runtime evidence confirms no duplicate state owner or lost recovery path |
| Failure isolation and recovery | PluginHost + Workbench | Crashing, hanging, disabled and incompatible extensions | Contribution cleanup, Job cancellation, UI unload, diagnostic projection, core Asset/layout/Preview persistence checks | `pass (fixture)` | Extension failure leaves core state and unrelated extensions usable in every runtime path |
| SDK/testkit lifecycle | Architecture + developer experience | Version policy, compatibility matrix and deprecation plan | SDK package, fixture/testkit release, retirement and migration tests; API diff review | `deferred` | Versioned SDK and testkit can be published, deprecated and retired without undocumented compatibility promises |
| Release readiness | Release engineering + architecture | Qualification report and issue inventory | `cargo clippy --workspace --all-targets -- -D warnings`, platform reports, P1 triage and accepted ADR references | `partial` | No unresolved P1, strict lint passes, all target qualification reports are attached and ADR status is explicit |

## Current Alpha evidence

The following are valid repository-owned evidence and must remain labeled as
fixtures where they do not exercise a production runtime:

- `crates/plugin-api`, `plugin-host`, `workbench-protocol`, `application-core`
  and `job-system` contract tests;
- the independent Tauri workspace tests and the desktop `npm test` gate;
- `examples/plugins/asset-inspector` and `examples/plugins/echo-sidecar`;
- the fake `HostFacade` and UI Kit bridge/component fixtures;
- the [plugin contract matrix](plugin-contract-matrix.md) and
  [plugin developer guide](plugin-developer-guide.md).

These fixtures prove typed seams, lifecycle cleanup, permission checks,
cancellation and failure projection. They do not establish package signing,
publisher revocation, a security sandbox, target-platform accessibility or a
public SDK ABI.

## Ownership and change control

The Alpha contract remains owned by `plugin-api`, `plugin-host`,
`workbench-protocol`, `application-core`, `JobSystem`, the desktop platform
mirror and the host UI Kit. This qualification document does not create a new
crate, protocol owner or durable state store. A promotion to an external SDK
must update the relevant ADR, glossary, compatibility matrix, release notes and
developer documentation in one review.
