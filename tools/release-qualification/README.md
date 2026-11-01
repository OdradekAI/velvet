# Native release qualification

This harness exercises the persistence boundary on a real host or a versioned
Linux container. It is a release gate for the invariants that can be tested
without simulating a physical power cut:

- an abruptly terminated child leaves the previous committed generation and
  `RecoverySet` pointer authoritative;
- an ACL-denied or read-only snapshot directory returns typed unavailability
  and never overwrites existing evidence;
- the durability report describes the effective atomic and `fsync` behavior;
- an operator-supplied removable or network mount can be checked for
  reachability and write policy.

The volume probe requires the supplied path to resolve to a mounted filesystem;
an ordinary directory is `unavailable`. A `pass` result means that the mount
was reachable and a temporary marker could be created and removed. Record the
actual filesystem and mount identity in the result matrix when qualifying
removable or network media. No path supplied is `not-configured`, not a pass.

Run the native macOS gate from the repository root:

```sh
./tools/release-qualification/run-macos.sh
```

Optional mount probes can be supplied as paths. The paths must be actual
mounted test volumes, and the test operator is responsible for testing the
disconnected state in a second run:

```sh
VELVET_REMOVABLE_VOLUME=/Volumes/VelvetQualification \
VELVET_NETWORK_VOLUME=/Volumes/VelvetNetworkQualification \
./tools/release-qualification/run-macos.sh
```

Run the Linux gate in the versioned `rust:1.85-bookworm` image. The container
runs the test process as the invoking user so Unix mode bits are enforced:

```sh
./tools/release-qualification/run-linux-docker.sh
```

For Linux volume probes, provide host mount points. They are mounted into the
container read/write for the reachability probe and are still recorded as
operator-supplied mounts:

```sh
VELVET_REMOVABLE_VOLUME=/mnt/velvet-removable \
VELVET_NETWORK_VOLUME=/mnt/velvet-network \
./tools/release-qualification/run-linux-docker.sh
```

Windows is intentionally deferred while no Windows environment is available.
The PowerShell entry point records that status without claiming a pass:

```powershell
pwsh ./tools/release-qualification/run-windows.ps1 -AllowDeferred
```

The generated result files are under `artifacts/release-qualification/` and
are local evidence. If Docker or its daemon is unavailable, the Linux script
writes a `blocked` result and exits non-zero; that is an environment gate, not
a Linux pass. If either mount path is not configured,
the script writes `incomplete` and exits non-zero after recording the core
results. The tracked decision and current result matrix are
[`docs/engineering/native-release-qualification-matrix.md`](../../docs/engineering/native-release-qualification-matrix.md).
