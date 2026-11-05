# plugin-api crate

## Responsibility

External extension contract: manifest, contribution descriptors, permissions, Preview/UI descriptors and install projections.

## Non-goals

It does not discover, install, activate, persist or execute extensions.

## Stability

Potential public SDK seam. MANIFEST_SCHEMA_VERSION and public serde shapes require fixture and compatibility review.

## Inputs and outputs

Inputs: Manifest JSON and host/version declarations.
Outputs: ExtensionManifest, ManifestValidationError, ExtensionId, VersionRange, ActivationEvent, Permission, contribution descriptors and install/recovery DTOs.

## State ownership

No durable state. PluginHost owns manifest provenance, lifecycle and extension data.

## Dependency direction

layout-model plus serde/serde_json.

## Public interface

ExtensionManifest, ManifestValidationError, ExtensionId, VersionRange, ActivationEvent, Permission, contribution descriptors and install/recovery DTOs.

## Module map

Target modules are version, manifest, contribution, permissions, preview, ui and install; the facade remains the stable export point.

## Tests

Manifest schema, unknown-field, identifier/reference validation, version and descriptor round-trip tests; cargo test -p velvet-plugin-api.

## Failure boundary

Decode and local validation errors are typed. Runtime, trust and storage failures belong to PluginHost.

## Migration notes

Do not treat RuntimeKind or Proposed plugin ADRs as proof of a production runtime. Keep unknown required fields fail-closed. See [plugin manifest schema 1](../../docs/engineering/plugin-manifest-v1.md).

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
