# ADR-0014: Portable data and operation safety

Status: Proposed
Date: 2026-11-02

## Context

Velvet already has owner-specific `StateSnapshot` generations, `RecoverySet`
coordination, extension backup participants and a user-initiated diagnostic
bundle. Those mechanisms protect local recovery, but they do not define a
portable user-data format or a safe contract for file, Job and remote side
effects. A renderer request that says “delete” or “restore” cannot be allowed
to become an implicit filesystem or remote-operation authority.

The problem crosses Profile, Workspace, Asset/source, PluginHost, JobSystem,
CoreTransport and persistence. A global backup database or generic operation
service would create a second owner and make an extension or unavailable source
block unrelated core state.

## Decision

Velvet uses a versioned logical bundle and a host-owned operation protocol. The
decision is proposed until the P0 contract fixtures and provider qualification
in the foundation plan pass.

### Logical bundle

The portable artifact has a bounded manifest with a bundle schema, selected
scopes, owner schema versions, integrity metadata, creation time and source
build. Sections are validated, migrated and staged before they can affect a
Profile. The artifact is data, not an executable package; it never supplies a
renderer path or a Tauri command.

The v1 scopes are:

- Profile preferences, locale and host-owned settings;
- Workspace app selection, layout, open-item references and user workspace
  state;
- Asset/source metadata, collections, user metadata, stable `AssetId` values
  and trusted fingerprints when present;
- extension data only through a bounded `BackupParticipant` representation;
  and
- diagnostics through a separate export-only `DiagnosticBundle`.

Raw Asset bytes, credentials, secrets, access tokens, caches, derived preview
or analysis results and unredacted environment data are excluded. Job results
are recomputed from complete provenance after import. Source paths and volume
identities are environment-bound references: they are revalidated and never
grant a capability or silently rebind an Asset.

Import follows this host-owned sequence:

```text
inspect → validate/migrate → stage owner sections → ImportPlan
       → explicit confirmation → RecoverySet commit → projection refresh
```

`ImportPlan` reports selected scopes, migrations, conflicts, unavailable
sources, skipped extension sections, estimated impact and expiry/revision. The
first contract restores explicitly selected owner scopes into the current
Profile. A silent merge of arbitrary Profile or Asset records is deferred
until each owner has a conflict model.

Required core sections commit atomically within the selected `RecoverySet`. A
failure before publication leaves the current set authoritative. Optional
extension sections are independent: a missing, untrusted or incompatible
extension does not block core restore; its bounded bytes are retained as inert
PluginHost quarantine data or explicitly discarded. An `ImportReceipt` records
per-owner status, migrations, quarantined sections and the committed
generation. A replace operation first preserves the current set as a normal
recovery point. Diagnostics are never imported into authoritative state.

An unavailable removable or network source is imported as metadata with
`SourceAvailability::degraded`. Preview and source-backed Jobs remain
retryable until the path, approved root and volume identity are revalidated.

### Operation protocol

Delete, overwrite, move, batch Job actions and remote operations use a typed
`OperationPlan`/`OperationReceipt` protocol. Domain owners retain side-effect
semantics and executors. `ApplicationCore` coordinates cross-owner policy and
the bounded journal through the existing persistence adapter; it does not
become a filesystem or remote-data owner.

Every operation has these phases:

1. **Validate:** check capability, owner, target identity, approved root and
   volume where relevant, current revision/fingerprint, conflict policy and
   compensation support.
2. **Preview:** issue an immutable plan id with item identities, expected
   revisions/fingerprints, risk, count, proposed changes, undo availability and
   expiry. Preview is not authorization.
3. **Commit:** require explicit confirmation and a stable operation id. Recheck
   every precondition; a changed target returns typed `plan-stale` and cannot be
   silently applied.
4. **Audit:** persist a bounded receipt with session/actor, per-item outcome,
   pre/post identity evidence, retryability and an optional compensation
   handle. Receipts contain no secrets.
5. **Undo/compensate:** start a new operation only when the owner supplies a
   valid undo token and its current preconditions still match. Undo is a
   bounded compensation attempt, not time travel.

The host writes `started` before invoking an external side effect and finalizes
`committed`, `partial`, `rolled-back`, `cancelled` or `unknown`. A crash or
disconnect requires querying the owner before retry. The host reuses an
operation id and per-item idempotency key only when the owner declares that
the effect is idempotent; a transport `requestId` alone never makes an external
effect safe to replay.

Local file operations validate canonical identity, approved roots and volume
identity. Trash or a bounded backup may provide an undo token; permanent delete
has no undo guarantee, and cross-volume copy/delete may be partial. Job
cancellation, retry and checkpoint restore remain JobSystem operations; an
`Interrupted` Job is not proof that external work was undone. Remote providers
must declare compensation or expose an explicit no-undo/unknown outcome.

### Ownership and failure isolation

- Profile, Workspace and Asset/source owners produce and validate their own
  logical sections and target identities.
- `PluginHost` owns extension sections, quarantine, participant migration and
  discard/retry actions.
- `JobSystem` owns Job state, checkpoints, provenance and Job-side effects.
- `ApplicationCore` coordinates selected owner transactions and operation
  policy; `persistence` stores bytes and receipts without interpreting domain
  meaning.
- `CoreTransport` carries typed envelopes and plans; Tauri maps them and never
  receives arbitrary paths or commands.

An extension can be uninstalled without making core import or Asset state
unusable. A stale preview, unavailable source, unknown IPC result or failed
optional participant remains visible and recoverable instead of becoming a
silent partial write.

### Implementation status (2026-11-03)

The protocol contract and persistence adapter now have a bounded
`OperationReceiptStore` with in-memory and atomic JSON-file implementations.
`ApplicationCore` adds an `OperationCoordinator` that revalidates observed
targets, serializes receipt transitions, writes `started` before invoking a
provider, preserves `unknown` outcomes after restart and blocks replay until
reconciliation. The coordinator still does not infer idempotency, grant file
or remote authority, or decide compensation validity; those remain provider
owner work.

The versioned `PortableBundle` and `ImportPlan` contracts now validate bounded,
path-free metadata, section ownership, integrity digests, schema compatibility
and plan expiry. `PortableBundleInspector` verifies selected section bytes and
keeps untrusted, corrupt or newer extension sections out of the selected core
import. It intentionally stops before owner payload decoding, staging or
`RecoverySet` publication; those steps remain the next owner-participant slice.

## Consequences

Positive:

- User-authored state can move between Profiles without copying secrets or
  stale derived data.
- Core restore has an atomic publication point while extension failures remain
  isolated.
- Destructive operations are reviewable, auditable and explicit about unknown
  or irreversible outcomes.
- Existing owner and persistence seams remain the only durable owners.

Costs:

- Every provider needs target identity, conflict and compensation semantics.
- Portable bundles and operation receipts need schema/version retention and
  bounded disk policy.
- A full merge model and cross-volume/remote undo cannot be promised until a
  concrete consumer supplies the necessary authority.

## Alternatives considered

1. Export the whole application directory. Rejected because it leaks secrets,
   caches, machine paths and incompatible generations.
2. Restore every owner independently. Rejected because a partial core restore
   can publish mixed Profile/Workspace/Asset generations.
3. Treat `requestId` as universal idempotency. Rejected because remote and
   filesystem side effects can have unknown outcomes.
4. Promise undo for every delete, Job or remote operation. Rejected because
   permanent deletion, cross-volume moves and remote providers may have no
   compensation.
5. Let the renderer or plugin own the operation journal. Rejected because it
   would bypass capability policy and create a second persistence owner.

## Open questions

- Which archive/container format and optional encryption should a future public
  bundle use?
- Which concrete file providers can guarantee trash/backup retention across
  volumes and platforms?
- When a consumer requires merge import, which owner-specific conflict rules
  can be added without changing the atomic selected-scope contract?

## References

- [ADR-0001: System topology and core ownership](0001-system-topology-and-core-ownership.md)
- [ADR-0002: Asset domain and derived data](0002-asset-domain-and-derived-data.md)
- [ADR-0005: Plugin host runtime, permissions and lifecycle](0005-plugin-host-runtime-and-permissions.md)
- [ADR-0007: Jobs, indexing and cache invalidation](0007-jobs-indexing-and-cache-invalidation.md)
- [ADR-0013: Native lifecycle and recovery](0013-native-lifecycle-and-recovery.md)
- [Workbench foundation plan](../engineering/workbench-foundation-plan.md)
