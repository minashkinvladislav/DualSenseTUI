#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Cargo runner did not receive an executable path." >&2
  exit 2
fi

binary="$1"
shift
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "$(basename "$(dirname "$binary")")" == "deps" ]]; then
  exec "$binary" "$@"
fi

app_bundle="$(dirname "$binary")/DualSenseTUI.app"
"${script_dir}/make-app-bundle.sh" "$binary" "$app_bundle"

exec "${app_bundle}/Contents/MacOS/DualSenseTUI" "$@"
