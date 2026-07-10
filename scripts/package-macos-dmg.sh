#!/usr/bin/env bash
set -euo pipefail

app_name="DualSenseTUI"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_dir="$(cd "${script_dir}/.." && pwd)"
version="$(awk -F\" '/^version =/ { print $2; exit }' "${workspace_dir}/Cargo.toml")"
distribution="${DUALSENSE_TUI_DISTRIBUTION:-0}"
notary_profile="${DUALSENSE_TUI_NOTARY_PROFILE:-}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "DMG packaging is available only on macOS." >&2
  exit 1
fi
if [[ -z "$version" ]]; then
  echo "Could not determine package version." >&2
  exit 1
fi

"${script_dir}/build-universal-macos-app.sh"

app_bundle="${workspace_dir}/target/gui/universal/release/${app_name}.app"
dist_dir="${workspace_dir}/dist"
archive_name="${app_name}-${version}-universal.dmg"
archive_path="${dist_dir}/${archive_name}"
checksum_path="${archive_path}.sha256"
staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/${app_name}-dmg.XXXXXX")"
trap 'rm -rf "$staging_dir"' EXIT

mkdir -p "$dist_dir"
rm -f "$archive_path" "$checksum_path"
cp -R "$app_bundle" "${staging_dir}/${app_name}.app"
ln -s /Applications "${staging_dir}/Applications"

diskutil image create from \
  --volumeName "$app_name" \
  --format UDZO \
  "$staging_dir" \
  "$archive_path"

if [[ "$distribution" == "1" ]]; then
  signing_identity="${DUALSENSE_TUI_CODESIGN_IDENTITY:-}"
  if [[ -z "$signing_identity" ]]; then
    signing_identity="$(security find-identity -v -p codesigning | awk '/Developer ID Application:/ { print $2; exit }')"
  fi
  if [[ -z "$signing_identity" ]]; then
    echo "A Developer ID Application identity is required to sign the DMG." >&2
    exit 1
  fi
  if [[ -z "$notary_profile" ]]; then
    echo "DUALSENSE_TUI_NOTARY_PROFILE is required for a distribution build." >&2
    exit 1
  fi

  codesign --force --sign "$signing_identity" --timestamp "$archive_path"
  xcrun notarytool submit "$archive_path" --keychain-profile "$notary_profile" --wait
  xcrun stapler staple "$archive_path"
  spctl --assess --type open --context context:primary-signature --verbose=4 "$archive_path"
else
  echo "Warning: built an ad-hoc DMG. Set DUALSENSE_TUI_DISTRIBUTION=1 with Developer ID and notary credentials for public distribution." >&2
fi

(
  cd "$dist_dir"
  shasum -a 256 "$archive_name" > "${archive_name}.sha256"
)
echo "Built $archive_path"
echo "Built $checksum_path"
