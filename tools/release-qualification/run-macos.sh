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

set +e
(
  cd "$repo_root"
  cargo test -p velvet-persistence --test release_qualification -- --test-threads=1 --nocapture
) >"$log_path" 2>&1
test_exit=$?
set -e

persistence_status="pass"
[[ "$test_exit" -eq 0 ]] || persistence_status="fail"
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
if [[ "$persistence_status" != "pass" || "$acl_status" == "not-enforced" || "$readonly_status" == "not-enforced" ]]; then
  overall_status="incomplete"
fi
if [[ "$removable_status" != "pass" || "$network_status" != "pass" ]]; then
  overall_status="incomplete"
fi
if [[ "$persistence_status" == "fail" ]]; then
  overall_status="fail"
fi

cat >"$output_dir/result.json" <<EOF
{
  "platform": "macos",
  "status": "$overall_status",
  "persistence": "$persistence_status",
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
[[ "$overall_status" == "pass" ]]
