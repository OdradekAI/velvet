---
id: contributions
title: Contributions
sidebar_label: Contributions
---

# Contributions

A contribution declares an extension capability for the Workbench, a WorkbenchApp, or a reusable capability namespace. Each descriptor requires `id`, `kind`, and `scope`. Fields such as `namespace`, `container`, and `zone` are optional in the serialized shape; the host validates whether they are required or allowed for the chosen kind and scope. The authoritative namespace is derived by the host.

## Scopes

- `workbench` covers global shell and cross-app contributions.
- `workbench-app` targets a specified WorkbenchApp and its compatible surfaces.
- `capability` covers reusable capabilities such as Preview providers and Jobs.

Scope describes where a contribution applies. It does not transfer layout, core data or permission ownership to the extension. Planned Source or Processor abstractions are not additional serialized contribution kinds in schema 1.

## Host relationship

The extension requests a placement; the host owns containers and layout. Manifest-level `activationEvents`, `requiredCapabilities`, `optionalCapabilities`, and `permissionSets` describe activation and permission requests. The host validates kind, scope, namespace, zone, container and effective grants before activation. A failed extension loses its own projections while unrelated extensions remain usable.

See the [Manifest reference](/reference/plugin-manifest-v1) for fields and the [Contract matrix](/reference/plugin-contract-matrix) for cross-layer evidence.
