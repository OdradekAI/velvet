# ADR-0015: Package trust and release response

Status: Proposed
Date: 2026-11-02

## Context

Tauri updater signature verification protects application artifacts, but it
does not establish trust for separately installed product extensions. A valid
signature also does not prove that a dependency was safe or that a package is
an entitlement. Velvet needs an offline-capable release and extension policy
that can recover from a bad package without making startup depend on a network
service.

## Decision

Application release trust and extension publisher trust are separate. The
decision remains Proposed until signed-package, key-rotation, revocation,
offline and installer fixtures pass the P1 qualification rows.

### Application release

`ApplicationCore`/`UpdateCoordinator` owns `ReleaseManifest` verification and
`UpdateTransaction`. A package is installable only when the pinned release root
(or an overlap-signed successor key), channel, version, target,
updater/data-schema ranges, minimum client version, digest, validity window and
monotonic release sequence all pass. A persistent release floor paired with
the committed `RecoverySet` prevents uninstall/reinstall from bypassing
anti-downgrade checks.

The Tauri updater adapter discovers and downloads bytes. It cannot choose an
arbitrary manifest, write a renderer-supplied path or commit a release.
Persistence stores the release floor, trust metadata and transaction journal
without interpreting signing policy. The previous package/data generation is
retained until `workbench-ready`; failure rolls back only the pending private
generation and enters Safe Mode. A failure before the application image can
execute remains an OS launcher/installer qualification boundary.

An explicitly selected offline installer uses the same signature, target,
digest, schema, minimum-version, channel and floor checks as an online update.
Stale discovery metadata blocks online discovery but does not prevent a
previously accepted application or RecoverySurface from starting. A skewed or
untrusted wall clock returns a typed freshness diagnostic; Job deadlines and
migration ordering use monotonic time.

### Extension publisher trust

`PluginHost` owns one `PluginTrustRecord` per installed extension. It binds the
package digest, `ExtensionId`, publisher key id, version, target/runtime,
manifest schema, host API range, declared capabilities, signature state,
accepted generation and optional provenance/SBOM references. The application
signer is not an extension publisher.

Before copy or activation, a bounded signature envelope must cover the package
digest and identity fields. Manifest/schema/target/capability and package-root
validation still apply. Invalid, unknown, revoked, expired or lower-generation
packages remain rejected or inactive. Unsigned local packages require explicit,
visible, Profile-scoped developer mode and never become production trust.

Publisher rotation requires an overlap-signed successor set. A cached
revocation or newer accepted generation wins over an otherwise valid old
signature. If a new key or freshness metadata cannot be verified offline, the
package is not activated and its committed data remains available.

SLSA/in-toto provenance and SPDX SBOM references are release pipeline evidence
and diagnostics. Stable-channel policy may require them, but they never replace
signature verification, capability policy or runtime isolation. A sidecar
process is an availability boundary, not a security sandbox.

Extension update is two-phase: inspect/verify, stage, lazy activate, health
check and commit. A failed activation or migration restores only the previous
extension package/data pair. Uninstall removes the package after stopping its
runtime but retains extension data and trust/recovery records until explicit
user deletion.

### Offline incident response

Incident response uses bounded signed `SafetyNotice` metadata rather than a
network-dependent startup service. A notice names an application release,
extension publisher/package or minimum-version floor and carries a sequence,
reason, validity window and signer. When present it can block install/commit,
disable/quarantine an extension, or force a typed update/recovery action for a
known compromised application generation. It never erases user data.

Offline guarantees are bounded: cached notices continue to apply, the current
previously accepted application may start, and an unknown extension is
degraded/disabled without blocking Core startup. Fresh notices are fetched by
release/PluginHost adapters when network access returns, verified before
persistence and surfaced in diagnostics; no user data is uploaded implicitly.

Trust states are explicit: `trusted`, `developer-unsigned`, `unknown`,
`revoked`, `incompatible`, `blocked` and `quarantined`. They are policy states,
not capabilities. A revoked extension loses projections, Job runners and
handles while its data remains recoverable or discardable.

### Supply-chain limits and incident assumptions

A signature cannot prove that a dependency or build input was safe. Protected
CI keys, locked dependencies, provenance/SBOM references, staged rollout and a
signed withdrawal notice reduce blast radius but do not provide cryptographic
proof of source integrity. This residual risk is recorded in diagnostics and
release review.

## Consequences

Positive:

- Offline installation and startup follow the same fail-closed compatibility
  rules as online updates.
- Extension revocation cannot take down Core or erase extension data.
- Application rollback, extension rollback and publisher revocation have
  separate owners and evidence.

Costs:

- Release engineering must protect root/publisher keys and publish signed
  metadata, provenance and platform packages.
- Trust records, release floors and SafetyNotices need bounded persistence and
  migration.
- Pre-image installer failures and remote notice distribution remain platform
  and service qualification work.

## Alternatives considered

1. Trust every extension with the application signer. Rejected because
   publisher identity and application release ownership differ.
2. Require a network check at startup. Rejected because offline recovery and
   local-first use must remain available.
3. Treat a valid signature as dependency provenance or entitlement. Rejected
   because integrity, build provenance, payment and capability policy have
   different issuers and lifecycles.
4. Delete a revoked extension and its data automatically. Rejected because
   incident response must preserve user recovery and explicit deletion choice.
5. Put kill-switch policy in the renderer or a generic Tauri command. Rejected
   because it would bypass the host trust and recovery boundary.

## Open questions

- Which concrete envelope format (for example COSE/DSSE) and release service
  will publish extension signatures and SafetyNotices?
- Which supported installers can retain the previous package atomically, and
  what adapter is required for the others?
- Which rollout service owns cohort assignment and online key-revocation
  distribution once the local signed-notice contract is stable?

## References

- [ADR-0005: Plugin host runtime, permissions and lifecycle](0005-plugin-host-runtime-and-permissions.md)
- [ADR-0013: Native lifecycle and recovery](0013-native-lifecycle-and-recovery.md)
- [Native desktop application and Tauri v2 research](../research/native-app-tauri-practices.md)
- [Workbench foundation plan](../engineering/workbench-foundation-plan.md)
