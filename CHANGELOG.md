# Changelog

## Unreleased

## 0.2.0 - 2026-07-10

- Refined the SwiftUI live-input dashboard with the controller's diamond face-button layout, live L1/R1 shoulder controls, compact Mapping rows, and no sidebar-toggle toolbar button.
- Reapplied the configured lightbar color on global macOS app-activation changes while the UI is inactive, avoiding the daemon fallback delay during normal focus switching.
- Added a native SwiftUI `DualSenseTUI.app` desktop interface backed by the same Rust IOKit service as the terminal UI.
- Added GUI controls for device status/live input, lightbar, haptics, audio-reactive haptics, adaptive triggers, mouse and keyboard output, controller mappings, system controls, profiles, and background mode.
- Improved the desktop controller visualization with D-pad state, aligned haptic demo groups and control rows, and paused full live snapshots on static forms for smooth Mapping scrolling.
- Added a JSON service protocol between the embedded Rust controller process and the SwiftUI app, preserving the terminal UI as an advanced interface.
- Added universal app and DMG packaging scripts with Developer ID signing, Hardened Runtime, notarization, stapling, and release workflow support.
- Added MAC-keyed controller profiles with fallback to the legacy global profile.
- Added named profile library selection, profile loading, and confirmed restore-to-defaults controls in the desktop app.
- Added IOKit hot-plug detection and one-time automatic reapplication of saved lightbar, trigger, and system-control settings after startup and reconnect.
- Added `--daemon` plus `--install-agent`, `--agent-status`, and `--uninstall-agent` for a per-user macOS LaunchAgent background service.
- Fixed the IOKit live-input callback lifetime with a manager-owned context.
- Added touchpad, six-axis motion, input sequence, Bluetooth CRC, battery-charge, headset, and microphone-state diagnostics.
- Added DualSense Edge Fn and rear-paddle input support.
- Added pairing and firmware feature-report diagnostics to the device view.
- Added a Sensors tab and a System tab for player LEDs, microphone mute/LED, controller speaker, microphone level, and HID audio route.
- Added custom adaptive-trigger resistance and vibration modes with configurable positions and frequency.
- Added opt-in audio-reactive haptics from system audio on macOS 14.2+ via Core Audio Tap.
- Fixed haptic output so manual pulses, demos, and audio-reactive playback send matching left/right motor samples.
- Added optional macOS keyboard-output mapping with Accessibility permission checks and key-release cleanup.
- Added optional macOS mouse output: pointer movement, drag, primary/secondary/middle clicks, and smooth scrolling.
- Fixed keyboard and mouse authorization to use Core Graphics event-posting access, with event-posting probes and Apple Development signing support for Cargo runs and release archives.
- Documented the user-space boundaries around CoreAudio haptics, virtual HID devices, and firmware flashing.

## 0.1.0 - Initial Release

- Added macOS IOKit DualSense discovery over USB and Bluetooth.
- Added lightbar RGB control with Bluetooth CRC output reports.
- Added live input view for buttons, analog triggers, sticks, and battery.
- Added haptics controls and demos for heavy/sharp actuator mixes.
- Added adaptive trigger presets for bow, machine gun, pistol, rigid, brake, pulse, and click.
- Added local JSON profile persistence under `~/.config/DualSenseTUI/profile.json`.
- Added terminal panel resizing and contextual controls help.
