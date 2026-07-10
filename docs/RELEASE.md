# DualSenseTUI Release Checklist

## Product Artifact

The public artifact is one universal macOS disk image:

```text
DualSenseTUI-<version>-universal.dmg
  DualSenseTUI.app -> Applications
```

It contains the SwiftUI desktop app and its signed Rust `DualSenseCore` helper. Do not publish the legacy terminal `.tar.gz` as the primary download.

## Version And Verification

1. Update `version` in `Cargo.toml`.
2. Add a matching entry to `CHANGELOG.md`.
3. Install full Xcode and select it with:

   ```bash
   sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
   ```

4. Run local verification:

   ```bash
   cargo fmt --check
   cargo test
   cargo clippy --all-targets -- -D warnings
   scripts/build-macos-app.sh release
   ```

5. Test `target/gui/release/DualSenseTUI.app` from Finder, not only from Terminal.

## Developer ID And Notarization

Public distribution requires Apple Developer Program membership, a **Developer ID Application** certificate, and notarization. Apple Development certificates are valid for local development but not for notarized public releases.

For a local signed release, create a notarytool keychain profile once:

```bash
xcrun notarytool store-credentials DualSenseTUI-notary \
  --apple-id "your-apple-id@example.com" \
  --team-id "YOUR_TEAM_ID" \
  --password "APP_SPECIFIC_PASSWORD"
```

Then package, sign, notarize, staple, and assess the universal DMG:

```bash
DUALSENSE_TUI_DISTRIBUTION=1 \
DUALSENSE_TUI_NOTARY_PROFILE=DualSenseTUI-notary \
scripts/package-macos-dmg.sh
```

The output is:

- `dist/DualSenseTUI-<version>-universal.dmg`
- `dist/DualSenseTUI-<version>-universal.dmg.sha256`

The script refuses a distribution build when the Developer ID identity or notary profile is missing. It enables Hardened Runtime and secure timestamps for both executables, signs the DMG, submits it through `notarytool`, staples the ticket, and runs `spctl` assessment.

## GitHub Release

The tag workflow always builds a universal DMG. Its distribution mode is explicit:

- With **none** of the signing secrets configured, it publishes an **Unsigned Preview** GitHub pre-release. The app is ad-hoc signed and not notarized; Gatekeeper can require an explicit approval before first launch.
- With **all** six secrets configured, it publishes a notarized stable release.
- With only some secrets configured, it fails rather than silently downgrading a presumed signed release.

Configure these repository secrets for a notarized stable release:

- `APPLE_CERTIFICATE_BASE64`: base64-encoded `.p12` containing the Developer ID Application certificate and private key.
- `APPLE_CERTIFICATE_PASSWORD`: password for that `.p12`.
- `KEYCHAIN_PASSWORD`: temporary CI keychain password.
- `APPLE_ID`: Apple ID used for notarization.
- `APPLE_TEAM_ID`: Apple Developer team ID.
- `APPLE_APP_SPECIFIC_PASSWORD`: app-specific password for notarization.

Create and push the annotated tag:

```bash
release_version="$(awk -F\" '/^version =/ { print $2; exit }' Cargo.toml)"
git tag -a "v${release_version}" -m "DualSenseTUI ${release_version}"
git push origin main "v${release_version}"
```

The workflow validates that the tag matches `Cargo.toml`. After correcting a workflow configuration issue, run **Release** manually from the same `v<version>` tag and enter that tag in the dispatch form.

## Distribution QA

1. Download the release DMG on a clean macOS user account or another Mac.
2. Verify the disk image mounts and offers `DualSenseTUI.app` plus the `Applications` shortcut.
3. Drag the app to Applications and launch it from Finder.
4. For a notarized release, confirm Gatekeeper accepts it without bypass instructions. For an Unsigned Preview, confirm the release page states the warning prominently.
5. Confirm the app starts the Rust controller service, detects a controller, and can request Accessibility for mouse output.
6. Test replacing a prior app at the same `/Applications/DualSenseTUI.app` path; profiles must remain available and the background service must still point to that path. Re-enable the background service only if the app was moved elsewhere.
