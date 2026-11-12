---
id: ui-bridge-protocol-v1
title: UI Bridge message protocol v1
---

# UI Bridge message protocol v1

Audience: isolated UI authors and host adapter maintainers. Status: **Alpha bridge version `1`, component contract `1`**. Owner: `apps/desktop/src/workbench/plugins/ui-bridge.ts`; theme and overlay DTOs live in the adjacent `ui-kit.ts`. These source types document the seam, not an SDK import path.

## Messages

Every message contains `type`. A question mark marks an optional field. The host adapter owns framing, origin/source validation and session disposal.

{/* bridge-messages:start */}
| type | Fields besides type | Meaning |
| --- | --- | --- |
| `host.ready` | `bridgeVersion, componentContractVersion, surfaceId, theme, size, context; locale?` | Initial host projection and supported versions. |
| `host.context` | `context` | Read-only WorkspaceContext snapshot. |
| `host.theme` | `theme` | Normalized semantic theme snapshot. |
| `host.locale` | `locale` | Bounded locale projection. |
| `host.resize` | `size` | width / height; devicePixelRatio is optional. |
| `host.focus` | `focused` | Host focus state. |
| `host.dispose` | `reason` | unmount, crash or incompatible; release local resources. |
| `host.error` | `code, message` | Bounded error; no general command result is returned. |
| `surface.ready` | `bridgeVersion; componentContractVersion?` | Report a compatible version. |
| `surface.focus-request` | `—` | Ask the host for focus. |
| `surface.command` | `requestId, commandId, args` | Dispatch a registered command through the bound host handler. |
| `surface.overlay-request` | `request: UiOverlayRequest` | Host owns the overlay lifecycle and focus. |
| `surface.notification` | `request: UiOverlayRequest` | Request a host notification. |
| `surface.progress` | `request: UiProgressRequest` | Project progress; this does not create or own a Job. |
| `surface.close` | `—` | Dispose the surface with unmount reason. |
| `surface.error` | `message` | Report a surface failure to the host. |

{/* bridge-messages:end */}

## Requests and errors

`UiOverlayRequest` requires `requestId` and `kind`; optional fields include `title`, `message`, `severity`, `anchor` and `actions`. Exact allowed keys and kind/anchor/action rules are validated by `validateUiOverlayRequest` in the Bridge source. It cannot contain React elements or arbitrary DOM.

`UiProgressRequest` requires `requestId`. Optional `value` is finite and between 0 and 1; `message` is bounded; `state` is `running`, `completed`, `failed` or `cancelled`. A progress message does not bypass Rust Job ownership.

Surface messages reject unknown fields and serialized payloads over 64 KiB. IDs are bounded to 256 characters, message text to 4096, and overlay actions to 32. Malformed data is rejected before a host handler runs; callers must not assume every invalid message receives an error response.

| Error code | Meaning / recovery |
| --- | --- |
| `bridge-version-mismatch` | Incompatible bridge; host fails and disposes the surface |
| `component-contract-version-mismatch` | Unsupported component version; host fails and disposes the surface |
| `command-handler-unavailable` | This surface has no bound command handler |
| `command-failed` | Host handler rejected; do not blindly replay a side effect |
| `overlay-handler-unavailable` / `notification-handler-unavailable` | Requested host adapter is absent |
| `overlay-request-failed` / `notification-request-failed` / `progress-request-failed` | Host request failed; retain bounded diagnostics |

Version 1 does not define a general success acknowledgement, result payload or cancellation message for `surface.command`. Its `requestId` is not an idempotency key. Job cancellation uses the authorized Job seam, not `surface.progress`.

## Lifecycle and security

The host sends `host.ready`; the surface reports `surface.ready`. Later context, theme, locale and size messages are projections. On disposal, unsubscribe and release resources. A host retry creates a new session; old messages must not be replayed.

Browser adapters validate source window and exact network origin when present; opaque origins do not gain ambient authority. Tauri child WebViews use explicit capabilities and cleanup. Keep this adapter boundary separate from runtime sandbox qualification. [Connect a surface](/guides/isolated-ui) explains the workflow; the [UI contract](ui-kit-contract-v1.md) owns semantic theme rules.

## Evidence

`apps/desktop/test/ui-kit-contract.test.mjs` and `apps/desktop/test/plugin-surface-adapter.test.mjs` exercise validation, negotiation, origin and disposal. Use the [test setup](/platform/testing). Native OS/WebView and accessibility qualification are separate [release gates](plugin-ecosystem-qualification.md).
