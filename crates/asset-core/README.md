# asset-core crate

## Responsibility

Stable Asset identity and value objects: AssetId, ContentKindId, ContentFingerprint and AssetRef.

## Non-goals

It does not own source paths, databases, indexing, previews, facets or persistence.

## Stability

High-stability leaf contract. New validation or fields require compatibility review because protocol and persistence consume these types.

## Inputs and outputs

Inputs: Validated or caller-provided identity strings.
Outputs: AssetId, ContentKindId, ContentFingerprint and AssetRef.

## State ownership

No durable state. Asset/source owners live above this crate.

## Dependency direction

No workspace dependencies.

## Public interface

AssetId, ContentKindId, ContentFingerprint and AssetRef.

## Module map

Single small facade is intentional; do not split until Asset domain has a second independent seam.

## Tests

cargo test -p velvet-asset-core; current crate has no behavior-heavy tests.

## Failure boundary

This crate currently has no I/O or runtime failure boundary. Format validation remains a future domain decision.

## Migration notes

Do not add source paths or extension data. A validated constructor must preserve existing wire and tuple-shape compatibility.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
