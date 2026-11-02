#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'run-macos.sh must run on macOS (detected %s)\n' "$(uname -s)" >&2
  exit 2
fi

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
output_dir="${VELVET_QUALIFICATION_OUTPUT_DIR:-$repo_root/artifacts/release-qualification/macos-$stamp}"
mkdir -p "$output_dir"
log_path="$output_dir/persistence.log"
runtime_log_path="$output_dir/runtime-supervision.log"
launcher_log_path="$output_dir/launcher-wrapper.log"
package_log_path="$output_dir/package-wiring.log"

set +e
(
  cd "$repo_root"
  cargo test -p velvet-persistence --test release_qualification -- --test-threads=1 --nocapture
) >"$log_path" 2>&1
test_exit=$?
set -e

persistence_status="pass"
[[ "$test_exit" -eq 0 ]] || persistence_status="fail"

set +e
(
  cd "$repo_root"
  cargo test -p velvet-plugin-host --lib 'runtime_lease::tests::native_reconciler_terminates_confirmed_orphan_process_group' -- --nocapture
) >"$runtime_log_path" 2>&1
runtime_exit=$?
set -e
runtime_status="pass"
[[ "$runtime_exit" -eq 0 ]] || runtime_status="fail"

set +e
(
  cd "$repo_root"
  bash tools/release-qualification/test-launcher-wrapper.sh
) >"$launcher_log_path" 2>&1
launcher_exit=$?
set -e
launcher_status="pass"
[[ "$launcher_exit" -eq 0 ]] || launcher_status="fail"

set +e
(
  cd "$repo_root"
  bash tools/release-qualification/test-package-wiring.sh
) >"$package_log_path" 2>&1
package_exit=$?
set -e
package_status="pass"
[[ "$package_exit" -eq 0 ]] || package_status="fail"

acl_status="pass"
if grep -q 'ACL probe skipped' "$log_path"; then
  acl_status="not-enforced"
fi
readonly_status="pass"
if grep -q 'read-only probe skipped' "$log_path"; then
  readonly_status="not-enforced"
fi

probe_volume() {
  local path="$1"
  local label="$2"
  local result
  result="$($script_dir/probe-volume.sh "$path" "$label")"
  printf '%s\n' "$result" >>"$output_dir/volumes.log"
  printf '%s\n' "$result" | awk '{print $3}'
}

removable_status="$(probe_volume "${VELVET_REMOVABLE_VOLUME:-}" removable)"
network_status="$(probe_volume "${VELVET_NETWORK_VOLUME:-}" network)"

overall_status="pass"
if [[ "$persistence_status" != "pass" || "$runtime_status" != "pass" || "$launcher_status" != "pass" || "$package_status" != "pass" || "$acl_status" == "not-enforced" || "$readonly_status" == "not-enforced" ]]; then
  overall_status="incomplete"
fi
if [[ "$removable_status" != "pass" || "$network_status" != "pass" ]]; then
  overall_status="incomplete"
fi
if [[ "$persistence_status" == "fail" ]]; then
  overall_status="fail"
fi
if [[ "$runtime_status" == "fail" ]]; then
  overall_status="fail"
fi
if [[ "$launcher_status" == "fail" ]]; then
  overall_status="fail"
fi
if [[ "$package_status" == "fail" ]]; then
  overall_status="fail"
fi

cat >"$output_dir/result.json" <<EOF
{
  "platform": "macos",
  "status": "$overall_status",
  "persistence": "$persistence_status",
  "runtimeSupervision": "$runtime_status",
  "launcherWrapper": "$launcher_status",
  "packageWiring": "$package_status",
  "acl": "$acl_status",
  "readonly": "$readonly_status",
  "removableVolume": "$removable_status",
  "networkVolume": "$network_status",
  "evidence": "$output_dir"
}
EOF

cat "$output_dir/result.json"
printf 'Evidence: %s\n' "$output_dir"
if [[ "$test_exit" -ne 0 ]]; then
  exit "$test_exit"
fi
if [[ "$runtime_exit" -ne 0 ]]; then
  exit "$runtime_exit"
fi
[[ "$overall_status" == "pass" ]]
