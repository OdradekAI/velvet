# preview-core crate

## Responsibility

Preview intent/context, bounded source seam, provider selection, PreviewResolver, sessions and generic fallback.

## Non-goals

It does not own Asset source locations, plugin runtime processes, UI framework components or durable preview caches.

## Stability

PreviewProvider, PreviewAssetSource, PreviewCandidate, PreviewSession and PreviewResolver are the public seam.

## Inputs and outputs

Inputs: PreviewRequest, PreviewContext and a bounded PreviewAssetSource.
Outputs: PreviewIntent, PreviewSurface, PreviewContext, PreviewProvider, PreviewResolver, built-in providers and fallback types.

## State ownership

PreviewResolver owns provider registry and resolution state; source ownership remains in ApplicationCore/source adapters.

## Dependency direction

asset-core and serde.

## Public interface

PreviewIntent, PreviewSurface, PreviewContext, PreviewProvider, PreviewResolver, built-in providers and fallback types.

## Module map

Target modules are intent/context, source, provider, resolver and builtin/fallback.

## Tests

Provider matching, bounded reads, provider failure fallback and surface tests; cargo test -p velvet-preview-core.

## Failure boundary

Provider failure becomes a typed error and may fall back to generic preview; paths and credentials do not cross the seam.

## Migration notes

Keep generic fallback usable while plugin preview contracts remain provisional.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
