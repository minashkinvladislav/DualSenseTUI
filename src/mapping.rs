use std::collections::HashSet;

use anyhow::{bail, Result};

use crate::model::{
    Button, GamepadState, KeyboardKey, KeyboardMappingProfile, MouseMappingProfile,
};

pub fn open_accessibility_settings() -> Result<()> {
    platform::open_accessibility_settings()
}

pub struct KeyboardMapper {
    pressed: HashSet<KeyboardKey>,
}

impl KeyboardMapper {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
        }
    }

    pub fn permission_status(&self) -> &'static str {
        platform::permission_status()
    }

    pub fn can_post_events(&self) -> bool {
        platform::can_post_events()
    }

    pub fn request_event_posting_access(&self) -> bool {
        platform::request_event_posting_access()
    }

    pub fn sync(&mut self, state: &GamepadState, profile: &KeyboardMappingProfile) -> Result<()> {
        if !profile.enabled {
            return self.release_all();
        }
        if !self.can_post_events() {
            bail!("grant Accessibility access to DualSenseTUI in System Settings")
        }

        let desired = profile
            .bindings
            .iter()
            .filter(|binding| !binding.to.is_disabled() && state.is_pressed(binding.from))
            .map(|binding| binding.to)
            .collect::<HashSet<_>>();

        self.transition_to(desired)
    }

    pub fn release_all(&mut self) -> Result<()> {
        let desired = HashSet::new();
        self.transition_to(desired)
    }

    fn transition_to(&mut self, desired: HashSet<KeyboardKey>) -> Result<()> {
        let released = self
            .pressed
            .difference(&desired)
            .copied()
            .collect::<Vec<_>>();
        for key in released {
            platform::post_key(key, false)?;
            self.pressed.remove(&key);
        }

        let pressed = desired
            .difference(&self.pressed)
            .copied()
            .collect::<Vec<_>>();
        for key in pressed {
            platform::post_key(key, true)?;
            self.pressed.insert(key);
        }
        Ok(())
    }
}

impl Drop for KeyboardMapper {
    fn drop(&mut self) {
        let _ = self.release_all();
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    const ALL: [Self; 3] = [Self::Left, Self::Right, Self::Middle];

    const fn platform_code(self) -> u32 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Middle => 2,
        }
    }
}

pub struct MouseMapper {
    pressed: HashSet<MouseButton>,
    cursor_remainder_x: f64,
    cursor_remainder_y: f64,
    scroll_remainder: f64,
}

impl MouseMapper {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            cursor_remainder_x: 0.0,
            cursor_remainder_y: 0.0,
            scroll_remainder: 0.0,
        }
    }

    pub fn permission_status(&self) -> &'static str {
        platform::permission_status()
    }

    pub fn can_post_events(&self) -> bool {
        platform::can_post_events()
    }

    pub fn request_event_posting_access(&self) -> bool {
        platform::request_event_posting_access()
    }

    pub fn sync(&mut self, state: &GamepadState, profile: &MouseMappingProfile) -> Result<()> {
        if !profile.enabled {
            self.reset_motion();
            return self.release_all();
        }
        if !self.can_post_events() {
            bail!("grant Accessibility access to DualSenseTUI in System Settings")
        }

        self.transition_to(desired_mouse_buttons(state))?;

        let cursor_x = mouse_axis(state.left_stick.x, profile.deadzone_percent)
            * f64::from(profile.pointer_speed);
        let cursor_y = mouse_axis(state.left_stick.y, profile.deadzone_percent)
            * f64::from(profile.pointer_speed);
        let (delta_x, delta_y) = self.take_cursor_delta(cursor_x, cursor_y);
        if delta_x != 0 || delta_y != 0 {
            platform::post_mouse_move(
                delta_x,
                delta_y,
                self.dragged_button().map(MouseButton::platform_code),
            )?;
        }

        let scroll = -mouse_axis(state.right_stick.y, profile.deadzone_percent)
            * f64::from(profile.scroll_speed);
        let scroll_delta = self.take_scroll_delta(scroll);
        if scroll_delta != 0 {
            platform::post_scroll(scroll_delta)?;
        }

        Ok(())
    }

    pub fn release_all(&mut self) -> Result<()> {
        for button in MouseButton::ALL {
            if self.pressed.contains(&button) {
                platform::post_mouse_button(button.platform_code(), false)?;
                self.pressed.remove(&button);
            }
        }
        self.reset_motion();
        Ok(())
    }

    fn transition_to(&mut self, desired: HashSet<MouseButton>) -> Result<()> {
        for button in MouseButton::ALL {
            if self.pressed.contains(&button) && !desired.contains(&button) {
                platform::post_mouse_button(button.platform_code(), false)?;
                self.pressed.remove(&button);
            }
        }

        for button in MouseButton::ALL {
            if desired.contains(&button) && !self.pressed.contains(&button) {
                platform::post_mouse_button(button.platform_code(), true)?;
                self.pressed.insert(button);
            }
        }

        Ok(())
    }

    fn take_cursor_delta(&mut self, x: f64, y: f64) -> (i32, i32) {
        self.cursor_remainder_x += x;
        self.cursor_remainder_y += y;
        let delta_x = self.cursor_remainder_x.trunc() as i32;
        let delta_y = self.cursor_remainder_y.trunc() as i32;
        self.cursor_remainder_x -= f64::from(delta_x);
        self.cursor_remainder_y -= f64::from(delta_y);
        (delta_x, delta_y)
    }

    fn take_scroll_delta(&mut self, scroll: f64) -> i32 {
        self.scroll_remainder += scroll;
        let delta = self.scroll_remainder.trunc() as i32;
        self.scroll_remainder -= f64::from(delta);
        delta
    }

    fn dragged_button(&self) -> Option<MouseButton> {
        MouseButton::ALL
            .into_iter()
            .find(|button| self.pressed.contains(button))
    }

    fn reset_motion(&mut self) {
        self.cursor_remainder_x = 0.0;
        self.cursor_remainder_y = 0.0;
        self.scroll_remainder = 0.0;
    }
}

impl Drop for MouseMapper {
    fn drop(&mut self) {
        let _ = self.release_all();
    }
}

fn desired_mouse_buttons(state: &GamepadState) -> HashSet<MouseButton> {
    let mut buttons = HashSet::new();
    if state.is_pressed(Button::Cross) {
        buttons.insert(MouseButton::Left);
    }
    if state.is_pressed(Button::Circle) {
        buttons.insert(MouseButton::Right);
    }
    if state.is_pressed(Button::Square) {
        buttons.insert(MouseButton::Middle);
    }
    buttons
}

fn mouse_axis(value: u8, deadzone_percent: u8) -> f64 {
    let normalized = ((f64::from(value) - 128.0) / 127.0).clamp(-1.0, 1.0);
    let deadzone = f64::from(deadzone_percent.min(95)) / 100.0;
    let magnitude = normalized.abs();
    if magnitude <= deadzone {
        return 0.0;
    }

    let adjusted = (magnitude - deadzone) / (1.0 - deadzone);
    normalized.signum() * adjusted * adjusted
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{ffi::c_void, process::Command, ptr};

    use anyhow::{bail, Context, Result};

    use crate::model::KeyboardKey;

    type Boolean = u8;
    type CGEventRef = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    const K_CG_HID_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
    const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
    const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
    const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
    const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
    const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
    const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
    const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
    const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
    const K_CG_EVENT_OTHER_MOUSE_DRAGGED: u32 = 27;
    const K_CG_SCROLL_EVENT_UNIT_PIXEL: u32 = 0;
    const ACCESSIBILITY_SETTINGS_URL: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightPostEventAccess() -> bool;
        fn CGRequestPostEventAccess() -> bool;
        fn CGEventCreate(source: *const c_void) -> CGEventRef;
        fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
        fn CGEventCreateKeyboardEvent(
            source: *const c_void,
            virtual_key: u16,
            key_down: Boolean,
        ) -> CGEventRef;
        fn CGEventCreateMouseEvent(
            source: *const c_void,
            mouse_type: u32,
            mouse_cursor_position: CGPoint,
            mouse_button: u32,
        ) -> CGEventRef;
        fn CGEventCreateScrollWheelEvent2(
            source: *const c_void,
            units: u32,
            wheel_count: u32,
            wheel1: i32,
            wheel2: i32,
            wheel3: i32,
        ) -> CGEventRef;
        fn CGEventPost(tap: u32, event: CGEventRef);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const c_void);
    }

    pub fn permission_status() -> &'static str {
        if can_post_events() {
            "Accessibility granted"
        } else {
            "Accessibility event posting required"
        }
    }

    pub fn can_post_events() -> bool {
        unsafe { CGPreflightPostEventAccess() }
    }

    pub fn request_event_posting_access() -> bool {
        if can_post_events() {
            return true;
        }

        let granted = unsafe { CGRequestPostEventAccess() };
        if !granted && !can_post_events() {
            // Posting a no-op mouse event also makes macOS request PostEvent access.
            post_permission_probe();
        }
        can_post_events()
    }

    pub fn open_accessibility_settings() -> Result<()> {
        let status = Command::new("/usr/bin/open")
            .arg(ACCESSIBILITY_SETTINGS_URL)
            .status()
            .context("could not open macOS Accessibility settings")?;
        if !status.success() {
            bail!("macOS Accessibility settings exited with {status}")
        }
        Ok(())
    }

    pub fn post_key(key: KeyboardKey, pressed: bool) -> Result<()> {
        let Some(virtual_key) = virtual_key_code(key) else {
            return Ok(());
        };
        let event = unsafe {
            CGEventCreateKeyboardEvent(ptr::null(), virtual_key, if pressed { 1 } else { 0 })
        };
        if event.is_null() {
            bail!("CGEventCreateKeyboardEvent returned null")
        }
        unsafe {
            CGEventPost(K_CG_HID_EVENT_TAP, event);
            CFRelease(event as *const c_void);
        }
        Ok(())
    }

    pub fn post_mouse_move(delta_x: i32, delta_y: i32, dragged_button: Option<u32>) -> Result<()> {
        let current = current_mouse_position()?;
        let mouse_type = match dragged_button {
            Some(0) => K_CG_EVENT_LEFT_MOUSE_DRAGGED,
            Some(1) => K_CG_EVENT_RIGHT_MOUSE_DRAGGED,
            Some(_) => K_CG_EVENT_OTHER_MOUSE_DRAGGED,
            None => K_CG_EVENT_MOUSE_MOVED,
        };
        post_mouse_event(
            mouse_type,
            CGPoint {
                x: current.x + f64::from(delta_x),
                y: current.y + f64::from(delta_y),
            },
            dragged_button.unwrap_or(0),
        )
    }

    pub fn post_mouse_button(button: u32, pressed: bool) -> Result<()> {
        let mouse_type = match (button, pressed) {
            (0, true) => K_CG_EVENT_LEFT_MOUSE_DOWN,
            (0, false) => K_CG_EVENT_LEFT_MOUSE_UP,
            (1, true) => K_CG_EVENT_RIGHT_MOUSE_DOWN,
            (1, false) => K_CG_EVENT_RIGHT_MOUSE_UP,
            (_, true) => K_CG_EVENT_OTHER_MOUSE_DOWN,
            (_, false) => K_CG_EVENT_OTHER_MOUSE_UP,
        };
        post_mouse_event(mouse_type, current_mouse_position()?, button)
    }

    pub fn post_scroll(vertical_delta: i32) -> Result<()> {
        let event = unsafe {
            CGEventCreateScrollWheelEvent2(
                ptr::null(),
                K_CG_SCROLL_EVENT_UNIT_PIXEL,
                1,
                vertical_delta,
                0,
                0,
            )
        };
        if event.is_null() {
            bail!("CGEventCreateScrollWheelEvent2 returned null")
        }
        post_event(event);
        Ok(())
    }

    fn current_mouse_position() -> Result<CGPoint> {
        let event = unsafe { CGEventCreate(ptr::null()) };
        if event.is_null() {
            bail!("CGEventCreate returned null")
        }
        let position = unsafe { CGEventGetLocation(event) };
        unsafe { CFRelease(event as *const c_void) };
        Ok(position)
    }

    fn post_permission_probe() {
        let Ok(position) = current_mouse_position() else {
            return;
        };
        let event =
            unsafe { CGEventCreateMouseEvent(ptr::null(), K_CG_EVENT_MOUSE_MOVED, position, 0) };
        if !event.is_null() {
            post_event(event);
        }
    }

    fn post_mouse_event(mouse_type: u32, position: CGPoint, button: u32) -> Result<()> {
        let event = unsafe { CGEventCreateMouseEvent(ptr::null(), mouse_type, position, button) };
        if event.is_null() {
            bail!("CGEventCreateMouseEvent returned null")
        }
        post_event(event);
        Ok(())
    }

    fn post_event(event: CGEventRef) {
        unsafe {
            CGEventPost(K_CG_HID_EVENT_TAP, event);
            CFRelease(event as *const c_void);
        }
    }

    fn virtual_key_code(key: KeyboardKey) -> Option<u16> {
        Some(match key {
            KeyboardKey::Disabled => return None,
            KeyboardKey::A => 0,
            KeyboardKey::S => 1,
            KeyboardKey::D => 2,
            KeyboardKey::F => 3,
            KeyboardKey::Q => 12,
            KeyboardKey::W => 13,
            KeyboardKey::E => 14,
            KeyboardKey::R => 15,
            KeyboardKey::Key1 => 18,
            KeyboardKey::Key2 => 19,
            KeyboardKey::Key3 => 20,
            KeyboardKey::Key4 => 21,
            KeyboardKey::Return => 36,
            KeyboardKey::Tab => 48,
            KeyboardKey::Space => 49,
            KeyboardKey::Escape => 53,
            KeyboardKey::Shift => 56,
            KeyboardKey::Option => 58,
            KeyboardKey::Control => 59,
            KeyboardKey::Left => 123,
            KeyboardKey::Right => 124,
            KeyboardKey::Down => 125,
            KeyboardKey::Up => 126,
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use anyhow::{bail, Result};

    use crate::model::KeyboardKey;

    pub fn permission_status() -> &'static str {
        "macOS only"
    }

    pub fn can_post_events() -> bool {
        false
    }

    pub fn request_event_posting_access() -> bool {
        false
    }

    pub fn open_accessibility_settings() -> Result<()> {
        bail!("Accessibility settings are available only on macOS")
    }

    pub fn post_key(_key: KeyboardKey, _pressed: bool) -> Result<()> {
        bail!("keyboard mapping is available only on macOS")
    }

    pub fn post_mouse_move(
        _delta_x: i32,
        _delta_y: i32,
        _dragged_button: Option<u32>,
    ) -> Result<()> {
        bail!("mouse mapping is available only on macOS")
    }

    pub fn post_mouse_button(_button: u32, _pressed: bool) -> Result<()> {
        bail!("mouse mapping is available only on macOS")
    }

    pub fn post_scroll(_vertical_delta: i32) -> Result<()> {
        bail!("mouse mapping is available only on macOS")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Button, KeyboardBinding};

    #[test]
    fn disabled_bindings_do_not_produce_desired_keyboard_events() {
        let profile = KeyboardMappingProfile::default();
        let state = GamepadState {
            buttons: vec![Button::Cross],
            ..GamepadState::default()
        };

        assert!(profile
            .bindings
            .iter()
            .filter(|binding| state.is_pressed(binding.from))
            .all(|binding| binding.to.is_disabled()));
    }

    #[test]
    fn keyboard_bindings_normalize_to_every_dualsense_button() {
        let mut profile = KeyboardMappingProfile {
            enabled: true,
            bindings: vec![KeyboardBinding {
                from: Button::Cross,
                to: KeyboardKey::Space,
            }],
        };

        profile.normalize_bindings();

        assert_eq!(profile.bindings.len(), Button::ALL.len());
        assert_eq!(profile.bindings[0].to, KeyboardKey::Space);
    }

    #[test]
    fn mouse_axis_applies_deadzone_and_preserves_direction() {
        assert_eq!(mouse_axis(128, 12), 0.0);
        assert_eq!(mouse_axis(143, 12), 0.0);
        assert!(mouse_axis(255, 12) > 0.0);
        assert!(mouse_axis(0, 12) < 0.0);
    }
}
