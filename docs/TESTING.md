# DualSenseTUI Manual Test Checklist

Use this order for a release candidate. Test the SwiftUI desktop app from Finder or Applications, and use a normal macOS Terminal window only for the advanced console checks. IOKit access, Accessibility permission, and focus-sensitive keyboard or mouse output must be tested outside SSH or an IDE pseudo-terminal.

## Test Record

| Field | Value |
| --- | --- |
| Date | |
| macOS version | |
| DualSense model / firmware | |
| Transport | USB / Bluetooth |
| Tester | |
| Result | Pass / Fail |

## 1. Build And Clean Profile

- [ ] Run the static checks:

  ```bash
  cargo fmt --check
  cargo test
  cargo clippy --all-targets -- -D warnings
  cargo build --release
  ```

- [ ] Start with an isolated profile so old preferences do not affect the result:

  ```bash
  XDG_CONFIG_HOME="$(mktemp -d)" cargo run
  ```

- [ ] Confirm that the app opens, can be exited with `q`, and leaves the terminal usable.
- [ ] Confirm there are no `malloc`, `pointer being freed`, or IOKit errors in the terminal after exit.

## Native Desktop App

- [ ] Confirm full Xcode is selected, then build the desktop app:

  ```bash
  scripts/build-macos-app.sh debug
  ```

- [ ] Open `target/gui/debug/DualSenseTUI.app` from Finder. No Terminal window may be required or opened.
- [ ] Confirm Dashboard, Lightbar, Haptics, Adaptive Triggers, Mouse Control, Mappings, System, and Profiles are available in the sidebar.
- [ ] Confirm Dashboard shows the selected device, live sticks/buttons, trigger levels, battery state, and profile status.
- [ ] On Dashboard, press every D-pad direction and each diagonal; the four D-pad arrows must highlight independently and the textual pressed-button state must match.
- [ ] On Haptics, confirm the demo actions are arranged as two aligned groups of four, and that control sliders and audio meters share the same left/right control columns.
- [ ] Scroll through Mappings repeatedly while the controller is connected. The form must remain responsive and must not jump or relayout from live-input updates.
- [ ] Change each primary setting from the GUI, then confirm the physical controller responds exactly as it does from the terminal console.
- [ ] In Profiles, enable and disable **Keep mappings active in background**. Confirm the status changes without requiring `sudo` or manually editing a plist.
- [ ] In Profiles, save a named profile, change a visible setting, load the named profile, and confirm the saved value is restored. Use **Restore Defaults**, confirm the dialog, then save the reset profile and verify it persists after restart.
- [ ] Quit the GUI while a mouse or keyboard mapping button is held. Confirm no synthesized input remains stuck.

## 2. USB Discovery And Lifecycle

- [ ] Connect the controller with a data-capable USB cable, then start DualSenseTUI.
- [ ] On the Devices tab, confirm the controller name, product ID, `USB` transport, model, MAC address, firmware, hardware version, and feature level are shown when the device exposes those reports.
- [ ] Unplug the controller while the app is running. The app must remain stable and clear live input automatically; `r` remains a manual recovery action.
- [ ] Reconnect the controller and confirm it reappears without pressing `r` or restarting the app.
- [ ] Repeat disconnect/reconnect at least three times. No allocator error or stale duplicate device entry is allowed.

## 3. Live Input

- [ ] Open Input with `2`.
- [ ] Press each face button, shoulder button, trigger click, Create, Options, L3, R3, PS, touchpad click, Mute, and each D-pad direction. Each must highlight once while pressed and clear after release.
- [ ] Move both sticks through their full travel. The dot and normalized values must move in the expected direction.
- [ ] Pull L2 and R2 independently, then together. Both analog gauges must respond from `0` to near `255`.
- [ ] Verify the status row reports plausible battery and charging state. On USB, `charging` or `full` is expected depending on controller state.
- [ ] Connect/disconnect a headset if available. Check headset, microphone-connected, and microphone-muted status.

## 4. Touchpad And Motion

- [ ] Open Sensors with `3`.
- [ ] Touch the pad with one finger, then two. Check both contact IDs and coordinates update; removing each finger must mark that contact inactive.
- [ ] Slide one finger from each edge to the opposite edge. Coordinates should span approximately `0..1920` horizontally and `0..1080` vertically.
- [ ] Slowly rotate and tilt the controller. Gyroscope and accelerometer values must change on the corresponding axes.
- [ ] Leave the controller still for several seconds. Sensor timestamp must keep advancing; values should settle rather than grow without input.

## 5. Lightbar And Haptics

- [ ] Open Lightbar with `4`, change R/G/B independently, and press `a` or `Enter`. Check the displayed swatch and physical lightbar match.
- [ ] Test black (`0,0,0`), a saturated primary color, and a mixed color. Reapplying a color must not trigger a startup animation or disconnect the controller.
- [ ] Open Haptics with `5`. Test `p` at several `Motor strength` values; the same sample must be sent to both motors.
- [ ] Play every haptics demo with `d` or `a`. Confirm every active sample is paired left/right, especially Impact, Tap, and Pulse train.
- [ ] Repeat several demos with `haptic-v2` and `legacy rumble` protocol. Output must stop after each demo and must not remain active after returning to another tab.
- [ ] On macOS 14.2 or later, run the signed app bundle (`cargo run` is sufficient) and select `Audio reactive` in Haptics. Press `Space`; on the first run, grant `DualSenseTUI.app` access in Privacy & Security > Screen & System Audio Recording.
- [ ] Play a safe music source. Either Bass input or Detail input should raise the shared output, and both motors should react together. Adjust Sensitivity and Noise gate, then confirm the controls take effect without restarting capture.
- [ ] Press `Space` to stop. Confirm vibration stops immediately. Repeat while changing controller selection, pressing `r`, disconnecting the controller, and quitting with `q`; no motor may remain active.
- [ ] On macOS below 14.2, selecting Audio reactive must show that the feature is unavailable and must not affect demos or normal haptics.

## 6. Adaptive Triggers

- [ ] Open Triggers with `6`. Start by selecting `Off` and applying it to `Both`.
- [ ] For `Left`, `Right`, and `Both`, apply Bow, Pistol, Rigid, Brake, Click, Machine gun, and Pulse. Confirm effects appear only on the selected trigger(s).
- [ ] After Machine gun and Pulse, apply `Off` or press `x`; trigger vibration must stop.
- [ ] Select `Resistance`, set Start and End positions, apply, and verify resistance begins and ends at the configured travel range.
- [ ] Select `Vibration`, set Start and Frequency, apply, and verify it persists until reset or another effect replaces it.
- [ ] Return to `Off` before changing transport or quitting.

## 7. System Controls And Audio Routing

- [ ] Open System with `7`.
- [ ] Cycle Player LEDs through Off and Player 1 through Player 5. Confirm the physical five-LED patterns change and the selected pattern is centered as expected.
- [ ] Toggle microphone mute, press `a` or `Enter`, and check the controller Mute LED and live microphone-muted status.
- [ ] Adjust Controller speaker and Microphone level; apply and verify with a safe audio source or headset when available.
- [ ] Test `Keep current`, `Headphones`, and `Controller speaker` output routes on USB. Do not expect DualSenseTUI to stream audio; macOS manages the USB CoreAudio endpoint separately.

## 8. Bluetooth Regression

- [ ] Disconnect USB, pair the same controller over Bluetooth, and start or refresh the app.
- [ ] Confirm Devices shows `Bluetooth` transport and the controller remains usable after at least one minute.
- [ ] Repeat Live Input, Lightbar, Haptics, Trigger, and System checks that are supported by the controller over Bluetooth.
- [ ] In Sensors, verify Bluetooth CRC is `valid` when the system exposes a complete frame. `not present` is acceptable when macOS strips the CRC; `invalid` must be recorded as a defect if it appears repeatedly with otherwise valid reports.
- [ ] Test reconnecting Bluetooth while the app is running. The device must be removed and re-added cleanly without pressing `r`.

## 9. DualSense Edge (When Available)

- [ ] Connect a DualSense Edge and confirm the Devices tab identifies it as `DualSense Edge`.
- [ ] Press Fn 1, Fn 2, Left paddle, and Right paddle on the Input tab. Each must highlight independently.
- [ ] Repeat the core lightbar, haptics, trigger, and system-control checks.

## 10. Mapping

- [ ] Open Mapping with `8`. In `Controller profile`, change a row, press `s`, restart the app, and confirm the saved profile reloads. With a readable MAC address, confirm the status path is under `profiles/` and that a second controller starts from its own profile. This is a logical profile, not a virtual controller remap.
- [ ] Press `m` to switch to `Keyboard output`.
- [ ] If this is the first run after upgrading to app bundles, remove old `DualSenseTUI` entries from Accessibility before granting access again.
- [ ] Run `security find-identity -v -p codesigning`. For a persistent local permission, confirm an Apple Development or Developer ID Application identity is available. If it reports zero identities, install full Xcode, add an Apple Account, and create an Apple Development certificate before validating keyboard or mouse output.
- [ ] Run `cargo run -- --request-event-posting-access`. It must report `Accessibility granted` after consent and must not initialize the DualSense HID backend.
- [ ] Press `k`. When event-synthesis access is not yet granted, DualSenseTUI must request the Core Graphics posting permission and open the macOS Accessibility settings page. Press `o` to reopen that page, then allow the executable and press `k` again.
- [ ] In macOS System Settings > Privacy & Security > Accessibility, verify the running `DualSenseTUI` app bundle is enabled. `cargo run` creates `target/debug/DualSenseTUI.app`; the release package contains a signed `DualSenseTUI.app` bundle.
- [ ] Assign `Cross -> Space` and `DPad Up/Down/Left/Right -> arrow keys` using Up/Down and Left/Right.
- [ ] Press `k` to enable keyboard output. The UI must show that output is active.
- [ ] Keep DualSenseTUI running, switch focus to a text editor or test application, and press/release the mapped controls. The target application must receive matching key-down and key-up events.
- [ ] Disable output with `k`, then confirm mapped controls no longer emit keys.
- [ ] While a mapped button is held, unplug the controller or quit with `q`. Confirm no key remains stuck in the target application. Do not force-kill the process during this check.
- [ ] Press `m` to switch to `Mouse output`. Confirm the screen documents left-stick pointer control, right-stick vertical scrolling, and the Cross/Circle/Square click bindings.
- [ ] With Mouse output disabled, move both sticks and press Cross, Circle, and Square. The pointer, scroll position, and clicks must remain unchanged.
- [ ] Enable Mouse output with `k`, switch focus to a safe target app, and move the left stick in every direction. The pointer must move smoothly, stop inside the configured dead zone, and respect pointer-speed changes.
- [ ] Move the right stick up and down over a scrollable target. Confirm it scrolls in the expected direction and scroll-speed changes take effect.
- [ ] Verify `Cross` left-clicks, `Circle` right-clicks, and `Square` middle-clicks. Hold Cross while moving the left stick and confirm drag-and-drop works; disable output or quit and confirm no mouse button remains held.
- [ ] Repeat the mapping checks in the desktop app: configure bindings in Mappings, enable keyboard output, configure Mouse Control, then verify the same keyboard and pointer behavior in the target application.

## 11. Profile Compatibility And Error Paths

- [ ] Start once with the existing pre-update `~/.config/DualSenseTUI/profile.json` if one exists. It must load without a parse error and receive default values for System, custom triggers, audio-reactive haptics, keyboard mappings, and mouse mapping.
- [ ] With no controller connected, visit every tab and use its documented controls. The app must show a clear no-device status and never panic.
- [ ] Attempt each apply action with no controller connected. It must fail gracefully without changing the saved profile unexpectedly.

## 12. Automatic Reapplication And Background Agent

- [ ] With a controller connected, set a distinctive lightbar color, non-default trigger effect, and player LED state, then press `s`.
- [ ] Disconnect and reconnect it over USB. Within about one second, confirm the saved lightbar, trigger, and system-control settings return without pressing `r` or `a`.
- [ ] Repeat over Bluetooth. Confirm that a haptics demo is not played automatically.
- [ ] In the desktop app's Profiles screen, enable **Keep mappings active in background**, then run `target/gui/debug/DualSenseTUI.app/Contents/MacOS/DualSenseCore --agent-status`; it must report both `installed: true` and `loaded: true`.
- [ ] Quit the desktop app, reconnect the controller, and confirm the background agent reapplies the saved profile. If keyboard or mouse output was enabled in the saved profile, verify its Accessibility permission still permits it.
- [ ] Disable **Keep mappings active in background** in Profiles and confirm `target/gui/debug/DualSenseTUI.app/Contents/MacOS/DualSenseCore --agent-status` reports that the agent is no longer installed. Do not leave a test agent installed unintentionally.

## 13. Release Gate

- [ ] Re-run the commands from section 1 after the manual checks.
- [ ] Build the universal local DMG with `scripts/package-macos-dmg.sh`.
- [ ] For public distribution, run the Developer ID/notarization flow from `docs/RELEASE.md`.
- [ ] Verify DMG contents, Finder installation flow, Gatekeeper assessment, and SHA-256 checksum.
- [ ] Attach this completed checklist to the release notes or QA record.

## Intentionally Out Of Scope

- Firmware flashing: verify only that firmware information is displayed. Use Sony's official updater for installation.
- Virtual HID gamepad remapping: the keyboard mapper is user-space output only. A virtual controller requires a separately signed DriverKit system extension.
