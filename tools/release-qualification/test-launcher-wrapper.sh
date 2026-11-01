#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/velvet-launcher-qualification.XXXXXX")"
trap 'rm -rf "$tmp_root"' EXIT

launcher_manifest="$repo_root/apps/desktop/src-tauri/Cargo.toml"

failure_child="$tmp_root/failing-child.sh"
cat >"$failure_child" <<'EOF'
#!/usr/bin/env sh
printf '%012800d' 0 >&2
exit 23
EOF
chmod 700 "$failure_child"

failure_data="$tmp_root/failure-data"
set +e
VELVET_APP_DATA_DIR="$failure_data" \
VELVET_APP_BINARY="$failure_child" \
cargo run --quiet --manifest-path "$launcher_manifest" --bin velvet-launcher -- \
  >"$tmp_root/failure.stdout" 2>"$tmp_root/failure.stderr"
failure_exit=$?
set -e
if [[ "$failure_exit" -ne 23 ]]; then
  printf 'launcher qualification failed: expected child exit 23, got %s\n' "$failure_exit" >&2
  exit 1
fi
grep -q '"phase": "exit"' "$failure_data/launcher-failure.json"
test -f "$failure_data/launcher-attempt.json"

missing_data="$tmp_root/missing-data"
set +e
VELVET_APP_DATA_DIR="$missing_data" \
VELVET_APP_BINARY="$tmp_root/does-not-exist" \
cargo run --quiet --manifest-path "$launcher_manifest" --bin velvet-launcher -- \
  >"$tmp_root/missing.stdout" 2>"$tmp_root/missing.stderr"
missing_exit=$?
set -e
if [[ "$missing_exit" -ne 1 ]]; then
  printf 'launcher qualification failed: expected loader exit 1, got %s\n' "$missing_exit" >&2
  exit 1
fi
grep -q '"phase": "loader"' "$missing_data/launcher-failure.json"
test -f "$missing_data/launcher-attempt.json"

success_child="$tmp_root/successful-child.sh"
cat >"$success_child" <<'EOF'
#!/usr/bin/env sh
exit 0
EOF
chmod 700 "$success_child"
success_data="$tmp_root/success-data"
VELVET_APP_DATA_DIR="$success_data" \
VELVET_APP_BINARY="$success_child" \
cargo run --quiet --manifest-path "$launcher_manifest" --bin velvet-launcher -- \
  >"$tmp_root/success.stdout" 2>"$tmp_root/success.stderr"
test ! -e "$success_data/launcher-attempt.json"

printf 'LAUNCHER_WRAPPER_RESULT pass\n'
