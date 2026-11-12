---
id: overview
title: Velvet Plugin Platform
sidebar_label: Overview
---

# Velvet Plugin Platform

The Plugin Platform is the main extension boundary. Plugins use manifests, contributions, capabilities, and UI surfaces to extend the Workbench. The UI Kit is the platform's UI capability layer for trusted first-party UI and isolated third-party UI surfaces.

## Platform layers

```text
Plugin Platform
├── Contributions     Views, Commands, Menus, Keybindings, Preview, Jobs
├── Capability grants scopes, zones, namespaces, permissions
├── UI surfaces       UI Kit, UI Bridge, Overlays, Host projections
├── Lifecycle         activation, suspension, failure, retry, disposal
├── Transport         typed requests, queries, events, sessions, resync
└── Qualification     contracts, stories, browser and release gates
```

Start with [Quick start](./quick-start). Choose a contribution in [Choose a contribution](./choose-contribution), then continue to [UI Kit](../ui-kit) or [UI Bridge](./ui-bridge).
