---
id: contributions
title: Contributions
sidebar_label: Contributions
---

# Contributions

A contribution registers an extension capability with the Workbench, a WorkbenchApp, or a reusable capability namespace. Each contribution declares scope, namespace, target app, container, zone, activation, and required permissions.

- `workbench` owns global shell and cross-app surfaces.
- `workbench-app` owns a WorkbenchApp surface and local containers.
- `capability` owns reusable Source, Preview, Processor, Job, or AssetFacet capabilities.

The host owns containers and layout. The ContributionRegistry validates scope, kind, namespace, zone, container, and permissions before activation.
