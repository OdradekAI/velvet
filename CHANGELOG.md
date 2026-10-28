# Changelog

All notable changes to Velvet (白翎) are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

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
