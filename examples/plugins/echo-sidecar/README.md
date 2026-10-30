# Echo Sidecar Plugin

This is the smallest runnable external Velvet plugin. It demonstrates the
same contracts used by an installed sidecar:

- `manifest.json` declares a sidecar runtime, a ViewContainer, a UI Bridge
  view, two commands, an asset context-menu entry and a capability-scoped
  `PreviewProvider`.
- `sidecar.mjs` keeps stdout reserved for protocol JSONL, echoes command
  arguments/context back to the host and returns a typed preview session for
  `preview.open`.
- `echo.crash` exits the sidecar so the host can isolate and recover the
  extension without taking down the Workbench.

Run the sidecar directly from this directory to inspect its protocol:

```sh
printf '%s\n' '{"protocolVersion":1,"kind":"invoke-command","extensionId":"velvet.example.echo","commandId":"echo.open","args":{"source":"manual"},"context":{"activeAppId":null,"keys":{}}}' | node sidecar.mjs
```

To install it for local discovery, copy both files into the application's
`plugins` data directory. Relative launch arguments beginning with `./` or
`../` are resolved relative to the manifest directory during discovery, so the
sidecar remains relocatable as a package.

The host also exposes the package installation seam as the versioned
`install-plugin` CoreTransport command. It validates and copies the directory
atomically; the native Tauri startup then discovers and starts the registered
extension on the next client launch.
