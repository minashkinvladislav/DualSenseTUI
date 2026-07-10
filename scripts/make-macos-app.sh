#!/usr/bin/env bash
set -euo pipefail

app_name="DualSenseTUI"
core_name="DualSenseCore"
signing_identifier="com.github.minashkinvladislav.dualsensetui"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(cd "${script_dir}/.." && pwd)"
template="${workspace_dir}/resources/macos/DualSenseTUI-GUI-Info.plist"
icon_path="${DUALSENSE_TUI_ICON_PATH:-}"
attributions_path="${workspace_dir}/docs/ATTRIBUTIONS.md"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "DualSenseTUI.app is available only on macOS." >&2
  exit 1
fi

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <gui-binary> <rust-core-binary> <DualSenseTUI.app>" >&2
  exit 2
fi

gui_binary="$1"
core_binary="$2"
app_bundle="$3"
gui_executable="${app_bundle}/Contents/MacOS/${app_name}"
core_executable="${app_bundle}/Contents/MacOS/${core_name}"
info_plist="${app_bundle}/Contents/Info.plist"
pkg_info="${app_bundle}/Contents/PkgInfo"

if [[ -z "$icon_path" ]]; then
  echo "DUALSENSE_TUI_ICON_PATH must point to a generated .icns file." >&2
  exit 2
fi

for input in "$gui_binary" "$core_binary" "$template" "$icon_path" "$attributions_path"; do
  if [[ ! -f "$input" ]]; then
    echo "Required file not found: $input" >&2
    exit 1
  fi
done

version="$(awk -F\" '/^version =/ { print $2; exit }' "${workspace_dir}/Cargo.toml")"
if [[ -z "$version" ]]; then
  echo "Could not determine package version." >&2
  exit 1
fi

rm -rf "$app_bundle"
mkdir -p "${app_bundle}/Contents/MacOS" "${app_bundle}/Contents/Resources"
cp "$gui_binary" "$gui_executable"
cp "$core_binary" "$core_executable"
cp "$icon_path" "${app_bundle}/Contents/Resources/DualSenseTUI.icns"
cp "$attributions_path" "${app_bundle}/Contents/Resources/ATTRIBUTIONS.md"
chmod 755 "$gui_executable" "$core_executable"
sed "s/@VERSION@/${version}/g" "$template" > "$info_plist"
printf 'APPL????' > "$pkg_info"
plutil -lint "$info_plist"

distribution="${DUALSENSE_TUI_DISTRIBUTION:-0}"
signing_identity="${DUALSENSE_TUI_CODESIGN_IDENTITY:-}"
if [[ -z "$signing_identity" && "$distribution" == "1" ]]; then
  signing_identity="$(security find-identity -v -p codesigning | awk '/Developer ID Application:/ { print $2; exit }')"
elif [[ -z "$signing_identity" ]]; then
  signing_identity="$(security find-identity -v -p codesigning | awk '/(Apple Development|Developer ID Application):/ { print $2; exit }')"
fi

sign_code() {
  local path="$1"
  if [[ "$signing_identity" == "-" ]]; then
    local requirements="=designated => identifier \"${signing_identifier}\""
    codesign --force --sign - --identifier "$signing_identifier" --requirements "$requirements" "$path"
  elif [[ "$distribution" == "1" ]]; then
    codesign --force --sign "$signing_identity" --identifier "$signing_identifier" --options runtime --timestamp "$path"
  else
    codesign --force --sign "$signing_identity" --identifier "$signing_identifier" "$path"
  fi
}

if [[ -z "$signing_identity" ]]; then
  if [[ "$distribution" == "1" ]]; then
    echo "A Developer ID Application identity is required for a distribution build." >&2
    exit 1
  fi
  signing_identity="-"
  echo "Warning: no Apple Development or Developer ID Application identity was found; using ad-hoc signing." >&2
fi

sign_code "$core_executable"
sign_code "$gui_executable"
sign_code "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"

echo "Built $app_bundle with bundle identifier $signing_identifier using $signing_identity"
