---
id: migration
title: Migration and compatibility
sidebar_label: Migration
---

# Migration and compatibility

Manifest schema, Host API, Bridge API, Runtime Protocol, Theme Schema, and Component Contract are versioned independently.

## Compatibility rules

- Optional additive fields can land within a major version.
- Required fields, message meaning, state ownership, or keyboard model changes require a major decision.
- Version changes require fixtures or migration evidence.
- Reconnect creates a new session and does not replay old events.
- Extension failure, disable, and uninstall cannot change core Asset, Layout, Preview, or Job authority.

Current versions are listed in the [Manifest reference](/reference/plugin-manifest-v1) and the [UI contract](/reference/ui-kit-contract-v1).
