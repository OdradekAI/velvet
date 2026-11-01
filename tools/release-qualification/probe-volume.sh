#!/usr/bin/env bash
set -euo pipefail

path="${1:-}"
label="${2:-volume}"

if [[ -z "$path" ]]; then
  printf 'VOLUME_RESULT %s not-configured no-path-supplied\n' "$label"
  exit 0
fi

if [[ ! -d "$path" ]]; then
  printf 'VOLUME_RESULT %s unavailable path-is-not-a-directory\n' "$label"
  exit 0
fi

device_id() {
  if stat -c '%d' "$1" >/dev/null 2>&1; then
    stat -c '%d' "$1"
  else
    stat -f '%d' "$1"
  fi
}

if command -v findmnt >/dev/null 2>&1 && ! findmnt -T "$path" >/dev/null 2>&1; then
  printf 'VOLUME_RESULT %s unavailable path-is-not-mounted\n' "$label"
  exit 0
fi
path_device="$(device_id "$path")"
parent_device="$(device_id "$(dirname "$path")")"
if [[ "$path_device" == "$parent_device" ]]; then
  printf 'VOLUME_RESULT %s unavailable path-has-no-distinct-mount-device\n' "$label"
  exit 0
fi

marker="$path/.velvet-release-qualification-${label//[^A-Za-z0-9_.-]/_}-$$"
if ! (umask 077; printf 'velvet-volume-probe\n' >"$marker" && rm -f "$marker"); then
  printf 'VOLUME_RESULT %s read-only-or-unavailable marker-write-failed\n' "$label"
  exit 0
fi

# A writable path is evidence that the supplied mount is reachable. It does
# not prove that the path is removable or network-backed; the operator must
# supply an actual mount and record that identity in the release matrix.
printf 'VOLUME_RESULT %s pass mounted-path-readable-and-writable\n' "$label"
