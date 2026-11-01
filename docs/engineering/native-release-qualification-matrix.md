# Native release qualification matrix

Status: active release gate; observed on 2026-11-01

Current host result: macOS core persistence/ACL/read-only checks are `pass`,
but the overall harness result is `incomplete` because no removable or network
mount was supplied. Linux Docker persistence/ACL/read-only checks are also
`pass` and its overall result is `incomplete` for the same missing mounts;
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
| Supplied removable-volume reachability | `not-configured` unless an actual mount is supplied | `not-configured` unless an actual mount is supplied | `deferred` | The probe requires a mounted filesystem and marker create/remove proves reachability only; mount identity and disconnect/reconnect evidence are recorded by the operator. |
| Supplied network-volume reachability | `not-configured` unless an actual mount is supplied | `not-configured` unless an actual mount is supplied | `deferred` | The probe requires a mounted filesystem. A disconnected source keeps identity and becomes typed unavailable; this application-level policy remains a P1 seam. |
| Physical power interruption | `manual-required` | `manual-required` | `deferred` | Requires a disposable test volume and operator-controlled interruption. The scripts do not claim to reproduce power loss. |

Commands:

- macOS: `./tools/release-qualification/run-macos.sh`
- Linux: `./tools/release-qualification/run-linux-docker.sh`
- Windows deferred record: `pwsh ./tools/release-qualification/run-windows.ps1 -AllowDeferred`

The scripts write untracked evidence under `artifacts/release-qualification/`.
Before a release, attach the generated `result.json`, logs, OS version,
filesystem type, mount identity and whether the run was native or containerized
to the release record. `not-configured`, `not-enforced`, `manual-required` and
`deferred` are incomplete gates and cannot be reported as passes.
