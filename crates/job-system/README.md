# job-system crate

## Responsibility

Rust-owned Job lifecycle, runner registration, cancellation, retry, deadline, budget, checkpoint, provenance and state-store seam.

## Non-goals

It does not own plugin runtime processes, UI progress state, filesystem paths or format-specific background loops.

## Stability

JobSystem, JobSpec, JobSnapshot, JobRunner, JobExecutionContext and JobStateStore are the public seam. Internal scheduling and cancellation details may move behind the facade.

## Inputs and outputs

Inputs are JobSpec, runner, execution policy, budget request and execution environment. Outputs are JobSnapshot, JobResult, JobError, progress events and state-store writes.

## State ownership

JobSystem owns Job lifecycle and in-memory state. A JobStateStore persists snapshots; callers and plugins receive handles or projections.

## Dependency direction

The crate has only serde and serde_json dependencies. Persistence and ApplicationCore provide adapters around its store seam.

## Public interface

JobId, JobStatus, JobSpec, JobSnapshot, JobRunner, JobExecutionContext, JobSystem, JobStateStore and budget policy types.

## Module map

lifecycle.rs is the private implementation module; budget.rs owns resource accounting. Future model, policy, execution, scheduler, cancellation and store modules remain in this crate.

## Tests

Run cargo test -p velvet-job-system. Tests cover cancellation, retry, deadline, budget, checkpoint, provenance and restart snapshots.

## Failure boundary

Runner panic, timeout, cancellation, budget rejection and store errors become typed Job outcomes. A failed Job cannot terminate ApplicationCore.

## Migration notes

Keep Job schema, checkpoint, environment and cancellation semantics stable. Do not add a second scheduler or move Job state into UI or PluginHost.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
