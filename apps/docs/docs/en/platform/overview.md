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

## Choose a reading path

- For your first plugin, start with [Quick start](./quick-start).
- To contribute a View, Command or Job, read [Contributions](./contributions).
- To render UI, [choose a contribution](./choose-contribution), then use the trusted [UI Kit](../ui-kit) or isolated [UI Bridge](./ui-bridge).
- For enforcement boundaries, read [Security](./security).
- For host component props, states and interactions, see [UI Kit components](../ui-kit/components/button).
