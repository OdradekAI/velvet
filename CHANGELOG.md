# Changelog

All notable changes to Velvet (白翎) are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [0.0.2] - 2026-10-29

### Added

- Added the React/Vite desktop Workbench shell with Velvet UI primitives.
- Added WorkbenchApp, AppSurface, shared context and scoped plugin contribution contracts.
- Added host-owned commands, menus, context expressions, overlays and the versioned UI bridge.
- Added revisioned layout transport with visibility, collapse, bounded resize, ViewContainer ordering and View movement.
- Added keyboard and pointer layout resizing plus workspace context-menu controls for Part visibility and collapse.
- Added the native Extensions manager with manifest and permission preflight, host API compatibility checks, versioned upgrades and transactional rollback.
- Added restart recovery for abandoned plugin staging and rollback directories, with deferred activation and explicit recovery actions.
- Added retryable recovery actions, startup cleanup for orphaned transaction directories, and recent recovery logs in the Extensions manager.
- Added OperationsPanel recovery history with plugin/status/action filters, clear and native JSON export.

### Changed

- Renamed the contextual global region to `AuxiliarySidebar` and standardized Workbench terminology.
- Promoted the canonical active layout field from `activeWorkspace` to `activeApp`, with migration compatibility for older state.
- Rust layout state now validates fixed Part geometry and preserves ViewContainer order through extension removal.

### Notes

- The Tauri IPC serialization and event subscription adapter remains the next integration step; this release contains the framework-neutral protocol and renderer seams.

## [0.0.1] - 2026-10-28

### Added

- Added the Apache License 2.0.
- Added the local-first Rust application core and transport seam for the Tauri client.
- Added the Workbench layout model with fixed navigation, contextual and operations zones.
- Added the `AssetManager` and `FeedWorkbench` workspace descriptors.
- Added the layered preview model with `Quick`, `Inspect` and `Analyze` intents.
- Added preview provider resolution with a generic fallback provider.
- Added the plugin API and initial Plugin Host lifecycle and zone validation.
- Added persistence and cancellable-job interfaces for future durable storage and background work.
- Added the framework-independent desktop TypeScript workbench skeleton.
- Added Rust unit tests and TypeScript type checking for the initial interfaces.

### Notes

- This is an architecture foundation release. The full Tauri window, asset indexing, format decoders and production plugin marketplace are not included yet.
- The Rust core currently uses an in-memory persistence adapter behind a stable interface.
