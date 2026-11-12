---
id: ui-bridge
title: UI Bridge
---

# UI Bridge

The Bridge connects an isolated surface to host-owned layout, focus, overlays and lifecycle. It carries semantic messages and read-only context/theme/locale projections; it does not expose host React components.

- [Connect a surface](../guides/isolated-ui): sequence, example and verification.
- [Message reference](/reference/ui-bridge-protocol-v1): fields, versions, errors and disposal.
- [Theme and component contract](/reference/ui-kit-contract-v1): semantic tokens and negotiation.
- [Security boundaries](./security): origin, session and capability responsibilities.

The current contract is an in-repository Alpha. The existence of an adapter does not qualify a production SDK or every native WebView.
