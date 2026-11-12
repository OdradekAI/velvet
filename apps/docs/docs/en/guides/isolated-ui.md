---
id: isolated-ui
title: Connect an isolated UI surface
---

# Connect an isolated UI surface

The current repository has iframe and Tauri child-WebView adapters with an Alpha message contract. It does not publish a framework SDK or qualify every native surface. Start with the Echo package's declared `contributions[].ui` and the [protocol reference](/reference/ui-bridge-protocol-v1).

## Surface sequence

1. Declare the View or AppSurface, its scope/zone, entrypoint and bridge version.
2. The host validates the declaration and mounts its adapter. It sends `host.ready` with context, size, theme and supported component version.
3. The surface reports `surface.ready` with its compatible bridge version.
4. Apply `host.context`, `host.theme`, `host.locale` and `host.resize` as projections. They do not grant data access.
5. Request a registered action with `surface.command`. The host handler performs authorization and dispatch.
6. On `host.dispose`, remove listeners and local resources. Retry uses a new host session.

A semantic command message has this shape:

```json
{"type":"surface.command","requestId":"inspect-1","commandId":"echo.open","args":{}}
```

This is a message payload, not a complete `postMessage` bootstrap or a production install recipe. Origin/source validation, transport framing and session ownership belong to the host adapter. Version 1 has no general command-result/ack message; `requestId` does not establish replay or deduplication guarantees.

The Echo `data:` UI is a fixture convenience. A production entrypoint must satisfy the actual origin, resource, CSP and capability policy. A sidecar process provides a crash boundary, not automatically an OS security sandbox. See [security](../platform/security).

## Verification

After [test setup](../platform/testing):

```sh
node apps/desktop/test/ui-kit-contract.test.mjs
node apps/desktop/test/plugin-surface-adapter.test.mjs
```

These fixtures cover message validation, theme/component negotiation, origin handling and disposal. Native WebView and accessibility qualification remain in the [maintainer evidence map](/reference/plugin-contract-matrix).
