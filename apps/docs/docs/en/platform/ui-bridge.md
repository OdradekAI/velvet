---
id: ui-bridge
title: UI Bridge
sidebar_label: UI Bridge
---

# UI Bridge

The UI Bridge is the framework-neutral boundary between an isolated plugin surface and the host. Plugins own rendering. The host owns layout, focus, overlays, permissions, and lifecycle.

Host-to-surface messages include `host.ready`, `host.context`, `host.theme`, `host.locale`, `host.resize`, `host.focus`, `host.dispose`, and `host.error`.

Surface-to-host messages include `surface.ready`, `surface.focus-request`, `surface.command`, `surface.overlay-request`, `surface.notification`, `surface.progress`, `surface.close`, and `surface.error`.

Version, origin, entrypoint, token, and failure rules are defined in the [UI Kit contract](/reference/ui-kit-contract-v1).
