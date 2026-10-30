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

## Consequences

The v0.0.3 host slice implements the shared lifecycle seam: JobSystem owns
snapshots and transitions, the persistence adapter writes them under the
application data directory, and projections travel through CoreTransport.
Queued/running work maps to retryable `Interrupted` state after a restart. The current sample worker exists to prove the boundary;
thumbnail, indexing, feed-sync and facet-specific providers remain future
consumers of the same contract.

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
