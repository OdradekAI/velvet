---
id: security
title: Security
sidebar_label: Security
---

# Security

UI placement does not grant data or system permissions. Effective capabilities are derived from the manifest, trust policy, Profile and Workspace policy, scope, zone, and host grant.

- Plugins do not receive host React components, DOM nodes, Tailwind classes, or source paths.
- The UI Bridge only sends bounded context, semantic theme, locale, size, and lifecycle data.
- Message schema, origin, entrypoint scheme, version, and payload size are validated at the boundary.
- The host owns Overlay, focus restoration, Job state, Asset identity, Layout, and Preview fallback.
- A crash, timeout, disable, uninstall, or incompatible surface is isolated to the extension.
