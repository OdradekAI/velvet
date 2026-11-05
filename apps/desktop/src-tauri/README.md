# velvet-desktop

## Responsibility

Standalone Tauri workspace and desktop composition root. It owns native startup, wire mapping, capability policy, recovery, activation and update assembly.

## Non-goals

It is not the owner of domain persistence, preview selection, plugin runtime internals or renderer business state.

## Stability

The run entrypoint and typed wire/command seam are stable native integration points. This workspace is intentionally independent from the root Cargo workspace.

## Inputs and outputs

Inputs are Tauri commands/events, native platform evidence and typed requests for TauriTransport. Outputs are typed wire responses/events and native lifecycle evidence.

## State ownership

Native startup/recovery/update state belongs to the desktop host. Product durable state remains with ApplicationCore and its domain owners.

## Dependency direction

The crate depends on all required root Velvet crates and Tauri plugins. Command handlers call TauriTransport and do not call persistence, preview or PluginHost internals.

## Public interface

run, the velvet-desktop binary, velvet-launcher and thin typed command handlers.

## Module map

crash_capture, instance, capability_policy, launcher_failure, native_recovery_window, platform_probe, release_policy and update_coordinator are owner-named modules. The root lib remains a composition root; future wire, commands, startup, recovery, activation and update extraction must remain internal.

## Tests

Run cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml separately from the root workspace. Native lifecycle and capability fixtures live in this workspace.

## Failure boundary

Native startup, capability and update failures are bounded and diagnostic. A command failure does not bypass CoreTransport or corrupt ApplicationCore state.

## Migration notes

Preserve command names, wire JSON, capability policy and launcher/recovery files. Do not add generic common, shared or utils buckets.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
