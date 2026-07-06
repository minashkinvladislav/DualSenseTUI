#!/usr/bin/env bash
set -euo pipefail

app_name="DualSenseTUI"
version="$(awk -F\" '/^version =/ { print $2; exit }' Cargo.toml)"
os="$(uname -s)"
arch="$(uname -m)"

if [[ "$os" != "Darwin" ]]; then
  echo "DualSenseTUI currently ships the IOKit backend only on macOS." >&2
  exit 1
fi

case "$arch" in
  arm64) target="aarch64-apple-darwin" ;;
  x86_64) target="x86_64-apple-darwin" ;;
  *)
    echo "Unsupported macOS architecture: $arch" >&2
    exit 1
    ;;
esac

cargo build --release

archive_base="${app_name}-${version}-${target}"
staging_dir="dist/${archive_base}"
archive_path="dist/${archive_base}.tar.gz"

rm -rf "$staging_dir"
mkdir -p "$staging_dir"

cp "target/release/${app_name}" "$staging_dir/"
cp README.md "$staging_dir/"
cp CHANGELOG.md "$staging_dir/"

tar -C dist -czf "$archive_path" "$archive_base"
shasum -a 256 "$archive_path" > "${archive_path}.sha256"

echo "Built $archive_path"
echo "Built ${archive_path}.sha256"
