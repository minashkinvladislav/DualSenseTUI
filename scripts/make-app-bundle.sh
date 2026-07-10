#!/usr/bin/env bash
set -euo pipefail

app_name="DualSenseTUI"
signing_identifier="com.github.minashkinvladislav.dualsensetui"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(cd "${script_dir}/.." && pwd)"
template="${workspace_dir}/resources/macos/Info.plist"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "DualSenseTUI app bundles are available only on macOS." >&2
  exit 1
fi

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <DualSenseTUI-binary> <DualSenseTUI.app>" >&2
  exit 2
fi

source_binary="$1"
app_bundle="$2"
app_executable="${app_bundle}/Contents/MacOS/${app_name}"
info_plist="${app_bundle}/Contents/Info.plist"
pkg_info="${app_bundle}/Contents/PkgInfo"

if [[ ! -f "$source_binary" ]]; then
  echo "Binary not found: $source_binary" >&2
  exit 1
fi
if [[ ! -f "$template" ]]; then
  echo "Info.plist template not found: $template" >&2
  exit 1
fi

version="$(awk -F\" '/^version =/ { print $2; exit }' "${workspace_dir}/Cargo.toml")"
if [[ -z "$version" ]]; then
  echo "Could not determine package version." >&2
  exit 1
fi

mkdir -p "${app_bundle}/Contents/MacOS"
cp "$source_binary" "$app_executable"
sed "s/@VERSION@/${version}/g" "$template" > "$info_plist"
printf 'APPL????' > "$pkg_info"
plutil -lint "$info_plist"

signing_identity="${DUALSENSE_TUI_CODESIGN_IDENTITY:-}"
if [[ -z "$signing_identity" ]]; then
  signing_identity="$(security find-identity -v -p codesigning | awk '/(Apple Development|Developer ID Application):/ { print $2; exit }')"
fi

sign_code() {
  if [[ "$signing_identity" == "-" ]]; then
    local requirements="=designated => identifier \"${signing_identifier}\""
    codesign --force --sign - --identifier "$signing_identifier" --requirements "$requirements" "$1"
  else
    codesign --force --sign "$signing_identity" --identifier "$signing_identifier" "$1"
  fi
}

if [[ -z "$signing_identity" ]]; then
  signing_identity="-"
  echo "Warning: no Apple Development or Developer ID Application identity was found; using ad-hoc signing." >&2
  echo "Event-posting access is not reliable with ad-hoc signing. Install Xcode, create an Apple Development certificate, and rerun cargo run." >&2
fi

sign_code "$app_executable"
sign_code "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"

echo "Built $app_bundle with bundle identifier $signing_identifier using $signing_identity"
