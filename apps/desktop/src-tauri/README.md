# Tauri adapter boundary

The Rust implementation for this boundary lives in `crates/tauri-adapter`.

When the Tauri application is introduced, command handlers in this directory should remain thin:

1. decode a typed request;
2. call `TauriTransport`;
3. return a typed response or event;
4. never access persistence, preview providers or plugin runtimes directly.

The renderer owns plugin surface placement. Browser and unsupported development
entrypoints use the sandboxed iframe adapter; Tauri HTTP and
`tauri-webview://` entrypoints use the child WebView adapter. The latter is
enabled with Tauri's `unstable` WebView feature and the narrowly scoped
`core:webview` permissions in `capabilities/default.json`; it is closed by the
UI Bridge session during app switching, plugin deactivation or restart
recovery.
