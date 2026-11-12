---
id: commands-and-views
title: Add Commands and Views
---

# Add Commands and Views

Start from the runnable [Echo package](https://github.com/OdradekAI/velvet/tree/main/examples/plugins/echo-sidecar). This guide explains its Alpha declaration path, not a standalone SDK generator.

## Declare the contribution

The example's `manifest.json` declares `echo.container` as `view-container` and `echo.view` as `view`. Both target `asset-manager` in `workbench-app` scope and the `navigation` zone. `viewContainerId` connects the View to its container. The View's `ui` field declares its surface entrypoint and `bridgeVersion`.

`echo.open` is a `command`. The `context-menu` contribution `echo.menu` refers to it through `commandId`, with the `asset` context anchor. A menu only exposes an action; the host still checks whether the command can execute. See the [manifest reference](/reference/plugin-manifest-v1) before editing fields.

## Exercise the runtime handler

From the repository root:

```sh
printf '%s\n' '{"protocolVersion":1,"kind":"invoke-command","extensionId":"velvet.example.echo","commandId":"echo.open","args":{"source":"manual"},"context":{"activeAppId":null,"keys":{}}}' | node examples/plugins/echo-sidecar/sidecar.mjs
```

The process emits protocol JSONL for `echo.open` and exits when stdin closes. This checks the sidecar's handler only; it bypasses host admission, permissions and UI mounting. Keep stdout for protocol records and stderr for bounded diagnostics.

## Mounting and cleanup

Host registration and activation determine whether the View appears. A View contributes content inside its declared zone and cannot mutate layout or mount another View's implementation. The host removes contributions after disable, crash or uninstall; recovery creates a new session.

After the [test setup](../platform/testing), run:

```sh
node apps/desktop/test/external-plugin-e2e.test.mjs
node apps/desktop/test/plugin-contribution-lifecycle.test.mjs
```

These tests cover the local host/adapter seams. Package trust and native platform release qualification remain separate. Continue with [isolated UI](./isolated-ui) or the [host registry record](/reference/plugin-registry-v1).
