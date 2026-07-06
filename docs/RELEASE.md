# DualSenseTUI Release Checklist

## Version

1. Update `version` in `Cargo.toml`.
2. Add a matching entry to `CHANGELOG.md`.
3. Run local verification:

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
cargo build --release
```

## Local macOS Package

Build the archive for the current Mac:

```bash
scripts/package-release.sh
```

Upload both files from `dist/`:

- `DualSenseTUI-<version>-<target>.tar.gz`
- `DualSenseTUI-<version>-<target>.tar.gz.sha256`

## GitHub Release

Create and push a tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds separate archives for:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

## Signing And Notarization

For a technical audience, unsigned tarballs with SHA-256 checksums are acceptable for early releases.

For broad macOS distribution, sign with a Developer ID Application certificate and submit the release artifact for Apple notarization. This requires an Apple Developer account and credentials that should be stored as CI secrets, not committed to the repository.

Recommended CI secrets for a future signed workflow:

- `APPLE_DEVELOPER_ID`
- `APPLE_ID`
- `APPLE_TEAM_ID`
- `APPLE_APP_SPECIFIC_PASSWORD`

Do not publish a signed release until the notarization step succeeds.
