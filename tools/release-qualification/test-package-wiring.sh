#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
target="${VELVET_TARGET_TRIPLE:-}"
if [[ -z "$target" ]]; then
  target="$(rustc -vV | awk '/^host: / {host=$2} END {print host}')"
fi

bash "$repo_root/tools/build-release-launcher.sh"

extension=""
if [[ "$target" == *-windows-* ]]; then
  extension=".exe"
fi
sidecar="$repo_root/apps/desktop/src-tauri/binaries/velvet-launcher-$target$extension"
test -f "$sidecar"
if [[ -z "$extension" ]]; then
  test -x "$sidecar"
fi

config_path="$repo_root/apps/desktop/src-tauri/tauri.conf.json"
for required in \
  '"default-src": ["'"'self'"'"]' \
  '"style-src": ["'"'self'"'"]' \
  '"style-src-attr": ["'"'unsafe-inline'"'"]' \
  '"script-src": ["'"'self'"'"]' \
  '"object-src": ["'"'none'"'"]' \
  '"dangerousDisableAssetCspModification": false' \
  '"externalBin": ["binaries/velvet-launcher"]'; do
  if ! grep -Fq "$required" "$config_path"; then
    printf 'release package policy is missing %s\n' "$required" >&2
    exit 1
  fi
done

printf 'PACKAGE_WIRING_RESULT pass target=%s sidecar=%s\n' "$target" "$sidecar"
