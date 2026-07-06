# DualSenseTUI

[![CI](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/ci.yml)
[![Release](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/release.yml/badge.svg)](https://github.com/minashkinvladislav/DualSenseTUI/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/minashkinvladislav/DualSenseTUI?include_prereleases)](https://github.com/minashkinvladislav/DualSenseTUI/releases)
![Rust](https://img.shields.io/badge/rust-1.82%2B-orange)
![macOS](https://img.shields.io/badge/macOS-IOKit-blue)

Terminal DualSense configurator for macOS.

## Features

- `ratatui` interface for device selection, lightbar RGB, haptic strength, adaptive triggers, and button mapping profile.
- macOS IOKit HID backend. No SDL2 runtime is required.
- USB output report `0x02` and Bluetooth output report `0x31` with CRC32 for Bluetooth packets.
- Live input view for pressed buttons, analog triggers, and both sticks.
- Haptics demos: click, thump, buzz, heartbeat, sweep, mixed heavy/sharp taps, and alternating pulses.
- Adaptive trigger presets: bow, machine gun, pistol, rigid, brake, pulse, and click.
- JSON profile stored at `~/.config/DualSenseTUI/profile.json`.

## Screenshots

<table>
  <tr>
    <td align="center">
      <img src="docs/screenshots/devices.png" width="420" alt="DualSenseTUI device selection">
      <br>
      <sub>Devices</sub>
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
      <img src="docs/screenshots/haptics.png" width="420" alt="DualSenseTUI haptics demos">
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
      <img src="docs/screenshots/mapping.png" width="420" alt="DualSenseTUI button mapping profile">
      <br>
      <sub>Button mapping</sub>
    </td>
  </tr>
</table>

## Install

Download the macOS archive for your CPU from a release, unpack it, and run the binary:

```bash
tar -xzf DualSenseTUI-0.1.0-aarch64-apple-darwin.tar.gz
./DualSenseTUI-0.1.0-aarch64-apple-darwin/DualSenseTUI
```

For local development:

```bash
cargo run
```

For an optimized local binary:

```bash
cargo build --release
./target/release/DualSenseTUI
```

## Controls

- `Tab` / `Shift+Tab`: switch panels
- `1`..`6`: open Devices, Input, Lightbar, Haptics, Triggers, Mapping
- Arrow keys: move or adjust values
- `+` / `-`: fine tune the active numeric value
- `Space`: toggle the active switch
- `a` / `Enter`: apply lightbar, play the selected haptics demo, or apply trigger preset
- `p`: pulse haptics
- `d`: play the selected haptics demo
- `x`: reset adaptive triggers to Off from the Triggers tab
- `s`: save profile
- `r`: refresh devices
- `[` / `]`: shrink or grow the Devices panel
- `{` / `}`: shrink or grow the Status panel
- `<` / `>`: shrink or grow the Controls panel
- `0`: reset panel sizes
- `q` / `Esc`: quit

## Haptics

`audio-haptics` uses the DualSense haptic actuator path. `compat rumble` uses the compatibility vibration flag. The two HID motor channels do not feel like identical left/right actuators: the heavy channel is lower and stronger, while the sharp channel is higher and thinner. DualSenseTUI compensates the heavy channel before sending output, and the demo list describes the expected feel for each pattern.

## Adaptive Triggers

The Triggers tab sends DualSense adaptive trigger effect blocks for L2, R2, or both triggers. Use the preset list to pick the feel, adjust intensity, then press `a` or `Enter` to apply.
`Machine gun` and `Pulse` use persistent trigger vibration mode and stay active until you apply another preset or press `x` to reset.

## Button Mapping

DualSense does not expose persistent firmware-level button remapping through the public HID output reports used here. DualSenseTUI saves a deterministic mapping profile that another program can consume, or that can later be connected to a virtual HID driver.

## Release

For a developer-facing release, ship compressed binaries plus checksums:

```bash
scripts/package-release.sh
```

The script builds `target/release/DualSenseTUI` and writes:

- `dist/DualSenseTUI-<version>-<target>.tar.gz`
- `dist/DualSenseTUI-<version>-<target>.tar.gz.sha256`

For wider macOS distribution, use a Developer ID certificate and Apple notarization. Unsigned binaries are still useful for technical users, but browser-downloaded unsigned archives may be blocked by Gatekeeper.
