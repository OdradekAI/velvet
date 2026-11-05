# persistence crate

## Responsibility

Storage adapters for bounded JSON/file/in-memory snapshots, RecoverySet, durability, journals, owner stores, releases and quarantine.

## Non-goals

It does not interpret domain semantics, own layout/Job/Profile/Plugin state or expose raw paths to extensions.

## Stability

StateSnapshotStore, RecoverySetStore, owner store traits and durability types are the adapter seam.

## Inputs and outputs

Inputs: Owner records, snapshots, store commands and durability settings.
Outputs: Snapshot, recovery, durability, journal, operation, release and quarantine stores/adapters.

## State ownership

Each domain owner remains authoritative; persistence owns storage mechanics and generations only.

## Dependency direction

job-system, layout-model and a compatibility dependency on workbench-protocol.

## Public interface

Snapshot, recovery, durability, journal, operation, release and quarantine stores/adapters.

## Module map

Existing owner-named modules remain; root implementation should continue moving toward journal, owner_records, recovery_set, release, quarantine and durability modules.

## Tests

Atomic stage/commit, checksum, corruption, RecoverySet and release qualification tests; cargo test -p velvet-persistence.

## Failure boundary

Read-only, corruption, partial commit and checksum failures preserve prior generations and never silently overwrite authority.

## Migration notes

The protocol dependency is a known compatibility edge. Remove it only through owner-specific record conversion and an ADR/fixture, not in a facade move.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
