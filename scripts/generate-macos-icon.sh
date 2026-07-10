#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(cd "${script_dir}/.." && pwd)"
output_path="${workspace_dir}/resources/macos/DualSenseTUI.icns"
source_path="${workspace_dir}/resources/macos/Seeklogo-PS5-Controller-Gamepad.png"
temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/dualsense-tui-icon.XXXXXX")"
iconset_dir="${temporary_dir}/DualSenseTUI.iconset"

cleanup() {
  rm -rf "$temporary_dir"
}
trap cleanup EXIT

if [[ ! -f "$source_path" ]]; then
  printf 'Missing Seeklogo icon source: %s\n' "$source_path" >&2
  exit 1
fi

swift "${script_dir}/generate-macos-icon.swift" "$source_path" "${temporary_dir}/icon_1024.png"
mkdir -p "$iconset_dir"

render_size() {
  local pixels="$1"
  local name="$2"
  sips -z "$pixels" "$pixels" "${temporary_dir}/icon_1024.png" --out "${iconset_dir}/${name}.png" >/dev/null
}

render_size 16 icon_16x16
render_size 32 icon_16x16@2x
render_size 32 icon_32x32
render_size 64 icon_32x32@2x
render_size 128 icon_128x128
render_size 256 icon_128x128@2x
render_size 256 icon_256x256
render_size 512 icon_256x256@2x
render_size 512 icon_512x512
render_size 1024 icon_512x512@2x

iconutil -c icns "$iconset_dir" -o "$output_path"
echo "$output_path"
