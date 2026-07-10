#!/usr/bin/env bash
set -euo pipefail

configuration="${1:-debug}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(cd "${script_dir}/.." && pwd)"

"${script_dir}/build-macos-app.sh" "$configuration"
app_bundle="${workspace_dir}/target/gui/${configuration}/DualSenseTUI.app"
exec /usr/bin/open -n "$app_bundle"
