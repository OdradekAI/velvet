# Native desktop application and Tauri v2 research

Date: 2026-10-31

This note records first-party guidance relevant to Velvet’s native desktop boundary. It separates framework capabilities from application policy: Tauri supplies IPC, windowing, packaging and capability controls, while Velvet still owns durable state, recovery policy, plugin trust, jobs and user-facing diagnostics.

## Findings from native desktop guidance

- **Persist incrementally, restore explicitly.** Microsoft’s Windows App SDK guidance recommends saving important state periodically and after significant actions, then restoring it on startup because desktop apps can be terminated by crashes, power loss, OS updates or external process termination. ([Manage app state effectively](https://learn.microsoft.com/en-us/windows/apps/develop/performance/state-management), [App lifecycle for Windows App SDK desktop apps](https://learn.microsoft.com/en-us/windows/apps/develop/launch/app-lifecycle))
- **Use safe writes and keep recoverable copies.** Apple’s document architecture describes autosaving by writing a new file and moving it into place, protecting against crashes and power failures while preserving older versions. ([Core App Behaviors](https://developer.apple.com/library/archive/documentation/DataManagement/Conceptual/DocBasedAppProgrammingGuideForOSX/StandardBehaviors/StandardBehaviors.html)) A practical extension of this rule is: write to a same-directory temporary file, fsync where required, rename atomically, and quarantine a corrupt document instead of deleting it.
- **Separate UI restoration from durable domain data.** Apple’s state restoration guidance stores enough UI identity to recreate windows and selections, while documents/databases remain the source of truth. ([Core App Behaviors](https://developer.apple.com/library/archive/documentation/DataManagement/Conceptual/DocBasedAppProgrammingGuideForOSX/StandardBehaviors/StandardBehaviors.html)) Velvet should keep Session/Window restoration hints separate from Asset, Workspace and Profile state.
- **Treat crash capture and restart supervision as separate reliability components.** Dropbox describes using an out-of-process Crashpad helper plus a watchdog that records whether termination was expected and detects crashes. ([Crash reporting in desktop Python applications](https://dropbox.tech/application/crash-reporting-in-desktop-python-applications)) A native host should be able to start in safe mode after repeated failures, quarantine the last failing optional component and preserve a usable shell.
- **Gate optional components by health, timeout and resource policy.** A production plugin host should validate manifests before activation, use bounded startup deadlines, supervise child processes, apply exponential backoff, quarantine restart loops, stage upgrades, and retain a last-known-good version. This is consistent with the failure-isolation requirements already recorded in Velvet’s plugin ADR. ([ADR-0005](../adr/0005-plugin-host-runtime-and-permissions.md))
- **Make lifecycle and recovery testable.** Recovery paths must be exercised with forced termination, corrupt state, interrupted migration, unavailable volumes, disconnected network drives, plugin timeouts and process crashes. A successful graceful shutdown test is insufficient for a desktop app.

## Tauri v2 capability map

| Native concern | Tauri v2 capability | Maturity and boundary |
|---|---|---|
| IPC and typed command errors | `invoke` commands, async commands, channels, `Result` serialization | Mature transport primitive. Tauri serializes through JSON/JSON-RPC-like IPC; application code must define stable DTOs, error codes, deadlines, cancellation and compatibility rules. ([Calling Rust from the Frontend](https://v2.tauri.app/develop/calling-rust/), [IPC](https://v2.tauri.app/concept/inter-process-communication/)) |
| Frontend privilege control | Capabilities, permissions, scopes, runtime authority | Mature least-privilege mechanism. Capabilities are per-window/webview; deny wins over allow; scopes constrain paths/commands. It does not protect insecure Rust code or overly broad scopes. ([Security](https://v2.tauri.app/security/), [Capabilities](https://v2.tauri.app/security/capabilities/), [Permissions](https://v2.tauri.app/reference/acl/permission/), [Runtime Authority](https://v2.tauri.app/security/runtime-authority/)) |
| Extra IPC hardening | CSP and Isolation pattern | Available, but requires deliberate configuration and frontend dependency review. Isolation inserts a sandboxed iframe to inspect/encrypt messages before they reach Rust. ([Isolation Pattern](https://v2.tauri.app/concept/inter-process-communication/isolation/)) |
| Window/layout restoration | `tauri-plugin-window-state` | Mature for window size, position, maximized/minimized/visible flags. It does not restore Velvet’s semantic layout, Workspace or extension state; those remain Rust-owned. ([Window State](https://v2.tauri.app/plugin/window-state/)) |
| Small persistent settings | `tauri-plugin-store` | Mature key/value store shared by Rust and the webview, with autosave/manual save. It is not a migration framework, backup system or authoritative domain store. ([Store](https://v2.tauri.app/plugin/store/)) |
| Structured relational persistence | `tauri-plugin-sql` with SQLite | Mature adapter with versioned migrations executed in a transaction; schema ownership, backups, compatibility and recovery policy remain application responsibilities. ([SQL](https://v2.tauri.app/plugin/sql/)) |
| Secrets and keys | `tauri-plugin-stronghold` | Available for encrypted secret storage. It does not decide credential lifecycle, rotation, export or OS-specific recovery policy. ([Stronghold](https://v2.tauri.app/reference/javascript/stronghold/)) |
| Files and directories | `tauri-plugin-fs`, path variables and scopes | Mature scoped access with path-traversal protection and explicit allow/deny paths. Symlink policy, network/removable volume behavior, file identity, atomic writes and permission prompts remain Velvet concerns. ([File System](https://v2.tauri.app/plugin/file-system/)) |
| External links and processes | `shell` plugin, opener support | Mature when scoped. Shell commands and arguments must be allow-listed; open URLs should use a narrow scheme/domain policy. Never expose a generic command executor from a Tauri command. ([Shell](https://v2.tauri.app/plugin/shell/), [Features and Recipes](https://v2.tauri.app/plugin/)) |
| Update transport | `tauri-plugin-updater` | Mature signed update transport. Updates require a public key in the app and a private signing key in release infrastructure; production endpoints use TLS. It supports platform/architecture targets, proxy/timeouts and explicit downgrade comparison in Rust. It is not a complete release rollout system, migration coordinator or semantic rollback policy. ([Updater](https://v2.tauri.app/plugin/updater/)) |
| App/package distribution | Tauri bundler, platform-specific signing/notarization guides | Mature build/bundle tooling for macOS, Windows and Linux formats. Code signing/notarization, installer testing, offline WebView2, architecture matrix and channel policy remain release engineering work. ([Distribute](https://v2.tauri.app/distribute/), [Windows installer](https://v2.tauri.app/distribute/windows-installer/), [macOS notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)) |
| Single instance and activation | `single-instance`, `deep-link`, `cli` plugins | Mature primitives with platform-specific caveats. Linux Snap/Flatpak DBus behavior and forwarding arguments must be tested. Deep-link payloads still need application validation. ([Single Instance](https://v2.tauri.app/plugin/single-instance/), [Deep Linking](https://v2.tauri.app/plugin/deep-linking/)) |
| OS integration | official plugins for dialog, clipboard, notification, global shortcut, autostart, positioner, opener, process and system tray | Broad coverage, but each permission is opt-in and behavior differs by OS/package format. Keep a compatibility matrix and graceful fallback for missing capabilities. ([Official plugin list](https://v2.tauri.app/plugin/)) |
| Logging | `tauri-plugin-log` | Mature file/stdout/webview logging. It is not a crash dump pipeline, privacy redaction policy or diagnostic bundle format. ([Logging](https://v2.tauri.app/plugin/logging/)) |
| Crash reporting and watchdog | No first-party Tauri crash reporter | Use an external crash/minidump service or a separately supervised helper, with consent, redaction and offline queueing. The community ecosystem lists `sentry-tauri`, but it is not a core Tauri guarantee. ([Tauri ecosystem](https://github.com/tauri-apps/awesome-tauri)) |
| Durable jobs | async commands, channels, shell sidecars | Tauri provides execution/streaming primitives, not a durable JobSystem with persisted state, retries, quotas, restart recovery, provenance or backpressure. Velvet’s Rust `JobSystem` must own these semantics. ([Calling Rust from the Frontend](https://v2.tauri.app/develop/calling-rust/)) |
| Untrusted product extensions | Tauri plugin lifecycle and permissions | Tauri plugins are host-integrated Rust/JS capabilities; they are not a complete marketplace, package-signing, revocation or out-of-process sandbox for third-party product extensions. Velvet’s WASM/sidecar `PluginHost` remains the correct isolation boundary. ([Plugin Development](https://v2.tauri.app/develop/plugins/), [ADR-0005](../adr/0005-plugin-host-runtime-and-permissions.md)) |

## What Velvet already covers

The current ADRs and foundation plan already encode several strong desktop rules:

- **Single state owner and recovery:** Rust owns Profile, Workspace, Session/Window, Job and extension state; persistence is an adapter; corrupt stores fall back to safe defaults with diagnostics. ([ADR-0001](../adr/0001-system-topology-and-core-ownership.md), [Workbench foundation plan](../engineering/workbench-foundation-plan.md))
- **Versioned transport:** the protocol envelope carries protocol/features, session/request ids, typed payloads, revision/sequence, deadlines, cancellation and typed errors; stale writes and sequence gaps have explicit behavior. ([ADR-0006](../adr/0006-host-protocol-and-ui-sdk.md))
- **Extension failure isolation:** WASM/sidecar runtimes, lazy activation, health checks, timeout handling, disabled/failed states, transaction staging and rollback are host-owned. ([ADR-0005](../adr/0005-plugin-host-runtime-and-permissions.md))
- **Long-running work:** the Rust `JobSystem` owns progress, cancellation, retry, provenance, restart recovery and cache invalidation. ([ADR-0007](../adr/0007-jobs-indexing-and-cache-invalidation.md))
- **UI fallback and accessibility ownership:** PreviewHost, overlays, focus restoration, loading/error/fallback states and localization are host concerns rather than plugin-owned shell behavior. ([ADR-0004](../adr/0004-preview-provider-and-preview-intent.md), [ADR-0011](../adr/0011-contribution-scopes-and-host-containers.md))

## Gaps to make explicit before calling the native boundary production-ready

This checklist records the requirements that informed ADR-0013. The lifecycle,
recovery and acceptance decisions below are now resolved there; remaining
items are implementation and release-qualification work tracked by the
foundation plan. The post-decision review is recorded in
[`native-lifecycle-recovery-review.md`](native-lifecycle-recovery-review.md);
it narrowed fixed resource constants, separated crash capture from lifecycle
supervision, added coordinated `RecoverySet` commits, and made WebView-native
recovery and release freshness explicit.

1. **Application startup policy:** define preflight phases, safe mode, last-known-good configuration, startup deadlines, crash-loop detection and a minimal recovery window that can launch with all optional extensions disabled.
2. **Crash and diagnostics pipeline:** define minidump/crash capture, privacy redaction, consent, offline queueing, user-visible recovery actions and an exportable diagnostic bundle. The log plugin alone is not enough.
3. **Data protection lifecycle:** define backup cadence, atomic-write/fsync rules, corrupt-file quarantine, migration checkpoints, rollback, restore-from-backup, export and deletion semantics. SQL transaction rollback does not replace backup policy.
4. **Extension trust:** application artifact signing is now fixed by ADR-0013;
   extension publisher trust, key rotation, revocation, downgrade protection,
   provenance/SBOM and the explicit developer-mode policy for local installs
   remain a separate `PluginHost` contract.
5. **Resource governance:** add per-plugin and per-job CPU, memory, disk, file descriptor, child-process and concurrency budgets, plus backpressure and timeout policy. A process boundary without quotas still permits denial of service.
6. **OS and WebView support matrix:** record supported OS versions, WebView2/WebKitGTK/WebKit requirements, CPU architectures, GPU/driver constraints, multi-monitor/high-DPI behavior, sleep/wake and offline/proxy behavior. Tauri delegates WebView patching to the OS, which is a security tradeoff and a compatibility obligation. ([Tauri Security](https://v2.tauri.app/security/))
7. **Multi-instance and file coordination:** define lock ownership, concurrent profile/workspace access, file-change reconciliation, removable/network volume disconnection and symlink behavior. Single-instance is a policy choice, not a complete data consistency strategy.
8. **Release operations:** define channels, staged rollout, minimum supported
   version, forced-update policy, update key rotation, failed-install recovery,
   offline installers and uninstall/data-retention behavior. ADR-0013 now fixes
   `stable`/`preview`, signed manifests and paired package/data recovery;
   Tauri’s updater still does not operate the rollout service.
9. **Privacy and retention:** classify logs, crash data, plugin data, caches and user assets; define redaction, consent, retention windows and a user-facing “export/delete my data” path.
10. **Failure-injection validation:** make forced process termination, corrupt state, interrupted migration, unavailable paths, invalid signatures, WebView mismatch, plugin hangs and update interruption part of CI or release qualification.
11. **Native interaction contract:** specify file associations, drag/drop, clipboard formats, notifications, shortcuts, deep links, tray behavior, IME, accessibility tree, RTL and reduced-motion behavior per OS. These are product contracts, not just plugin checkboxes.
12. **Time and power semantics:** use monotonic clocks for deadlines/retries, handle wall-clock changes, sleep/wake and battery/thermal throttling, and define whether background jobs may continue on metered or battery-powered devices.

## Suggested next decisions for Velvet

- Use the accepted native lifecycle/recovery ADR and its P0/P1 acceptance
  matrix as the release-qualification gate for preflight, Safe Mode,
  recovery-supervision ownership and startup UX.
- Keep `RecoverySupervisor` as the lifecycle journal/policy boundary and treat
  `CrashCaptureAdapter` as a separate optional diagnostic seam; do not reintroduce
  a mandatory watchdog/helper before measurements prove the need.
- Keep extension trust in the `PluginHost` contract; ADR-0013 now records
  application signing, key rotation, channels, staged rollout and update/data
  recovery without treating the updater as a migration coordinator.
- Extend the persistence acceptance scenarios with backup/restore, interrupted migration, corrupt-file quarantine and user initiated data deletion.
- Extend JobSystem acceptance with per-owner resource budgets, persisted queue recovery and sleep/network policy.
- Add a platform matrix and a failure-injection test matrix to the foundation completion checklist.
- Add a versioned resource policy, coordinated `RecoverySet` fault tests,
  Tauri capability-negative tests, crash-evidence fixtures and cold-start/idle
  lifecycle baselines before production qualification.
- Apply the plugin-focused [lifecycle and recovery review](plugin-ecosystem-lifecycle-and-recovery-review.md):
  keep sidecars as availability isolation only, add host-orchestrated
  extension storage/backup/migration participants, and keep package trust
  separate from paid entitlement.
