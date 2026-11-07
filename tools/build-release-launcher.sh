#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
tauri_root="$repo_root/apps/desktop/src-tauri"
target="${VELVET_TARGET_TRIPLE:-}"

if [[ -z "$target" ]]; then
  target="$(rustc -vV | awk '/^host: / {host=$2} END {print host}')"
fi
if [[ -z "$target" ]]; then
  printf 'could not determine the Rust target triple\n' >&2
  exit 2
fi

cargo_target_dir="${CARGO_TARGET_DIR:-$tauri_root/target}"
if [[ "$cargo_target_dir" != /* ]]; then
  cargo_target_dir="$repo_root/$cargo_target_dir"
fi

printf 'Building velvet-launcher for %s\n' "$target"
TAURI_CONFIG='{"bundle":{"externalBin":null}}' cargo build \
  --manifest-path "$tauri_root/Cargo.toml" \
  --bin velvet-launcher \
  --release \
  --target "$target"

extension=""
if [[ "$target" == *-windows-* ]]; then
  extension=".exe"
fi
source="$cargo_target_dir/$target/release/velvet-launcher$extension"
if [[ ! -f "$source" ]]; then
  printf 'launcher build did not produce %s\n' "$source" >&2
  exit 1
fi

destination_dir="$tauri_root/binaries"
destination="$destination_dir/velvet-launcher-$target$extension"
rm -rf "$destination_dir"
mkdir -p "$destination_dir"
cp "$source" "$destination"
if [[ "$extension" != ".exe" ]]; then
  chmod 755 "$destination"
fi
printf 'Prepared Tauri external binary %s\n' "$destination"
