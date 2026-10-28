# Tauri adapter boundary

The Rust implementation for this boundary lives in `crates/tauri-adapter`.

When the Tauri application is introduced, command handlers in this directory should remain thin:

1. decode a typed request;
2. call `TauriTransport`;
3. return a typed response or event;
4. never access persistence, preview providers or plugin runtimes directly.
