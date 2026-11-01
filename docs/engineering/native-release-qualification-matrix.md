# Native release qualification matrix

Status: active release gate; observed on 2026-11-01

Current host result: macOS core persistence/ACL/read-only/runtime supervision
checks are `pass`. Linux Docker persistence/ACL/read-only checks are also
`pass`, including runtime supervision. Real removable and network mounts are
explicitly `deferred` for the current foundation slice, so neither platform
claims release qualification for mount disconnect/reconnect continuity;
Windows is `deferred`.

The persistence suite is a deterministic cross-target gate. This matrix adds
host evidence for process interruption and filesystem permissions. A result is
only `pass` when the corresponding script ran on that platform. Cross-target
compilation is evidence that code builds; it does not qualify runtime behavior.

| Scenario | macOS | Linux (Docker) | Windows | Evidence and invariant |
| --- | --- | --- | --- | --- |
| Abrupt process after staged write | `pass` | `pass` | `deferred` | macOS native and Linux Docker runs passed; the prior committed snapshot remains authoritative and staged bytes remain inspectable. |
| RecoverySet pointer publication boundary | `pass` | `pass` | `deferred` | macOS native and Linux Docker runs passed; owner commit cannot publish a mixed or partially referenced recovery set. |
| ACL denied snapshot directory | `pass` | `pass` | `deferred` | Both runs passed with OS-enforced mode bits; Linux Docker ran as the invoking non-root UID. |
| Read-only snapshot directory | `pass` | `pass` | `deferred` | Both runs passed; a failed stage does not create defaults or replace a committed generation. |
| Durability report and unsupported capability | `pass` | `pass` | `deferred` | Both runs passed; the report records effective atomic/file/directory sync behavior and never overstates crash durability. |
| Release wrapper loader/exit evidence | `pass (fixture)` | `pass (fixture)` | `deferred` | `test-launcher-wrapper.sh` runs the actual Rust launcher against missing, failing and successful children; platform package wiring and failure before the wrapper image loads remain release gates. |
| OS crash artifact metadata | `pass (parser fixture)` | `pass (metadata fallback)` | `deferred` | macOS `.crash`/`.ips` header fixtures project bounded process/exception/signal/reason fields; Linux core/minidump files retain path-free hash metadata when no safe parser is available. |
| Confirmed orphan sidecar identity and process-group termination | `pass` | `pass` | `deferred` | Native release scripts run the platform runtime lease test: start-token identity must match before the isolated process group is terminated. Windows Job Object runtime remains deferred. |
| Supplied removable-volume reachability | `deferred` | `deferred` | `deferred` | Deferred by the current foundation decision. Before a release claims removable-volume continuity, run the probe with a real mount, record filesystem identity, disconnect/reconnect it and verify typed unavailable/recovery behavior. |
| Supplied network-volume reachability | `deferred` | `deferred` | `deferred` | Deferred by the current foundation decision. Before a release claims network-volume continuity, run the probe with a real mount, record filesystem identity and verify source identity survives disconnect/reconnect. |
| Physical power interruption | `deferred` | `deferred` | `deferred` | Deferred for the current release qualification cycle. When enabled, it requires a disposable test volume and operator-controlled interruption; the scripts do not claim to reproduce power loss. |

Commands:

- macOS: `./tools/release-qualification/run-macos.sh`
- Linux: `./tools/release-qualification/run-linux-docker.sh`
- Windows deferred record: `pwsh ./tools/release-qualification/run-windows.ps1 -AllowDeferred`

The scripts write untracked evidence under `artifacts/release-qualification/`.
The wrapper fixture is an integration check of the release boundary; it does
not reproduce an OS failure before the wrapper image can be loaded. Such a
failure remains an OS/package-manager evidence gate.
Before a release, attach the generated `result.json`, logs, OS version,
filesystem type, mount identity and whether the run was native or containerized
to the release record. `not-configured`, `not-enforced`, `manual-required` and
`deferred` are incomplete gates and cannot be reported as passes.
