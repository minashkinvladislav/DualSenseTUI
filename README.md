# DualSenseTUI

[![CI](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/ci.yml)
[![Release](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/release.yml/badge.svg)](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/minashkinvladislav/DualSenseTUI?include_prereleases)](https://github.com/minashkinvladislav/DualSenseTUI/releases)
![Rust](https://img.shields.io/badge/rust-1.82%2B-orange)
![macOS](https://img.shields.io/badge/macOS-IOKit-blue)

Native macOS desktop app for configuring a Sony DualSense controller. DualSenseTUI is built with SwiftUI and talks to the controller directly through IOKit; no SDL2 runtime is required. It supports macOS 13 or later on Apple Silicon and Intel Macs.

The desktop app is the primary interface. The original Ratatui console remains available for terminal-first development in the [appendix](#appendix-terminal-ui).

## Desktop App

| Screen | What it does |
| --- | --- |
| **Dashboard** | Shows the connected controller, USB/Bluetooth state, battery, raw L2/R2 values, sticks, D-pad, face buttons, and L1/R1 in real time. |
| **Lightbar** | Sets the controller light color and can keep the configured color active while the app is not focused. |
| **Haptics** | Provides strength control and symmetric demos: click, thump, buzz, heartbeat, sweep, impact, tap, and pulse train. It also supports opt-in audio-reactive haptics from system audio on macOS 14.2 or later. |
| **Adaptive Triggers** | Applies presets such as bow, pistol, machine gun, brake, pulse, and click to L2, R2, or both triggers. Resistance and vibration modes are configurable. |
| **Mouse Control** | Turns the DualSense into a macOS pointing device with adjustable pointer speed, dead zone, scrolling, clicks, and drag. |
| **Mappings** | Configures optional keyboard output and controller mapping profiles. Keyboard and mouse output are user-space mappings, not a virtual gamepad. |
| **System** | Controls player LEDs, mute state and LED, controller speaker, microphone level, and HID audio route. |
| **Profiles** | Saves per-controller settings, restores defaults, manages a reusable named-profile library, and controls the background service. |
| **Diagnostics** | Exposes pairing and firmware details, touchpad contacts, motion sensors, Bluetooth CRC state, and DualSense Edge Fn/rear-paddle input. |

`Haptic v2` and `Legacy rumble` select controller compatibility behavior for the same HID motor values; they are not audio sources. Haptics demos and audio-reactive playback send the same level to both motors.

## Screenshots

<table>
  <tr>
    <td align="center">
      <img src="docs/screenshots/devices.png" width="420" alt="DualSenseTUI dashboard and controller selection">
      <br>
      <sub>Dashboard</sub>
    </td>
    <td align="center">
      <img src="docs/screenshots/input.png" width="420" alt="DualSenseTUI live input view">
      <br>
      <sub>Live input</sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="docs/screenshots/lightbar.png" width="420" alt="DualSenseTUI lightbar controls">
      <br>
      <sub>Lightbar</sub>
    </td>
    <td align="center">
      <img src="docs/screenshots/haptics.png" width="420" alt="DualSenseTUI haptics controls">
      <br>
      <sub>Haptics</sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="docs/screenshots/triggers.png" width="420" alt="DualSenseTUI adaptive trigger presets">
      <br>
      <sub>Adaptive triggers</sub>
    </td>
    <td align="center">
      <img src="docs/screenshots/mapping.png" width="420" alt="DualSenseTUI keyboard mappings">
      <br>
      <sub>Keyboard mappings</sub>
    </td>
  </tr>
</table>

## Install

### Download

Download `DualSenseTUI-<version>-universal.dmg` from [Releases](https://github.com/minashkinvladislav/DualSenseTUI/releases), open it, and drag `DualSenseTUI.app` to `Applications`. Then launch it from Applications or Spotlight; Terminal is not required.

The release page identifies whether an artifact is notarized. An **Unsigned Preview** is ad-hoc signed and can require an explicit macOS confirmation on first launch. Verify its published SHA-256 checksum before opening it.

### Build from source

The native app requires full Xcode, not only Command Line Tools:

```bash
scripts/run-macos-app.sh
```

This builds and launches `target/gui/debug/DualSenseTUI.app` with the embedded Rust controller service.

## Everyday Use

1. Connect a DualSense over USB or Bluetooth and open DualSenseTUI.
2. Check the **Dashboard** to confirm the controller and live input are visible.
3. Open the relevant screen to change the lightbar, haptics, triggers, mouse controls, mappings, or system controls.
4. In **Profiles**, choose **Save for Controller** to make the current settings reapply after reconnect.

### Permissions

- **Accessibility** is requested only when enabling keyboard output or mouse control. Use the app's **Grant Accessibility** and **Open Settings** controls, then allow the signed `DualSenseTUI.app` bundle in macOS Settings.
- **Screen & System Audio Recording** is requested only when starting audio-reactive haptics. It is available on macOS 14.2 or later.
- Lightbar, haptics, adaptive triggers, diagnostics, and controller profiles do not need Accessibility permission.

## Mouse Control

Enable **Mouse Control** in the app, then switch focus to the application you want to control:

| DualSense input | macOS action |
| --- | --- |
| Left stick | Move pointer |
| Right stick, vertical axis | Scroll |
| Cross | Left click |
| Circle | Right click |
| Square | Middle click |
| Hold a click and move left stick | Drag |

Disable mouse output before returning the controller to a game. The app releases held mouse buttons when output is disabled, the controller disconnects, or DualSenseTUI exits.

## Profiles and Background Mode

**Profiles** stores controller-specific settings by pairing MAC address under:

```text
~/.config/DualSenseTUI/profiles/aa-bb-cc-dd-ee-ff.json
```

The screen also provides a separate named library at `~/.config/DualSenseTUI/saved-profiles/`. Use **Save to Library** for reusable presets, **Load Library Profile** to apply one, and **Restore Defaults** to reset the current controller state.

To keep mappings and saved settings active after closing the window, move the app to `/Applications` and enable **Keep mappings active in background** in **Profiles**. DualSenseTUI installs a per-user LaunchAgent without `sudo`; re-enable it after moving the app to another path.

## Support Boundaries

DualSenseTUI reads firmware information but does not flash firmware; use Sony's official updater for firmware installation. It does not install a virtual HID gamepad or perform firmware-level button remapping. Keyboard and mouse features are optional user-space output mappings and do not suppress the controller's original input.

## Development and Release

Run the local checks with:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Use the [manual test checklist](docs/TESTING.md) for hardware regression testing. Maintainers can find packaging, signing, notarization, and release instructions in [docs/RELEASE.md](docs/RELEASE.md).

## Appendix: Terminal UI

The Ratatui interface is retained for advanced workflows, development, and users who prefer a terminal. It uses the same IOKit backend and controller profiles as the desktop app.

### Run the console

```bash
cargo run
```

For an optimized build:

```bash
cargo run --release
```

On macOS, `cargo run` creates `target/debug/DualSenseTUI.app` and runs the inner terminal executable. For reliable keyboard or mouse event-posting permission in local builds, use full Xcode with an Apple Development certificate. Command Line Tools-only builds use ad-hoc signing, which is not a reliable TCC permission path.

To request and inspect event-posting access without opening the HID backend:

```bash
cargo run -- --request-event-posting-access
```

### Console controls

| Keys | Action |
| --- | --- |
| `Tab` / `Shift+Tab`, `1` through `8` | Change panel |
| Arrow keys, `+` / `-` | Move selection or adjust the selected value |
| `Space` | Toggle the selected state, start/stop audio haptics, run a demo, or reset a mapping |
| `a` / `Enter` | Apply the current screen's action |
| `p`, `d`, `x` | Pulse haptics, run the selected demo, or reset adaptive triggers |
| `m`, `k`, `o` | Change mapping view, toggle output, or open Accessibility Settings |
| `s`, `r` | Save the controller profile or refresh devices |
| `[`, `]`, `{`, `}`, `<`, `>`, `0` | Resize or reset terminal panels |
| `q` / `Esc` | Quit |

### Terminal background-agent fallback

After saving a controller profile from the console, the same per-user background service can be managed from Terminal:

```bash
target/debug/DualSenseTUI.app/Contents/MacOS/DualSenseTUI --install-agent
target/debug/DualSenseTUI.app/Contents/MacOS/DualSenseTUI --agent-status
target/debug/DualSenseTUI.app/Contents/MacOS/DualSenseTUI --uninstall-agent
```

The agent watches for controller connections and reapplies saved settings after reconnect. It has no terminal UI, and no `sudo` is required.
