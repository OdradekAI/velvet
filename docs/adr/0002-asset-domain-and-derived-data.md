# ADR-0002: Asset domain and derived data

Status: Proposed
Date: 2026-10-28

## Context

The product handles images, RAW files, video, audio, documents and 3D models. A file may move, be renamed, appear through multiple sources or acquire data from several extensions. Preview results, EXIF, histograms, AI labels and thumbnails are derived data and must not replace the asset's identity.

## Proposed decision

Use a stable `AssetId` as the identity used by the workbench. Track source locations separately from content identity:

```text
Asset
├── AssetId
├── source locations
├── content fingerprint
├── ContentKind
├── core metadata
└── lifecycle state
```

Extension-produced information is stored as namespaced `AssetFacet` records:

```text
AssetFacet {
    asset_id,
    extension_id,
    schema_version,
    payload,
    provenance,
    status,
    updated_at
}
```

Core metadata, user edits and extension facets use different provenance and migration rules. A disabled or removed extension must not make the asset inaccessible.

`Workspace`, `Collection`, `AssetFacet` and `Job` are distinct domain concepts. A workspace stores layout and open items; a collection stores user organization; a facet stores derived or extension-owned information; a job stores asynchronous work and its result state.

## Alternatives considered

1. Use the absolute file path as `AssetId`. Rejected because rename, move, duplicate and removable-source workflows would break identity.
2. Let each extension own a private database. Rejected because uninstall, backup, migration and cross-extension search would be unreliable.
3. Store every extension result in the core asset row. Rejected because extension schemas and lifecycles would become coupled to core migrations.

## Consequences

Positive:

- Extension data can be recomputed, migrated or discarded independently.
- Search and inspector features can query a common facet store.
- The same asset can be shown in both built-in workspaces.

Costs:

- Identity reconciliation needs explicit rules for duplicates and external sources.
- Facet schemas require versioning and stale-state handling.

## Current implementation note

The v0.0.4 foundation now persists application-owned source registrations in a
versioned JSON adapter. Each registration keeps an opaque `AssetId`, a
canonical regular-file location, a declared `ContentKind` and an optional
source-owned content fingerprint. `application-core` owns registration,
removal and recovery behavior; `persistence` only reads and writes the
versioned records. Preview providers and the renderer receive bounded content
or typed projections, never the stored path. Asset reconciliation, move
tracking, duplicate identity and fingerprint policy remain open questions for
the Asset domain decision.

## Open questions

- Is the content fingerprint a full hash, a partial hash or a tiered fingerprint?
- How are mutable assets and version history represented?
- Which facets are exportable, searchable or private to an extension?
