#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
output_dir="${VELVET_QUALIFICATION_OUTPUT_DIR:-$repo_root/artifacts/release-qualification/linux-docker-$stamp}"
mkdir -p "$output_dir"
image="velvet-release-qualification:rust-1.85-bookworm"

if ! command -v docker >/dev/null 2>&1; then
  cat >"$output_dir/result.json" <<EOF
{
  "platform": "linux-docker",
  "status": "blocked",
  "reason": "Docker CLI is unavailable; no Linux runtime result is claimed.",
  "evidence": "$output_dir"
}
EOF
  cat "$output_dir/result.json"
  printf 'Evidence: %s\n' "$output_dir"
  exit 2
fi

if ! docker info >/dev/null 2>"$output_dir/docker-info.log"; then
  cat >"$output_dir/result.json" <<EOF
{
  "platform": "linux-docker",
  "status": "blocked",
  "reason": "Docker CLI is installed but the Docker daemon is unavailable; no Linux runtime result is claimed.",
  "evidence": "$output_dir"
}
EOF
  cat "$output_dir/result.json"
  printf 'Evidence: %s\n' "$output_dir"
  exit 2
fi

docker build --quiet -f "$script_dir/Dockerfile.linux" -t "$image" "$script_dir" >"$output_dir/docker-build.log"

docker_args=(
  run --rm
  --user "$(id -u):$(id -g)"
  --env HOME=/tmp
  --env CARGO_HOME=/tmp/cargo
  --volume "$repo_root:/workspace"
  --workdir /workspace
)

if [[ -n "${VELVET_REMOVABLE_VOLUME:-}" && -d "$VELVET_REMOVABLE_VOLUME" ]]; then
  docker_args+=(--volume "$VELVET_REMOVABLE_VOLUME:/qualification/removable:rw")
  docker_args+=(--env VELVET_REMOVABLE_VOLUME=/qualification/removable)
fi
if [[ -n "${VELVET_NETWORK_VOLUME:-}" && -d "$VELVET_NETWORK_VOLUME" ]]; then
  docker_args+=(--volume "$VELVET_NETWORK_VOLUME:/qualification/network:rw")
  docker_args+=(--env VELVET_NETWORK_VOLUME=/qualification/network)
fi

set +e
docker "${docker_args[@]}" "$image" -c '
  set -euo pipefail
  cargo test -p velvet-persistence --test release_qualification -- --test-threads=1 --nocapture
  bash tools/release-qualification/probe-volume.sh "${VELVET_REMOVABLE_VOLUME:-}" removable
  bash tools/release-qualification/probe-volume.sh "${VELVET_NETWORK_VOLUME:-}" network
' >"$output_dir/persistence.log" 2>&1
test_exit=$?
set -e

persistence_status="pass"
[[ "$test_exit" -eq 0 ]] || persistence_status="fail"
acl_status="pass"
if grep -q 'ACL probe skipped' "$output_dir/persistence.log"; then
  acl_status="not-enforced"
fi
readonly_status="pass"
if grep -q 'read-only probe skipped' "$output_dir/persistence.log"; then
  readonly_status="not-enforced"
fi
removable_status="$(awk '$2 == "removable" {print $3; exit}' "$output_dir/persistence.log" || true)"
network_status="$(awk '$2 == "network" {print $3; exit}' "$output_dir/persistence.log" || true)"
[[ -n "$removable_status" ]] || removable_status="not-configured"
[[ -n "$network_status" ]] || network_status="not-configured"

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
  "platform": "linux-docker",
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
