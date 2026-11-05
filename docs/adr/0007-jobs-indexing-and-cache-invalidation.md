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

The Asset/source owner supplies the complete-content fingerprint through the
bounded `PreviewAssetSource` seam when a preview or capability needs it. The
preview resolver may query that value but never computes an identity from an
opaque path or a bounded preview slice. A source without a fingerprint returns
an explicit cache miss; updating content without a new trusted fingerprint
clears the in-memory source's previous fingerprint rather than reusing it.

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
runtime input and result payloads are size-checked, result schema versions are
validated, and progress/checkpoint/cancelled events are bounded and projected
through the same reporter. A plugin-owned `cancelled` event is cooperative
self-cancellation; the durable transition remains owned by `JobSystem`.
The `JobSystem` supplies a deterministic runner only for Jobs owned by `core`;
an extension-owned Job with a missing runner becomes a retryable diagnosed
failure instead of executing under another owner or the host fallback. It
catches runner panics and rejects stale progress/completion from an older
attempt after cancellation and retry.

An isolated plugin runtime failure is a host event with extension id, runtime
session, Job id, attempt, failure kind (`crash`, `trap`, `timeout`, malformed
response or budget kill), exit evidence and retryability. `JobSystem` converts
the event into an interrupted/failed Job snapshot and rejects late events; it
does not replay an unknown external side effect. A runtime crash affects only
Jobs owned by that extension under the default one-process-per-extension
failure domain. A Core crash or power loss interrupts every in-flight Job and
relies on the persisted snapshot/checkpoint path on the next launch.

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

Only derived results with complete provenance are eligible for cache-style
invalidation and recomputation. A runner that produces user-authored or
authoritative domain data must hand that data to its domain owner for its own
snapshot and migration policy; it cannot rely on the derived-result exclusion
from core recovery.

Until a concrete Preview or Facet consumer needs a materialized artifact store,
the completed Job result index is the only reusable cache index. Provider and
Facet owners do not create a second generic cache: changing content, provider
identity/version or settings produces a new provenance and therefore a new
Job. A future owner-specific artifact store must use the same key and declare
its own retention, invalidation and recovery policy.

### Resource governance, cancellation and sleep recovery

`JobSystem` schedules work under device-global, owner and Job-class budgets
through a versioned `ResourceBudgetPolicy`. The selected platform/workload
profile provides hard limits for memory, disk, file descriptors, process count,
queue depth and concurrency. CPU and network are soft budgets enforced through
priority, byte accounting and backpressure. Each accepted Job claim is
persisted in its snapshot and projected to plugin runners as read-only data;
the alpha ledger exposes bounded device/owner pressure and delays new starts
instead of rejecting a hard-valid Job. Network work is allowed only when
its owner has the capability and the current offline/metered policy permits it.
The alpha profile must be bounded and observable; numeric limits remain release
configuration rather than a Job protocol promise. A process-local device tier
is not reported as a strict cross-profile limit until a coordinator exists.

Cancellation is cooperative. A cancellation token is persisted with the Job
attempt; the runner must observe it and return a typed cancelled outcome. A
healthy shared plugin runtime is not terminated merely because one Job was
cancelled. A deadline, malformed response, crash or unhealthy runtime may
receive a bounded grace period and be terminated by `PluginHost`; a core
in-process runner cannot be force-killed and instead fails through its
deadline/diagnostic path. `JobSystem` rejects all late progress and completion
using the Job id, attempt and provenance.

Only typed transient failures are automatically retried. The default policy
allows three attempts with bounded exponential backoff. The alpha scheduler
does not add random jitter so retry timing remains deterministic and testable;
the backoff and attempt caps are still bounded per Job. Permission,
schema, invalid-input, explicit cancellation and deterministic provider
failures require explicit user retry. A missing runner or invalid provenance
remains a diagnosed retryable state and never executes under another owner.

Before system suspend, the scheduler stops admitting background work, asks
runners to checkpoint and persists snapshots. The macOS/Linux host adapter
uses fixed `scutil`/`nmcli` sources, Tauri `RunEvent::Resumed` and a bounded
monotonic wake-gap fallback to feed the typed environment seam. Unknown
network evidence is preserved as `unknown` and handled by the Job policy.
After wake, local Jobs resume when their owner and storage are valid; network
Jobs wait while offline, metered or unknown according to policy. Wake recovery
is rate-limited so queue recovery cannot overwhelm the device.

## Consequences

The v0.0.4 alpha host slice implements the shared lifecycle seam: JobSystem owns
snapshots and transitions, the persistence adapter writes them under the
application data directory, and projections travel through CoreTransport.
Queued/running work maps to retryable `Interrupted` state after a restart. The
current slice now also proves runner registration, cooperative cancellation,
panic isolation, persisted result payloads and provenance projection. P0 Job
governance now adds a bounded `JobExecutionPolicy` (monotonic deadline,
cancellation grace, retry cap/backoff, network class and background flag),
typed `Retrying`/`Cancelling`/`Paused` states, schema-versioned checkpoints
with a 256 KiB payload limit, and a typed suspend/network environment seam.
`Transient` and deadline failures alone enter automatic retry; cancellation,
invalid input, permission and deterministic failures remain explicit user
actions. Attempt ids reject late progress/results, and a retry increments the
attempt before the next runner starts. The same policy is projected through
Rust protocol, Tauri wire and TypeScript without giving the renderer a second
scheduler. Product thumbnail, indexing, feed-sync and facet-specific runners
remain consumers of the same contract. Capability owners can now opt into
exact provenance reuse without creating a second cache or persistence owner.

Positive:

- All long-running work has consistent cancellation and progress behavior.
- Plugin-produced data can be recomputed and diagnosed.
- The UI does not need format-specific background task code.

Costs:

- Job schemas and cache policies must be versioned.
- Runtime memory/process usage telemetry and measured platform metering
  confidence remain release qualification seams; the current claim fields and
  bounded soft pressure are host-enforced, and macOS/Linux signal delivery
  uses fixed adapters plus the typed environment command.

## Open questions

- Which jobs may run concurrently per asset and per device?
- How are partial results exposed to inspectors?
- How long are failed artifacts retained for diagnosis?

## Proposed hardening clarification (2026-11-02)

This ADR remains `Proposed`. `JobSystem` continues to be the sole owner of Job
state, checkpoints, budgets, provenance, retry/cancel transitions and derived
result reuse. The renderer, Tauri adapter and extension runtime cannot create a
second Job record or scheduler.

The existing `JobExecutionEnvironment` is extended only through bounded,
versioned optional facts. Storage availability may be writable, read-only,
full, unavailable or unknown; power/thermal pressure is advisory. Suspend and
network state keep their current typed values. Unknown storage or pressure is
conservative for background work, while foreground and recovery actions remain
available. A missing removable/network source stays an Asset/source
`SourceAvailability::degraded` condition rather than becoming a global Job
failure.

Deadlines, retry backoff, cancellation grace and wake-gap handling use a
monotonic clock. Wall-clock changes affect only release/freshness diagnostics.
An endpoint or proxy failure is a typed retryable owner result, not evidence
that a default route is usable. Long suspend first pauses admission and writes a
bounded checkpoint when the runner supports it; after wake a new attempt must
revalidate storage, source identity, network policy and owner/runtime
availability. An unsupported checkpoint never makes an `Interrupted` Job
silently resumable.

Job-side destructive effects use ADR-0014's `OperationPlan` and
`OperationReceipt`; `requestId` is not universal external idempotency, and an
unknown side effect is queried/reconciled before retry. Derived results and
caches remain recomputable and are excluded from portable user-data bundles.

The first implementation slice now carries the versioned storage and power
facts through Rust `JobSystem`, `CoreTransport`, Tauri wire errors and desktop
types. Background admission pauses conservatively for non-writable/unknown
storage and critical/unknown power pressure; the native adapter still emits
only the qualified network/suspend observations, so real storage/battery/
thermal probing remains a platform qualification task.
