#!/usr/bin/env bash
set -euo pipefail

configuration="${1:-debug}"
app_name="DualSenseTUI"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(cd "${script_dir}/.." && pwd)"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "DualSenseTUI.app can be built only on macOS." >&2
  exit 1
fi

developer_dir="$(xcode-select -p 2>/dev/null || true)"
if [[ "$developer_dir" == "/Library/Developer/CommandLineTools" || -z "$developer_dir" ]]; then
  echo "Full Xcode is required to compile the SwiftUI application. Install Xcode and select it with xcode-select." >&2
  exit 1
fi

case "$configuration" in
  debug)
    cargo build --bin "$app_name"
    rust_binary="${workspace_dir}/target/debug/${app_name}"
    build_dir="${workspace_dir}/target/gui/debug"
    ;;
  release)
    cargo build --release --bin "$app_name"
    rust_binary="${workspace_dir}/target/release/${app_name}"
    build_dir="${workspace_dir}/target/gui/release"
    ;;
  *)
    echo "Configuration must be debug or release." >&2
    exit 2
    ;;
esac

case "$(uname -m)" in
  arm64) architecture="arm64" ;;
  x86_64) architecture="x86_64" ;;
  *)
    echo "Unsupported macOS architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

mkdir -p "$build_dir"
gui_binary="${build_dir}/${app_name}"
app_bundle="${build_dir}/${app_name}.app"
icon_path="${workspace_dir}/resources/macos/DualSenseTUI.icns"
module_cache_dir="${build_dir}/swift-module-cache"
compiler_home="${build_dir}/swift-home"
sdk_path="$(xcrun --sdk macosx --show-sdk-path)"
mkdir -p "$module_cache_dir" "$compiler_home"

HOME="$compiler_home" CLANG_MODULE_CACHE_PATH="$module_cache_dir" xcrun --sdk macosx swiftc \
  -parse-as-library \
  -O \
  -module-cache-path "$module_cache_dir" \
  -Xcc "-fmodules-cache-path=${module_cache_dir}" \
  -sdk "$sdk_path" \
  -target "${architecture}-apple-macos13.0" \
  -framework SwiftUI \
  -framework AppKit \
  -framework Combine \
  "${workspace_dir}"/macos/DualSenseApp/Sources/DualSenseApp/*.swift \
  -o "$gui_binary"

DUALSENSE_TUI_ICON_PATH="$icon_path" \
  "${script_dir}/make-macos-app.sh" "$gui_binary" "$rust_binary" "$app_bundle"
echo "$app_bundle"
