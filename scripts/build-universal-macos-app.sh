#!/usr/bin/env bash
set -euo pipefail

app_name="DualSenseTUI"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(cd "${script_dir}/.." && pwd)"
build_dir="${workspace_dir}/target/gui/universal/release"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Universal macOS builds are available only on macOS." >&2
  exit 1
fi

developer_dir="$(xcode-select -p 2>/dev/null || true)"
if [[ "$developer_dir" == "/Library/Developer/CommandLineTools" || -z "$developer_dir" ]]; then
  echo "Full Xcode is required to compile the SwiftUI application. Install Xcode and select it with xcode-select." >&2
  exit 1
fi

rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --bin "$app_name" --target aarch64-apple-darwin
cargo build --release --bin "$app_name" --target x86_64-apple-darwin

mkdir -p "$build_dir"
core_binary="${build_dir}/DualSenseCore"
gui_binary="${build_dir}/${app_name}"
gui_arm64_binary="${build_dir}/${app_name}-arm64"
gui_x86_64_binary="${build_dir}/${app_name}-x86_64"
app_bundle="${build_dir}/${app_name}.app"
icon_path="${workspace_dir}/resources/macos/DualSenseTUI.icns"
module_cache_dir="${build_dir}/swift-module-cache"
compiler_home="${build_dir}/swift-home"
sdk_path="$(xcrun --sdk macosx --show-sdk-path)"
mkdir -p "$module_cache_dir" "$compiler_home"

lipo -create \
  "${workspace_dir}/target/aarch64-apple-darwin/release/${app_name}" \
  "${workspace_dir}/target/x86_64-apple-darwin/release/${app_name}" \
  -output "$core_binary"

build_gui_architecture() {
  local target="$1"
  local output="$2"
  local cache="${module_cache_dir}/${target%%-*}"
  mkdir -p "$cache"

  HOME="$compiler_home" CLANG_MODULE_CACHE_PATH="$cache" xcrun --sdk macosx swiftc \
    -parse-as-library \
    -O \
    -module-cache-path "$cache" \
    -Xcc "-fmodules-cache-path=${cache}" \
    -sdk "$sdk_path" \
    -target "$target" \
    -framework SwiftUI \
    -framework AppKit \
    -framework Combine \
    "${workspace_dir}"/macos/DualSenseApp/Sources/DualSenseApp/*.swift \
    -o "$output"
}

build_gui_architecture arm64-apple-macos13.0 "$gui_arm64_binary"
build_gui_architecture x86_64-apple-macos13.0 "$gui_x86_64_binary"
lipo -create "$gui_arm64_binary" "$gui_x86_64_binary" -output "$gui_binary"

DUALSENSE_TUI_ICON_PATH="$icon_path" \
  "${script_dir}/make-macos-app.sh" "$gui_binary" "$core_binary" "$app_bundle"
lipo -info "${app_bundle}/Contents/MacOS/${app_name}"
lipo -info "${app_bundle}/Contents/MacOS/DualSenseCore"
echo "$app_bundle"
