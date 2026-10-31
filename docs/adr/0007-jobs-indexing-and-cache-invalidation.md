# ADR-0007: Jobs, indexing and cache invalidation

Status: Proposed
Date: 2026-10-28

## Context

Thumbnail generation, RAW decoding, 3D previews, video analysis, AI tagging and feed synchronization can be expensive. They must not block the workbench, and their results must remain explainable when an extension is disabled or an input file changes.

## Proposed decision

Use one Rust-owned `JobSystem` for indexing, preview analysis, thumbnail generation and synchronization. A job has an extension or core owner, an input fingerprint, a progress stream, cancellation, retry policy and a result status.

Derived artifacts are cached by:

```text
content fingerprint
+ job kind
+ provider / extension id
+ provider version
+ settings profile hash
```

When an input, provider or setting changes, the cache becomes stale and the job is rescheduled. The workbench observes jobs through events and renders them in `OperationsPanel`.

The JobSystem is also the authoritative owner for Job result and log state.
Persistence supplies only a versioned storage adapter. A Job snapshot carries
its owner, input/provider fingerprint, schema/version, progress, cancellation,
retry and recovery status; a renderer or extension cannot persist a second Job
record. On restart, queued or running work becomes an explicit retryable
interrupted state, while derived results remain invalidatable and
recomputable independently of the extension that produced them.

The execution seam is a Rust `JobRunner` registry keyed by owner and declared
Job kind. Runner registrations are removed when their owner is disabled,
crashes or uninstalls; queued and running Jobs owned by that extension are
cancelled before the runtime is shut down. A runner receives an opaque `JobExecutionContext` with the Job id,
attempt, input, provenance, progress reporter and cooperative cancellation
handle; it returns a typed content type and JSON payload or a typed failure. A
plugin-backed runner uses the isolated runtime's `invoke-job`/`job.run` seam;
runtime input and result payloads are size-checked, progress events are bounded
and projected through the same reporter.
The `JobSystem` supplies a deterministic runner only for Jobs owned by `core`;
an extension-owned Job with a missing runner becomes a retryable diagnosed
failure instead of executing under another owner or the host fallback. It
catches runner panics and rejects stale progress/completion from an older
attempt after cancellation and retry.

`JobProvenance` is part of the persisted Job snapshot and includes the owner,
optional Asset/content identity, input fingerprint, provider id/version,
settings fingerprint and provenance schema version. A successful runner stores
one `JobResult` with its own schema version, content type and payload beside
the snapshot. Tauri and the desktop receive this as a read-only projection;
they never write results or provenance.

The JobSystem exposes an exact provenance reuse seam for capability owners. A
caller may submit through `submit_or_reuse`; it returns a compatible completed
result only when an input fingerprint is present, the Job kind matches and the
entire normalized provenance matches. Missing fingerprints are always cache
misses. The lookup and submission are serialized so concurrent callers do not
create duplicate work through this seam. Stale or incompatible results remain
diagnosable snapshots and are never presented as a current cache hit.

## Consequences

The v0.0.3 host slice implements the shared lifecycle seam: JobSystem owns
snapshots and transitions, the persistence adapter writes them under the
application data directory, and projections travel through CoreTransport.
Queued/running work maps to retryable `Interrupted` state after a restart. The
current slice now also proves runner registration, cooperative cancellation,
panic isolation, persisted result payloads and provenance projection. Product
thumbnail, indexing, feed-sync and facet-specific runners remain consumers of
the same contract. Capability owners can now opt into exact provenance reuse
without creating a second cache or persistence owner.

Positive:

- All long-running work has consistent cancellation and progress behavior.
- Plugin-produced data can be recomputed and diagnosed.
- The UI does not need format-specific background task code.

Costs:

- Job schemas and cache policies must be versioned.
- Backpressure and memory limits need explicit defaults.

## Open questions

- Which jobs may run concurrently per asset and per device?
- How are partial results exposed to inspectors?
- How long are failed artifacts retained for diagnosis?
