# Changelog

All notable changes to Velvet (白翎) are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Organized documentation into plugin development, API/protocol, trusted UI and maintainer paths, with a runnable first fixture and local search.
- Corrected bilingual Manifest and transport references and added translation review status, contract consistency checks and a documentation PR gate.
- Completed all Chinese reference translations and reviewed every bilingual page against source contracts; documentation releases now require complete review and matching code examples, API types and defaults.

## [0.0.6] - 2026-11-07

### Added

- Added the Alpha plugin platform contract across manifest validation, lifecycle, capabilities, host registries, typed errors, cancellation and compatibility checks.
- Added host-owned command, menu, keybinding, diagnostics, notification, confirmation and Job seams with failure isolation and disable cleanup.
- Added the framework-neutral UI Kit contract, theme/token negotiation, plugin surface lifecycle and Asset Inspector demo fixture.
- Added evidence-backed Eagle populated-library qualification for AssetGrid zoom geometry, selection modifiers, filtering, search and focus observations.
- Added independent AssetGrid contract, lifecycle stories, generated catalog metadata and browser host qualification for range selection.
- Added the ecosystem qualification matrix that separates repository fixtures from future external SDK gates.
- Added the 0.0.6 plugin platform audit with explicit runtime, UI Bridge, revocation and manifest-discovery follow-ups.

### Changed

- Moved native shortcut event dispatch into the Workbench command seam while keeping Tauri event listening in the platform adapter.
- Added diagnostics and platform dependency checks to the desktop test gate.
- Changed Desktop CI to run the full matrix for pull requests, version tags and manual dispatches; ordinary pushes to `main` no longer start a duplicate run.
- Closed the strict Clippy gate for the current protocol dispatch envelope with a documented local exception.

## [0.0.5] - 2026-11-04

### Added

- Added native foundation recovery, persistence, protocol and operation-safety hardening across the Rust core and Tauri boundary.
- Added signed plugin package verification, persistent trust evidence, Profile-scoped developer mode and overlap-signed publisher key rotation.
- Added protected publisher trust provisioning with a pinned publisher root, expiry, monotonic sequence and atomic verified storage.

### Changed

- Updated the desktop, Tauri and Rust workspace versions to `0.0.5`.
- Kept release publication and installer qualification as a later `0.1.0` milestone.

## [0.0.4] - 2026-10-31

### Added

- Added the host-managed Preview execution path with built-in bounded text/image renderers, plugin providers, augmentations, actions and JobSystem handoff.
- Added versioned persistent Preview source locations that resolve opaque `AssetId` values to canonical regular files across restarts without exposing paths to providers or the renderer.
- Added Rust-owned profile locale persistence and plugin localization resource loading through CoreTransport.
- Added bounded ZIP plugin package extraction and native directory/ZIP package selection.
- Added exact Job provenance reuse, persisted result payloads, runner registration and plugin-job failure isolation.

### Changed

- Updated the desktop, Tauri and Rust workspace versions to `0.0.4`.
- Kept Preview source fingerprints under trusted Rust source ownership so renderer requests cannot forge cache provenance.

## [0.0.3] - 2026-10-30

### Added

- Added the external plugin runtime path with manifest discovery, isolated sidecar startup, command invocation, contribution projection and crash recovery.
- Added the native plugin manager flow for install, enable, disable, uninstall, permission preflight, host API compatibility checks, upgrades and transactional rollback.
- Added startup recovery for interrupted plugin installs, orphan cleanup, retryable recovery actions and OperationsPanel recovery log filtering, clearing and export.
- Added the Rust-owned JobSystem lifecycle through CoreTransport and Tauri, including task creation, progress events, cancellation, retry, failure state and restart recovery.
- Added application-data persistence for job snapshots and recovery of queued or running jobs as retryable `Interrupted` jobs after restart.
- Added real Tauri acceptance coverage for plugin startup, task progress, failure, retry, cancellation and crash/restart recovery.

### Changed

- Updated the desktop, Tauri and Rust workspace versions to `0.0.3`.
- Updated the release documentation to describe the current native Workbench foundation and its remaining business-workspace scope.

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
