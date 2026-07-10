#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "package-release.sh now builds the universal macOS DMG. Use package-macos-dmg.sh directly in new automation." >&2
exec "${script_dir}/package-macos-dmg.sh" "$@"
