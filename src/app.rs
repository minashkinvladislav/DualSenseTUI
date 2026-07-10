use std::{
    collections::HashMap,
    io::{self, Stdout},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    audio_reactive::{AudioReactiveHaptics, AudioReactiveState},
    config,
    dualsense::{
        demo_frames, trigger_effect_for_profile, AdaptiveTriggerEffect, DeviceInfo,
        DualSenseBackend, DualSenseControl, HapticFrame, HapticOutput,
    },
    mapping::{open_accessibility_settings, KeyboardMapper, MouseMapper},
    model::{
        AdaptiveTriggerPreset, GamepadState, HapticDemo, MouseMappingProfile, Profile,
        TriggerTarget,
    },
    ui,
};

const AUTO_APPLY_DELAY: Duration = Duration::from_millis(350);
const DAEMON_INPUT_TICK: Duration = Duration::from_millis(20);
const LIGHTBAR_INACTIVE_REAPPLY_INTERVAL: Duration = Duration::from_secs(2);
const LIGHTBAR_INACTIVE_REAPPLY_BURST: [Duration; 3] = [
    Duration::from_millis(50),
    Duration::from_millis(150),
    Duration::from_millis(400),
];

mod gui_service;

#[derive(Clone)]
struct ProfileDraft {
    profile: Profile,
    dirty: bool,
}

pub fn run() -> Result<()> {
    let mut app = ConfiguratorApp::new()?;
    let mut terminal = TerminalSession::start()?;
    let result = run_event_loop(&mut terminal.terminal, &mut app);
    app.shutdown_haptics();
    result
}

/// Runs the controller monitor without creating a terminal UI.
///
/// This is used by the per-user LaunchAgent. It intentionally keeps the
/// process alive: controller output settings are volatile and keyboard/mouse
/// mappings need a live process to synthesize events.
pub fn run_daemon() -> Result<()> {
    // Bind the command socket before opening IOKit. A desktop shell can then
    // wait for this daemon instead of creating a second HID owner.
    let mut command_server = gui_service::DaemonCommandServer::start()?;
    let mut app = ConfiguratorApp::new()?;
    // macOS may briefly restore its own controller LED state after a client
    // starts. Keep the saved colour owned by this one long-lived process,
    // rather than relying on a GUI focus event to restore it later.
    app.enable_daemon_lightbar_keepalive();
    loop {
        app.update_input_state();
        app.tick_audio_reactive_haptics();
        command_server.wait_for_commands(&mut app, DAEMON_INPUT_TICK);
    }
}

/// Runs the JSON service used by the native macOS application shell.
///
/// When the background daemon is active, this service forwards its stdio
/// protocol to it so the daemon remains the only HID owner. Without a daemon,
/// it runs the same backend locally for an app-only session.
pub fn run_gui_service() -> Result<()> {
    gui_service::run()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    Devices,
    Input,
    Sensors,
    Lightbar,
    Haptics,
    Triggers,
    System,
    Mapping,
}

impl Tab {
    pub const ALL: [Self; 8] = [
        Self::Devices,
        Self::Input,
        Self::Sensors,
        Self::Lightbar,
        Self::Haptics,
        Self::Triggers,
        Self::System,
        Self::Mapping,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Devices => "Devices",
            Self::Input => "Input",
            Self::Sensors => "Sensors",
            Self::Lightbar => "Lightbar",
            Self::Haptics => "Haptics",
            Self::Triggers => "Triggers",
            Self::System => "System",
            Self::Mapping => "Mapping",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Devices => 0,
            Self::Input => 1,
            Self::Sensors => 2,
            Self::Lightbar => 3,
            Self::Haptics => 4,
            Self::Triggers => 5,
            Self::System => 6,
            Self::Mapping => 7,
        }
    }

    pub const fn compact_title(self) -> &'static str {
        match self {
            Self::Devices => "Dev",
            Self::Input => "Input",
            Self::Sensors => "Sense",
            Self::Lightbar => "Light",
            Self::Haptics => "Haptic",
            Self::Triggers => "Trig",
            Self::System => "System",
            Self::Mapping => "Map",
        }
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightbarField {
    Red,
    Green,
    Blue,
}

impl LightbarField {
    const ALL: [Self; 3] = [Self::Red, Self::Green, Self::Blue];

    const fn channel_index(self) -> usize {
        match self {
            Self::Red => 0,
            Self::Green => 1,
            Self::Blue => 2,
        }
    }

    fn move_by(self, delta: isize) -> Self {
        Self::ALL[move_index(self.channel_index(), Self::ALL.len(), delta)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HapticField {
    State,
    Mode,
    Strength,
    ReactiveState,
    ReactiveSensitivity,
    ReactiveThreshold,
    Demo,
}

impl HapticField {
    const ALL: [Self; 7] = [
        Self::State,
        Self::Mode,
        Self::Strength,
        Self::ReactiveState,
        Self::ReactiveSensitivity,
        Self::ReactiveThreshold,
        Self::Demo,
    ];

    const fn index(self) -> usize {
        match self {
            Self::State => 0,
            Self::Mode => 1,
            Self::Strength => 2,
            Self::ReactiveState => 3,
            Self::ReactiveSensitivity => 4,
            Self::ReactiveThreshold => 5,
            Self::Demo => 6,
        }
    }

    fn move_by(self, delta: isize) -> Self {
        Self::ALL[move_index(self.index(), Self::ALL.len(), delta)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerField {
    Target,
    Mode,
    Intensity,
    StartPosition,
    EndPosition,
    Frequency,
    Presets,
}

impl TriggerField {
    const ALL: [Self; 7] = [
        Self::Target,
        Self::Mode,
        Self::Intensity,
        Self::StartPosition,
        Self::EndPosition,
        Self::Frequency,
        Self::Presets,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Target => 0,
            Self::Mode => 1,
            Self::Intensity => 2,
            Self::StartPosition => 3,
            Self::EndPosition => 4,
            Self::Frequency => 5,
            Self::Presets => 6,
        }
    }

    fn move_by(self, delta: isize) -> Self {
        Self::ALL[move_index(self.index(), Self::ALL.len(), delta)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemField {
    PlayerIndicator,
    MicrophoneMute,
    SpeakerVolume,
    MicrophoneVolume,
    AudioRoute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseField {
    Enabled,
    PointerSpeed,
    Deadzone,
    ScrollSpeed,
}

impl MouseField {
    const ALL: [Self; 4] = [
        Self::Enabled,
        Self::PointerSpeed,
        Self::Deadzone,
        Self::ScrollSpeed,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Enabled => 0,
            Self::PointerSpeed => 1,
            Self::Deadzone => 2,
            Self::ScrollSpeed => 3,
        }
    }

    fn move_by(self, delta: isize) -> Self {
        Self::ALL[move_index(self.index(), Self::ALL.len(), delta)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingView {
    ControllerProfile,
    KeyboardOutput,
    MouseOutput,
}

impl MappingView {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ControllerProfile => "Controller profile",
            Self::KeyboardOutput => "Keyboard output",
            Self::MouseOutput => "Mouse output",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::ControllerProfile => Self::KeyboardOutput,
            Self::KeyboardOutput => Self::MouseOutput,
            Self::MouseOutput => Self::ControllerProfile,
        }
    }
}

impl SystemField {
    const ALL: [Self; 5] = [
        Self::PlayerIndicator,
        Self::MicrophoneMute,
        Self::SpeakerVolume,
        Self::MicrophoneVolume,
        Self::AudioRoute,
    ];

    const fn index(self) -> usize {
        match self {
            Self::PlayerIndicator => 0,
            Self::MicrophoneMute => 1,
            Self::SpeakerVolume => 2,
            Self::MicrophoneVolume => 3,
            Self::AudioRoute => 4,
        }
    }

    fn move_by(self, delta: isize) -> Self {
        Self::ALL[move_index(self.index(), Self::ALL.len(), delta)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PanelLayout {
    pub devices_size: u16,
    pub status_size: u16,
    pub controls_height: u16,
}

impl PanelLayout {
    const MIN_DEVICES_SIZE: u16 = 20;
    const MAX_DEVICES_SIZE: u16 = 48;
    const MIN_STATUS_SIZE: u16 = 24;
    const MAX_STATUS_SIZE: u16 = 56;
    const MIN_CONTROLS_HEIGHT: u16 = 4;
    const MAX_CONTROLS_HEIGHT: u16 = 8;

    fn adjust_devices(&mut self, delta: i16) {
        self.devices_size = adjust_u16(
            self.devices_size,
            delta,
            Self::MIN_DEVICES_SIZE,
            Self::MAX_DEVICES_SIZE,
        );
    }

    fn adjust_status(&mut self, delta: i16) {
        self.status_size = adjust_u16(
            self.status_size,
            delta,
            Self::MIN_STATUS_SIZE,
            Self::MAX_STATUS_SIZE,
        );
    }

    fn adjust_controls(&mut self, delta: i16) {
        self.controls_height = adjust_u16(
            self.controls_height,
            delta,
            Self::MIN_CONTROLS_HEIGHT,
            Self::MAX_CONTROLS_HEIGHT,
        );
    }
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            devices_size: 32,
            status_size: 36,
            controls_height: 6,
        }
    }
}

pub struct ConfiguratorApp {
    backend: DualSenseBackend,
    keyboard_mapper: KeyboardMapper,
    mouse_mapper: MouseMapper,
    profile_drafts: HashMap<String, ProfileDraft>,
    active_profile_key: String,
    auto_apply_at: Option<Instant>,
    lightbar_reapply_at: Option<Instant>,
    lightbar_keepalive_while_inactive: bool,
    lightbar_reapply_burst_index: usize,
    pub audio_reactive: AudioReactiveHaptics,
    audio_reactive_device: Option<usize>,
    pub profile: Profile,
    pub layout: PanelLayout,
    pub devices: Vec<DeviceInfo>,
    pub selected_device: usize,
    pub active_tab: Tab,
    pub selected_lightbar_field: LightbarField,
    pub selected_haptic_field: HapticField,
    pub selected_haptic_demo: HapticDemo,
    pub selected_trigger_field: TriggerField,
    pub selected_system_field: SystemField,
    pub selected_mapping: usize,
    pub selected_keyboard_mapping: usize,
    pub selected_mouse_field: MouseField,
    pub mapping_view: MappingView,
    pub live_input: Option<GamepadState>,
    pub input_status: String,
    pub mapping_status: String,
    pub mouse_mapping_status: String,
    pub status: String,
    pub profile_path: String,
    pub dirty: bool,
    pub should_quit: bool,
}

impl ConfiguratorApp {
    fn new() -> Result<Self> {
        let backend = DualSenseBackend::new()?;
        let keyboard_mapper = KeyboardMapper::new();
        let mouse_mapper = MouseMapper::new();
        let mapping_status = format!(
            "Keyboard output disabled ({})",
            keyboard_mapper.permission_status()
        );
        let mouse_mapping_status = format!(
            "Mouse output disabled ({})",
            mouse_mapper.permission_status()
        );

        let devices = backend.devices();
        let selected_device = 0;
        let selected_mac = devices
            .get(selected_device)
            .and_then(|device| device.mac_address.as_deref());
        let profile = config::load_profile_for_device(selected_mac)?;
        let active_profile_key = profile_key(selected_mac);
        let profile_path = profile_path_for_device(selected_mac);
        let status = if devices.is_empty() {
            "No DualSense detected".to_string()
        } else {
            format!(
                "Detected {} DualSense device(s); saved profiles will auto-apply",
                devices.len()
            )
        };

        Ok(Self {
            backend,
            keyboard_mapper,
            mouse_mapper,
            profile_drafts: HashMap::new(),
            active_profile_key,
            auto_apply_at: Some(Instant::now() + AUTO_APPLY_DELAY),
            lightbar_reapply_at: None,
            lightbar_keepalive_while_inactive: false,
            lightbar_reapply_burst_index: 0,
            audio_reactive: AudioReactiveHaptics::default(),
            audio_reactive_device: None,
            profile,
            layout: PanelLayout::default(),
            devices,
            selected_device,
            active_tab: Tab::Devices,
            selected_lightbar_field: LightbarField::Red,
            selected_haptic_field: HapticField::State,
            selected_haptic_demo: HapticDemo::Click,
            selected_trigger_field: TriggerField::Target,
            selected_system_field: SystemField::PlayerIndicator,
            selected_mapping: 0,
            selected_keyboard_mapping: 0,
            selected_mouse_field: MouseField::Enabled,
            mapping_view: MappingView::ControllerProfile,
            live_input: None,
            input_status: "Waiting for input reports".to_string(),
            mapping_status,
            mouse_mapping_status,
            status,
            profile_path,
            dirty: false,
            should_quit: false,
        })
    }

    fn refresh_devices(&mut self) {
        let _ = self.stop_audio_reactive_haptics();
        let previously_selected_mac = self.selected_device_mac().map(str::to_owned);
        match self.backend.refresh() {
            Ok(()) => {
                self.devices = self.backend.devices();
                if let Some(mac) = previously_selected_mac.as_deref() {
                    if let Some(index) = self.devices.iter().position(|device| {
                        device
                            .mac_address
                            .as_deref()
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(mac))
                    }) {
                        self.selected_device = index;
                    } else if self.selected_device >= self.devices.len() {
                        self.selected_device = self.devices.len().saturating_sub(1);
                    }
                } else if self.selected_device >= self.devices.len() {
                    self.selected_device = self.devices.len().saturating_sub(1);
                }
                self.release_output_mappings();
                self.live_input = None;
                self.input_status = "Waiting for input reports".to_string();
                let profile_result = self.activate_selected_profile();
                self.schedule_auto_apply();
                self.status = if self.devices.is_empty() {
                    "No DualSense detected".to_string()
                } else {
                    format!("Refreshed: {} DualSense device(s)", self.devices.len())
                };
                if let Err(error) = profile_result {
                    self.status = format!("{}; profile load failed: {error:#}", self.status);
                }
            }
            Err(error) => {
                self.status = format!("Refresh failed: {error:#}");
            }
        }
    }

    fn update_input_state(&mut self) {
        if let Err(error) = self.backend.pump_events() {
            self.input_status = format!("Input event pump unavailable: {error:#}");
            self.apply_scheduled_lightbar_reapply();
            return;
        }

        if self.backend.take_device_change() {
            self.refresh_devices();
        }

        if self.devices.is_empty() {
            let _ = self.stop_audio_reactive_haptics();
            self.release_output_mappings();
            self.live_input = None;
            self.input_status = "No DualSense selected".to_string();
            self.apply_scheduled_profiles();
            self.apply_scheduled_lightbar_reapply();
            return;
        }

        match self.backend.read_state(self.selected_device) {
            Ok(Some(state)) => {
                self.input_status = format!("Live input packets: {}", state.packet_count);
                self.sync_keyboard_mapping(&state);
                self.sync_mouse_mapping(&state);
                self.live_input = Some(state);
            }
            Ok(None) => {
                self.input_status = "Waiting for input reports".to_string();
            }
            Err(error) => {
                let _ = self.stop_audio_reactive_haptics();
                self.live_input = None;
                self.input_status = format!("Input unavailable: {error:#}");
            }
        }

        self.apply_scheduled_profiles();
        self.apply_scheduled_lightbar_reapply();
    }

    fn selected_device_mac(&self) -> Option<&str> {
        self.devices
            .get(self.selected_device)
            .and_then(|device| device.mac_address.as_deref())
    }

    fn activate_selected_profile(&mut self) -> Result<()> {
        let mac = self.selected_device_mac().map(str::to_owned);
        let next_key = profile_key(mac.as_deref());
        if next_key == self.active_profile_key {
            self.profile_path = profile_path_for_device(mac.as_deref());
            return Ok(());
        }

        let next_profile = if let Some(draft) = self.profile_drafts.get(&next_key) {
            draft.clone()
        } else {
            ProfileDraft {
                profile: config::load_profile_for_device(mac.as_deref())?,
                dirty: false,
            }
        };

        self.profile_drafts.insert(
            self.active_profile_key.clone(),
            ProfileDraft {
                profile: self.profile.clone(),
                dirty: self.dirty,
            },
        );
        self.active_profile_key = next_key;
        self.profile = next_profile.profile;
        self.dirty = next_profile.dirty;
        self.profile_path = profile_path_for_device(mac.as_deref());
        Ok(())
    }

    fn sync_keyboard_mapping(&mut self, state: &GamepadState) {
        match self
            .keyboard_mapper
            .sync(state, &self.profile.keyboard_mapping)
        {
            Ok(()) => {
                self.mapping_status = if self.profile.keyboard_mapping.enabled {
                    "Keyboard output active; switch focus to the target app".to_string()
                } else {
                    format!(
                        "Keyboard output disabled ({})",
                        self.keyboard_mapper.permission_status()
                    )
                };
            }
            Err(error) => {
                self.profile.keyboard_mapping.enabled = false;
                self.mapping_status = format!("Keyboard output disabled: {error:#}");
                self.status = self.mapping_status.clone();
            }
        }
    }

    fn release_keyboard_mapping(&mut self) {
        if let Err(error) = self.keyboard_mapper.release_all() {
            self.mapping_status = format!("Keyboard release failed: {error:#}");
        }
    }

    fn sync_mouse_mapping(&mut self, state: &GamepadState) {
        match self.mouse_mapper.sync(state, &self.profile.mouse_mapping) {
            Ok(()) => {
                self.mouse_mapping_status = if self.profile.mouse_mapping.enabled {
                    "Mouse output active; switch focus to the target app".to_string()
                } else {
                    format!(
                        "Mouse output disabled ({})",
                        self.mouse_mapper.permission_status()
                    )
                };
            }
            Err(error) => {
                self.profile.mouse_mapping.enabled = false;
                self.mouse_mapping_status = format!("Mouse output disabled: {error:#}");
                self.status = self.mouse_mapping_status.clone();
            }
        }
    }

    fn release_mouse_mapping(&mut self) {
        if let Err(error) = self.mouse_mapper.release_all() {
            self.mouse_mapping_status = format!("Mouse release failed: {error:#}");
        }
    }

    fn release_output_mappings(&mut self) {
        self.release_keyboard_mapping();
        self.release_mouse_mapping();
    }

    fn toggle_audio_reactive_haptics(&mut self) {
        if self.audio_reactive.state().is_running() {
            match self.stop_audio_reactive_haptics() {
                Ok(()) => self.status = "Audio-reactive haptics stopped".to_string(),
                Err(error) => {
                    self.status =
                        format!("Audio-reactive haptics stopped with HID error: {error:#}")
                }
            }
            return;
        }

        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }
        if !self.profile.haptics.enabled {
            self.status = "Enable haptics before starting audio-reactive output".to_string();
            return;
        }

        let state = self.audio_reactive.start(Instant::now());
        if state.is_running() {
            self.audio_reactive_device = Some(self.selected_device);
            self.status = "Audio-reactive haptics started from system audio".to_string();
        } else {
            self.audio_reactive_device = None;
            self.status = audio_capture_status_message(state);
        }
    }

    fn stop_audio_reactive_haptics(&mut self) -> Result<()> {
        let send_off = self.audio_reactive.stop();
        let target = self.audio_reactive_device.take();
        if send_off {
            if let Some(index) = target {
                self.backend.set_haptics(
                    index,
                    HapticOutput::OFF,
                    self.profile.haptics.audio_haptics,
                )?;
            }
        }
        Ok(())
    }

    fn tick_audio_reactive_haptics(&mut self) {
        let Some(index) = self.audio_reactive_device else {
            return;
        };

        if !self.profile.haptics.enabled {
            if let Err(error) = self.stop_audio_reactive_haptics() {
                self.status = format!("Audio-reactive haptics stopped with HID error: {error:#}");
            } else {
                self.status =
                    "Audio-reactive haptics stopped because haptics are disabled".to_string();
            }
            return;
        }

        let state_before_tick = self.audio_reactive.state();
        let output = self.audio_reactive.tick(
            Instant::now(),
            &self.profile.haptics.audio_reactive,
            self.profile.haptics.strength(),
        );
        let state_after_tick = self.audio_reactive.state();

        if let Some(output) = output {
            if let Err(error) =
                self.backend
                    .set_haptics(index, output, self.profile.haptics.audio_haptics)
            {
                self.audio_reactive.stop();
                self.audio_reactive_device = None;
                self.status = format!("Audio-reactive haptics stopped: {error:#}");
                return;
            }
        }

        if state_before_tick.is_running() && !state_after_tick.is_running() {
            self.audio_reactive_device = None;
            self.status = audio_capture_status_message(state_after_tick);
        }
    }

    fn shutdown_haptics(&mut self) {
        let _ = self.stop_audio_reactive_haptics();
    }

    fn save_profile(&mut self) {
        let mac = self.selected_device_mac().map(str::to_owned);
        match config::save_profile_for_device(mac.as_deref(), &self.profile) {
            Ok(path) => {
                self.dirty = false;
                self.profile_path = path.display().to_string();
                self.profile_drafts.insert(
                    self.active_profile_key.clone(),
                    ProfileDraft {
                        profile: self.profile.clone(),
                        dirty: false,
                    },
                );
                self.status = format!("Saved {}", path.display());
            }
            Err(error) => {
                self.status = format!("Save failed: {error:#}");
            }
        }
    }

    fn set_lightbar_inactive(&mut self, inactive: bool) {
        if self.lightbar_keepalive_while_inactive == inactive {
            return;
        }

        self.lightbar_keepalive_while_inactive = inactive;
        self.lightbar_reapply_burst_index = 0;
        self.lightbar_reapply_at = None;

        if inactive {
            self.start_lightbar_reapply_burst();
        }
    }

    fn reapply_lightbar_after_app_resign(&mut self) {
        if self.lightbar_keepalive_while_inactive {
            self.start_lightbar_reapply_burst();
        }
    }

    fn enable_daemon_lightbar_keepalive(&mut self) {
        self.lightbar_keepalive_while_inactive = true;
        self.lightbar_reapply_burst_index = 0;
        self.lightbar_reapply_at = None;
        self.schedule_next_inactive_lightbar_reapply();
    }

    fn save_named_profile(&mut self, name: &str) {
        match config::save_named_profile(name, &self.profile) {
            Ok(profile) => {
                self.status = format!("Saved profile '{}' to the library", profile.name);
            }
            Err(error) => {
                self.status = format!("Profile library save failed: {error:#}");
            }
        }
    }

    fn load_named_profile(&mut self, id: &str) {
        match config::load_named_profile(id) {
            Ok((profile_descriptor, profile)) => {
                self.replace_active_profile(
                    profile,
                    format!("Loaded profile '{}'", profile_descriptor.name),
                );
            }
            Err(error) => {
                self.status = format!("Profile load failed: {error:#}");
            }
        }
    }

    fn reset_profile_to_defaults(&mut self) {
        self.replace_active_profile(Profile::default(), "Profile reset to defaults".to_string());
    }

    fn replace_active_profile(&mut self, profile: Profile, action: String) {
        // A profile choice is most often a visible colour choice. Send its
        // RGB report first, before stopping haptics or applying triggers, so
        // the controller responds without waiting for unrelated output.
        let lightbar_error = if self.devices.is_empty() {
            None
        } else {
            self.backend
                .set_lightbar(self.selected_device, profile.lightbar)
                .err()
        };
        let stop_error = self.stop_audio_reactive_haptics().err();
        let haptics_stop_error = if self.devices.is_empty() {
            None
        } else {
            self.backend
                .set_haptics(
                    self.selected_device,
                    HapticOutput::OFF,
                    self.profile.haptics.audio_haptics,
                )
                .err()
        };
        self.release_output_mappings();
        self.profile = profile;
        self.dirty = true;
        self.mapping_status = if self.profile.keyboard_mapping.enabled {
            "Keyboard output will activate on controller input".to_string()
        } else {
            format!(
                "Keyboard output disabled ({})",
                self.keyboard_mapper.permission_status()
            )
        };
        self.mouse_mapping_status = if self.profile.mouse_mapping.enabled {
            "Mouse output will activate on controller input".to_string()
        } else {
            format!(
                "Mouse output disabled ({})",
                self.mouse_mapper.permission_status()
            )
        };
        self.profile_drafts.insert(
            self.active_profile_key.clone(),
            ProfileDraft {
                profile: self.profile.clone(),
                dirty: true,
            },
        );

        let mut status = action;
        if let Some(error) = stop_error {
            status.push_str(&format!("; audio-reactive haptics stop failed: {error:#}"));
        }
        if let Some(error) = haptics_stop_error {
            status.push_str(&format!("; haptics stop failed: {error:#}"));
        }
        if let Some(error) = lightbar_error {
            status.push_str(&format!("; lightbar switch failed: {error:#}"));
        }

        if self.devices.is_empty() {
            status.push_str("; no controller selected");
        } else {
            let profile = self.profile.clone();
            match self.apply_profile_controls_to_device(self.selected_device, &profile) {
                Ok(()) => status.push_str("; applied to the selected controller"),
                Err(error) => status.push_str(&format!("; apply failed: {error:#}")),
            }
        }

        status.push_str("; save to persist");
        self.status = status;
    }

    fn schedule_auto_apply(&mut self) {
        self.auto_apply_at = Some(Instant::now() + AUTO_APPLY_DELAY);
    }

    fn apply_scheduled_profiles(&mut self) {
        let Some(apply_at) = self.auto_apply_at else {
            return;
        };
        if Instant::now() < apply_at {
            return;
        }

        self.auto_apply_at = None;
        self.apply_saved_profiles();
    }

    fn apply_scheduled_lightbar_reapply(&mut self) {
        let Some(apply_at) = self.lightbar_reapply_at else {
            return;
        };
        if Instant::now() < apply_at {
            return;
        }

        self.lightbar_reapply_at = None;
        self.reapply_lightbar_after_focus_loss();

        if self.lightbar_keepalive_while_inactive && !self.devices.is_empty() {
            self.schedule_next_inactive_lightbar_reapply();
        }
    }

    fn reapply_lightbar_after_focus_loss(&mut self) {
        if self.devices.is_empty() {
            return;
        }
        if let Err(error) = self
            .backend
            .reapply_lightbar(self.selected_device, self.profile.lightbar)
        {
            self.status = format!("Lightbar reapply failed after focus loss: {error:#}");
        }
    }

    fn start_lightbar_reapply_burst(&mut self) {
        self.lightbar_reapply_burst_index = 0;
        self.lightbar_reapply_at = None;
        self.reapply_lightbar_after_focus_loss();
        self.schedule_next_inactive_lightbar_reapply();
    }

    fn schedule_next_inactive_lightbar_reapply(&mut self) {
        let (delay, next_burst_index) =
            inactive_lightbar_reapply_delay(self.lightbar_reapply_burst_index);
        self.lightbar_reapply_burst_index = next_burst_index;
        self.lightbar_reapply_at = Some(Instant::now() + delay);
    }

    fn apply_saved_profiles(&mut self) {
        let mac_addresses = self
            .devices
            .iter()
            .map(|device| device.mac_address.clone())
            .collect::<Vec<_>>();
        let has_global_profile = config::profile_path().is_file();
        let mut applied = 0;
        let mut errors = Vec::new();

        for (index, mac) in mac_addresses.iter().enumerate() {
            let has_saved_profile = mac
                .as_deref()
                .filter(|address| !address.trim().is_empty())
                .is_some_and(config::device_profile_exists)
                || has_global_profile;
            let is_active_profile = index == self.selected_device
                && self.active_profile_key == profile_key(mac.as_deref());
            // A daemon may be running while the TUI saves an updated profile.
            // Reload saved data here instead of retaining the startup copy; only
            // an unsaved edit in this process takes precedence temporarily.
            let profile = if is_active_profile && self.dirty {
                Some(self.profile.clone())
            } else if has_saved_profile {
                match config::load_profile_for_device(mac.as_deref()) {
                    Ok(profile) => Some(profile),
                    Err(error) => {
                        errors.push(format!("profile for {}: {error:#}", device_label(mac)));
                        None
                    }
                }
            } else {
                None
            };

            let Some(profile) = profile else {
                continue;
            };

            if is_active_profile && !self.dirty {
                self.profile = profile.clone();
                self.profile_drafts.insert(
                    self.active_profile_key.clone(),
                    ProfileDraft {
                        profile: profile.clone(),
                        dirty: false,
                    },
                );
            }

            match self.apply_profile_to_device(index, &profile) {
                Ok(()) => applied += 1,
                Err(error) => errors.push(format!("{}: {error:#}", device_label(mac))),
            }
        }

        if !errors.is_empty() {
            self.status = format!("Automatic apply failed: {}", errors.join("; "));
        } else if applied > 0 {
            self.status = format!("Automatically applied saved profile to {applied} controller(s)");
        }
    }

    fn apply_profile_to_device(&mut self, index: usize, profile: &Profile) -> Result<()> {
        let mut failures = Vec::new();

        if let Err(error) = self.backend.set_lightbar(index, profile.lightbar) {
            failures.push(format!("lightbar: {error:#}"));
        }

        match self.apply_profile_controls_to_device(index, profile) {
            Ok(()) => {}
            Err(error) => failures.push(format!("controls: {error:#}")),
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(failures.join(", ")))
        }
    }

    fn apply_profile_controls_to_device(&mut self, index: usize, profile: &Profile) -> Result<()> {
        let mut failures = Vec::new();

        let trigger_effect = trigger_effect_for_profile(&profile.adaptive_triggers);
        let (left_trigger, right_trigger) =
            trigger_effects_for(profile.adaptive_triggers.target, trigger_effect);
        if let Err(error) = self
            .backend
            .set_adaptive_triggers(index, left_trigger, right_trigger)
        {
            failures.push(format!("adaptive triggers: {error:#}"));
        }

        if let Err(error) = self.backend.set_system_controls(index, &profile.system) {
            failures.push(format!("system controls: {error:#}"));
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(failures.join(", ")))
        }
    }

    fn apply_lightbar(&mut self) {
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }

        match self
            .backend
            .set_lightbar(self.selected_device, self.profile.lightbar)
        {
            Ok(()) => {
                let color = self.profile.lightbar;
                let mac = self.selected_device_mac().map(str::to_owned);
                match config::save_lightbar_for_device(mac.as_deref(), color) {
                    Ok(path) => {
                        self.profile_path = path.display().to_string();
                        match config::load_profile_for_device(mac.as_deref()) {
                            Ok(saved_profile) => self.dirty = self.profile != saved_profile,
                            Err(_) => self.dirty = true,
                        }
                        self.status = format!(
                            "Lightbar applied and saved: #{:02x}{:02x}{:02x}",
                            color.r, color.g, color.b
                        );
                    }
                    Err(error) => {
                        self.dirty = true;
                        self.status =
                            format!("Lightbar applied, but color could not be saved: {error:#}");
                    }
                }
            }
            Err(error) => {
                self.status = format!("Lightbar failed: {error:#}");
            }
        }
    }

    fn pulse_haptics(&mut self) {
        if let Err(error) = self.stop_audio_reactive_haptics() {
            self.status = format!("Could not stop audio-reactive haptics: {error:#}");
            return;
        }
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }
        if !self.profile.haptics.enabled {
            self.status = "Haptics are disabled".to_string();
            return;
        }

        let frame = HapticFrame::symmetric(self.profile.haptics.strength(), 180);
        match self.backend.pulse_haptics(
            self.selected_device,
            frame.left,
            frame.right,
            self.profile.haptics.audio_haptics,
        ) {
            Ok(()) => {
                self.status = "Haptic pulse sent".to_string();
            }
            Err(error) => {
                self.status = format!("Haptics failed: {error:#}");
            }
        }
    }

    fn play_haptic_demo(&mut self) {
        if let Err(error) = self.stop_audio_reactive_haptics() {
            self.status = format!("Could not stop audio-reactive haptics: {error:#}");
            return;
        }
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }
        if !self.profile.haptics.enabled {
            self.status = "Haptics are disabled".to_string();
            return;
        }

        let demo = self.selected_haptic_demo;
        let frames = demo_frames(demo);
        match self.backend.play_haptics(
            self.selected_device,
            &frames,
            self.profile.haptics.audio_haptics,
        ) {
            Ok(()) => {
                self.status = format!("Haptics demo played: {}", demo.label());
            }
            Err(error) => {
                self.status = format!("Haptics demo failed: {error:#}");
            }
        }
    }

    fn apply_adaptive_triggers(&mut self) {
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }

        let trigger_profile = &self.profile.adaptive_triggers;
        let description = trigger_effect_description(trigger_profile);
        let effect = trigger_effect_for_profile(trigger_profile);
        let result = self.send_trigger_effect(effect);

        match result {
            Ok(()) => {
                self.status = format!(
                    "Adaptive triggers applied: {} on {}",
                    description,
                    self.profile.adaptive_triggers.target.label()
                );
            }
            Err(error) => {
                self.status = format!("Adaptive triggers failed: {error:#}");
            }
        }
    }

    fn apply_system_controls(&mut self) {
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }

        match self
            .backend
            .set_system_controls(self.selected_device, &self.profile.system)
        {
            Ok(()) => {
                self.status = format!(
                    "System controls applied: {}, mic {}, route {}",
                    self.profile.system.player_indicator.label(),
                    if self.profile.system.microphone_muted {
                        "muted"
                    } else {
                        "live"
                    },
                    self.profile.system.audio_route.label()
                );
            }
            Err(error) => {
                self.status = format!("System controls failed: {error:#}");
            }
        }
    }

    fn send_trigger_effect(&mut self, effect: AdaptiveTriggerEffect) -> Result<()> {
        let (left, right) = trigger_effects_for(self.profile.adaptive_triggers.target, effect);
        self.backend
            .set_adaptive_triggers(self.selected_device, left, right)
    }

    fn reset_adaptive_triggers(&mut self) {
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }

        match self.backend.set_adaptive_triggers(
            self.selected_device,
            AdaptiveTriggerEffect::off(),
            AdaptiveTriggerEffect::off(),
        ) {
            Ok(()) => {
                self.profile.adaptive_triggers.preset = AdaptiveTriggerPreset::Off;
                self.dirty = true;
                self.status = "Adaptive triggers reset".to_string();
            }
            Err(error) => {
                self.status = format!("Adaptive trigger reset failed: {error:#}");
            }
        }
    }

    fn select_tab(&mut self, tab: Tab) {
        self.active_tab = tab;
    }

    fn next_tab(&mut self) {
        self.active_tab = Tab::from_index(self.active_tab.index() + 1);
    }

    fn previous_tab(&mut self) {
        self.active_tab = Tab::from_index(self.active_tab.index() + Tab::ALL.len() - 1);
    }

    fn move_selection(&mut self, delta: isize) {
        match self.active_tab {
            Tab::Devices => {
                let previous_device = self.selected_device;
                let next_device = move_index(self.selected_device, self.devices.len(), delta);
                if next_device != previous_device {
                    let _ = self.stop_audio_reactive_haptics();
                }
                self.selected_device = next_device;
                self.live_input = None;
                if self.selected_device != previous_device {
                    self.release_output_mappings();
                    if let Err(error) = self.activate_selected_profile() {
                        self.status = format!("Profile load failed: {error:#}");
                    } else {
                        self.status = "Selected controller profile".to_string();
                    }
                }
            }
            Tab::Input => {}
            Tab::Sensors => {}
            Tab::Lightbar => {
                self.selected_lightbar_field = self.selected_lightbar_field.move_by(delta);
            }
            Tab::Haptics => {
                self.selected_haptic_field = self.selected_haptic_field.move_by(delta);
            }
            Tab::Triggers => {
                self.selected_trigger_field = self.selected_trigger_field.move_by(delta);
            }
            Tab::System => {
                self.selected_system_field = self.selected_system_field.move_by(delta);
            }
            Tab::Mapping => match self.mapping_view {
                MappingView::ControllerProfile => {
                    self.selected_mapping =
                        move_index(self.selected_mapping, self.profile.mappings.len(), delta);
                }
                MappingView::KeyboardOutput => {
                    self.selected_keyboard_mapping = move_index(
                        self.selected_keyboard_mapping,
                        self.profile.keyboard_mapping.bindings.len(),
                        delta,
                    );
                }
                MappingView::MouseOutput => {
                    self.selected_mouse_field = self.selected_mouse_field.move_by(delta);
                }
            },
        }
    }

    fn adjust_active(&mut self, delta: i16) {
        match self.active_tab {
            Tab::Devices => {
                self.move_selection(isize::from(delta.signum()));
            }
            Tab::Input => {}
            Tab::Sensors => {}
            Tab::Lightbar => {
                if delta != 0 {
                    self.profile
                        .lightbar
                        .adjust_channel(self.selected_lightbar_field.channel_index(), delta);
                    self.dirty = true;
                }
            }
            Tab::Haptics => match self.selected_haptic_field {
                HapticField::State => {
                    if delta != 0 {
                        self.toggle_haptics_enabled();
                    }
                }
                HapticField::Strength => {
                    self.profile
                        .haptics
                        .set_strength(adjust_u8(self.profile.haptics.strength(), delta));
                    if delta != 0 {
                        self.dirty = true;
                    }
                }
                HapticField::Mode => {
                    if delta != 0 {
                        self.toggle_audio_haptics();
                    }
                }
                HapticField::ReactiveState => {
                    if delta != 0 {
                        self.toggle_audio_reactive_haptics();
                    }
                }
                HapticField::ReactiveSensitivity => {
                    if delta != 0 {
                        self.profile.haptics.audio_reactive.sensitivity_percent = adjust_u8(
                            self.profile.haptics.audio_reactive.sensitivity_percent,
                            delta,
                        )
                        .clamp(25, 250);
                        self.dirty = true;
                    }
                }
                HapticField::ReactiveThreshold => {
                    if delta != 0 {
                        self.profile.haptics.audio_reactive.threshold_percent =
                            adjust_u8(self.profile.haptics.audio_reactive.threshold_percent, delta)
                                .min(90);
                        self.dirty = true;
                    }
                }
                HapticField::Demo => {
                    if delta > 0 {
                        self.selected_haptic_demo = self.selected_haptic_demo.next();
                    } else if delta < 0 {
                        self.selected_haptic_demo = self.selected_haptic_demo.previous();
                    }
                }
            },
            Tab::Triggers => {
                adjust_trigger_field(&mut self.profile, self.selected_trigger_field, delta);
                if delta != 0 {
                    self.dirty = true;
                }
            }
            Tab::System => {
                self.adjust_system_field(delta);
            }
            Tab::Mapping => match self.mapping_view {
                MappingView::ControllerProfile => {
                    if let Some(mapping) = self.profile.mappings.get_mut(self.selected_mapping) {
                        mapping.to = if delta > 0 {
                            mapping.to.next()
                        } else if delta < 0 {
                            mapping.to.previous()
                        } else {
                            mapping.to
                        };
                        if delta != 0 {
                            self.dirty = true;
                        }
                    }
                }
                MappingView::KeyboardOutput => {
                    if let Some(binding) = self
                        .profile
                        .keyboard_mapping
                        .bindings
                        .get_mut(self.selected_keyboard_mapping)
                    {
                        binding.to = if delta > 0 {
                            binding.to.next()
                        } else if delta < 0 {
                            binding.to.previous()
                        } else {
                            binding.to
                        };
                        if delta != 0 {
                            self.dirty = true;
                        }
                    }
                }
                MappingView::MouseOutput => self.adjust_mouse_mapping_field(delta),
            },
        }
    }

    fn toggle_active(&mut self) {
        match self.active_tab {
            Tab::Haptics => match self.selected_haptic_field {
                HapticField::State => self.toggle_haptics_enabled(),
                HapticField::Mode => self.toggle_audio_haptics(),
                HapticField::ReactiveState => self.toggle_audio_reactive_haptics(),
                HapticField::Demo => self.play_haptic_demo(),
                HapticField::Strength
                | HapticField::ReactiveSensitivity
                | HapticField::ReactiveThreshold => {}
            },
            Tab::Mapping => match self.mapping_view {
                MappingView::ControllerProfile => {
                    if let Some(mapping) = self.profile.mappings.get_mut(self.selected_mapping) {
                        mapping.to = mapping.from;
                        self.dirty = true;
                    }
                }
                MappingView::KeyboardOutput => {
                    if let Some(binding) = self
                        .profile
                        .keyboard_mapping
                        .bindings
                        .get_mut(self.selected_keyboard_mapping)
                    {
                        binding.to = crate::model::KeyboardKey::Disabled;
                        self.dirty = true;
                    }
                }
                MappingView::MouseOutput => self.toggle_mouse_mapping(),
            },
            Tab::System => {
                if self.selected_system_field == SystemField::MicrophoneMute {
                    self.toggle_microphone_mute();
                }
            }
            Tab::Devices | Tab::Input | Tab::Sensors | Tab::Lightbar | Tab::Triggers => {}
        }
    }

    fn toggle_haptics_enabled(&mut self) {
        if self.profile.haptics.enabled {
            if let Err(error) = self.stop_audio_reactive_haptics() {
                self.status = format!("Could not stop audio-reactive haptics: {error:#}");
                return;
            }
        }
        self.profile.haptics.enabled = !self.profile.haptics.enabled;
        self.dirty = true;
        self.status = if self.profile.haptics.enabled {
            "Haptics enabled".to_string()
        } else {
            "Haptics disabled".to_string()
        };
    }

    fn toggle_audio_haptics(&mut self) {
        if let Err(error) = self.stop_audio_reactive_haptics() {
            self.status = format!("Could not stop audio-reactive haptics: {error:#}");
            return;
        }
        self.profile.haptics.audio_haptics = !self.profile.haptics.audio_haptics;
        self.dirty = true;
        self.status = if self.profile.haptics.audio_haptics {
            "Haptics mode: audio-haptics".to_string()
        } else {
            "Haptics mode: compat rumble".to_string()
        };
    }

    fn adjust_system_field(&mut self, delta: i16) {
        if delta == 0 {
            return;
        }

        match self.selected_system_field {
            SystemField::PlayerIndicator => {
                self.profile.system.player_indicator = if delta > 0 {
                    self.profile.system.player_indicator.next()
                } else {
                    self.profile.system.player_indicator.previous()
                };
            }
            SystemField::MicrophoneMute => self.toggle_microphone_mute(),
            SystemField::SpeakerVolume => {
                self.profile.system.speaker_volume =
                    adjust_u8(self.profile.system.speaker_volume, delta);
            }
            SystemField::MicrophoneVolume => {
                self.profile.system.microphone_volume =
                    adjust_u8(self.profile.system.microphone_volume, delta).min(0x40);
            }
            SystemField::AudioRoute => {
                self.profile.system.audio_route = if delta > 0 {
                    self.profile.system.audio_route.next()
                } else {
                    self.profile.system.audio_route.previous()
                };
            }
        }
        self.dirty = true;
    }

    fn toggle_microphone_mute(&mut self) {
        self.profile.system.microphone_muted = !self.profile.system.microphone_muted;
        self.dirty = true;
        self.status = if self.profile.system.microphone_muted {
            "Microphone will be muted when system controls are applied".to_string()
        } else {
            "Microphone will be enabled when system controls are applied".to_string()
        };
    }

    fn toggle_mapping_view(&mut self) {
        self.mapping_view = self.mapping_view.next();
        self.status = format!("Mapping view: {}", self.mapping_view.label());
    }

    fn toggle_keyboard_mapping(&mut self) {
        if self.profile.keyboard_mapping.enabled {
            self.profile.keyboard_mapping.enabled = false;
            self.release_keyboard_mapping();
            self.mapping_status = format!(
                "Keyboard output disabled ({})",
                self.keyboard_mapper.permission_status()
            );
            self.status = "Keyboard output disabled".to_string();
            self.dirty = true;
            return;
        }

        if !self.keyboard_mapper.can_post_events() {
            let access_granted = self.keyboard_mapper.request_event_posting_access();
            self.mapping_status = format!(
                "Keyboard output unavailable ({})",
                self.keyboard_mapper.permission_status()
            );
            if access_granted {
                self.status =
                    "Accessibility access granted; press k again to enable keyboard output"
                        .to_string();
            } else {
                self.open_accessibility_settings();
            }
            return;
        }

        self.profile.keyboard_mapping.enabled = true;
        self.mapping_status = "Keyboard output active; switch focus to the target app".to_string();
        self.status = "Keyboard output enabled".to_string();
        self.dirty = true;
    }

    fn toggle_mouse_mapping(&mut self) {
        if self.profile.mouse_mapping.enabled {
            self.profile.mouse_mapping.enabled = false;
            self.release_mouse_mapping();
            self.mouse_mapping_status = format!(
                "Mouse output disabled ({})",
                self.mouse_mapper.permission_status()
            );
            self.status = "Mouse output disabled".to_string();
            self.dirty = true;
            return;
        }

        if !self.mouse_mapper.can_post_events() {
            let access_granted = self.mouse_mapper.request_event_posting_access();
            self.mouse_mapping_status = format!(
                "Mouse output unavailable ({})",
                self.mouse_mapper.permission_status()
            );
            if access_granted {
                self.status = "Accessibility access granted; press k again to enable mouse output"
                    .to_string();
            } else {
                self.open_accessibility_settings();
            }
            return;
        }

        self.profile.mouse_mapping.enabled = true;
        self.mouse_mapping_status =
            "Mouse output active; switch focus to the target app".to_string();
        self.status = "Mouse output enabled".to_string();
        self.dirty = true;
    }

    fn adjust_mouse_mapping_field(&mut self, delta: i16) {
        if delta == 0 {
            return;
        }

        match self.selected_mouse_field {
            MouseField::Enabled => self.toggle_mouse_mapping(),
            MouseField::PointerSpeed => {
                self.profile.mouse_mapping.pointer_speed =
                    adjust_u8(self.profile.mouse_mapping.pointer_speed, delta).clamp(
                        MouseMappingProfile::MIN_POINTER_SPEED,
                        MouseMappingProfile::MAX_POINTER_SPEED,
                    );
                self.status = format!(
                    "Mouse pointer speed: {}",
                    self.profile.mouse_mapping.pointer_speed
                );
                self.dirty = true;
            }
            MouseField::Deadzone => {
                self.profile.mouse_mapping.deadzone_percent =
                    adjust_u8(self.profile.mouse_mapping.deadzone_percent, delta)
                        .min(MouseMappingProfile::MAX_DEADZONE_PERCENT);
                self.status = format!(
                    "Mouse dead zone: {}%",
                    self.profile.mouse_mapping.deadzone_percent
                );
                self.dirty = true;
            }
            MouseField::ScrollSpeed => {
                self.profile.mouse_mapping.scroll_speed =
                    adjust_u8(self.profile.mouse_mapping.scroll_speed, delta).clamp(
                        MouseMappingProfile::MIN_SCROLL_SPEED,
                        MouseMappingProfile::MAX_SCROLL_SPEED,
                    );
                self.status = format!(
                    "Mouse scroll speed: {}",
                    self.profile.mouse_mapping.scroll_speed
                );
                self.dirty = true;
            }
        }
    }

    fn toggle_mapping_output(&mut self) {
        match self.mapping_view {
            MappingView::ControllerProfile => {
                self.status = "Controller mapping profile changes are saved with s".to_string();
            }
            MappingView::KeyboardOutput => self.toggle_keyboard_mapping(),
            MappingView::MouseOutput => self.toggle_mouse_mapping(),
        }
    }

    fn open_accessibility_settings(&mut self) {
        match open_accessibility_settings() {
            Ok(()) => {
                self.status =
                    "Accessibility settings opened; enable the signed DualSenseTUI.app, then press k again"
                        .to_string();
            }
            Err(error) => {
                self.status = format!("Could not open Accessibility settings: {error:#}");
            }
        }
    }

    fn apply_active_panel(&mut self) {
        match self.active_tab {
            Tab::Lightbar => self.apply_lightbar(),
            Tab::Haptics => {
                if self.selected_haptic_field == HapticField::ReactiveState {
                    self.toggle_audio_reactive_haptics();
                } else {
                    self.play_haptic_demo();
                }
            }
            Tab::Triggers => self.apply_adaptive_triggers(),
            Tab::System => self.apply_system_controls(),
            Tab::Mapping => self.toggle_mapping_output(),
            Tab::Devices | Tab::Input | Tab::Sensors => {
                self.status = "No apply action for this panel".to_string();
            }
        }
    }

    fn resize_devices_panel(&mut self, delta: i16) {
        self.layout.adjust_devices(delta);
        self.status = format!("Devices panel size: {}", self.layout.devices_size);
    }

    fn resize_status_panel(&mut self, delta: i16) {
        self.layout.adjust_status(delta);
        self.status = format!("Status panel size: {}", self.layout.status_size);
    }

    fn resize_controls_panel(&mut self, delta: i16) {
        self.layout.adjust_controls(delta);
        self.status = format!("Controls panel height: {}", self.layout.controls_height);
    }

    fn reset_layout(&mut self) {
        self.layout = PanelLayout::default();
        self.status = "Panel layout reset".to_string();
    }
}

impl Drop for ConfiguratorApp {
    fn drop(&mut self) {
        let _ = self.stop_audio_reactive_haptics();
    }
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut ConfiguratorApp,
) -> Result<()> {
    loop {
        app.update_input_state();
        app.tick_audio_reactive_haptics();
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(20))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    Ok(())
}

fn handle_key(app: &mut ConfiguratorApp, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Left => {
                app.resize_devices_panel(-2);
                return;
            }
            KeyCode::Right => {
                app.resize_devices_panel(2);
                return;
            }
            KeyCode::Up => {
                app.resize_controls_panel(-1);
                return;
            }
            KeyCode::Down => {
                app.resize_controls_panel(1);
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.previous_tab(),
        KeyCode::Char('1') => app.select_tab(Tab::Devices),
        KeyCode::Char('2') => app.select_tab(Tab::Input),
        KeyCode::Char('3') => app.select_tab(Tab::Sensors),
        KeyCode::Char('4') => app.select_tab(Tab::Lightbar),
        KeyCode::Char('5') => app.select_tab(Tab::Haptics),
        KeyCode::Char('6') => app.select_tab(Tab::Triggers),
        KeyCode::Char('7') => app.select_tab(Tab::System),
        KeyCode::Char('8') => app.select_tab(Tab::Mapping),
        KeyCode::Char('r') => app.refresh_devices(),
        KeyCode::Char('s') => app.save_profile(),
        KeyCode::Char('[') => app.resize_devices_panel(-2),
        KeyCode::Char(']') => app.resize_devices_panel(2),
        KeyCode::Char('{') => app.resize_status_panel(-2),
        KeyCode::Char('}') => app.resize_status_panel(2),
        KeyCode::Char('<') => app.resize_controls_panel(-1),
        KeyCode::Char('>') => app.resize_controls_panel(1),
        KeyCode::Char('0') => app.reset_layout(),
        KeyCode::Char('a') => app.apply_active_panel(),
        KeyCode::Enter => app.apply_active_panel(),
        KeyCode::Char('p') => app.pulse_haptics(),
        KeyCode::Char('d') => app.play_haptic_demo(),
        KeyCode::Char('m') if app.active_tab == Tab::Mapping => app.toggle_mapping_view(),
        KeyCode::Char('k') if app.active_tab == Tab::Mapping => app.toggle_mapping_output(),
        KeyCode::Char('o') if app.active_tab == Tab::Mapping => app.open_accessibility_settings(),
        KeyCode::Char('x') => {
            if app.active_tab == Tab::Triggers {
                app.reset_adaptive_triggers();
            }
        }
        KeyCode::Char(' ') => app.toggle_active(),
        KeyCode::Up => app.move_selection(-1),
        KeyCode::Down => app.move_selection(1),
        KeyCode::Left => app.adjust_active(-5),
        KeyCode::Right => app.adjust_active(5),
        KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_active(1),
        KeyCode::Char('-') | KeyCode::Char('_') => app.adjust_active(-1),
        _ => {}
    }
}

fn move_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let len = len as isize;
    (current as isize + delta).rem_euclid(len) as usize
}

fn inactive_lightbar_reapply_delay(burst_index: usize) -> (Duration, usize) {
    match LIGHTBAR_INACTIVE_REAPPLY_BURST.get(burst_index).copied() {
        Some(delay) => (delay, burst_index + 1),
        None => (LIGHTBAR_INACTIVE_REAPPLY_INTERVAL, burst_index),
    }
}

fn adjust_u8(value: u8, delta: i16) -> u8 {
    (i16::from(value) + delta).clamp(0, 255) as u8
}

fn adjust_u16(value: u16, delta: i16, min: u16, max: u16) -> u16 {
    (i32::from(value) + i32::from(delta)).clamp(i32::from(min), i32::from(max)) as u16
}

fn profile_key(mac_address: Option<&str>) -> String {
    mac_address
        .filter(|address| !address.trim().is_empty())
        .map(|address| address.to_ascii_lowercase())
        .unwrap_or_else(|| "default".to_string())
}

fn profile_path_for_device(mac_address: Option<&str>) -> String {
    mac_address
        .filter(|address| !address.trim().is_empty())
        .map(config::profile_path_for_device)
        .unwrap_or_else(config::profile_path)
        .display()
        .to_string()
}

fn device_label(mac_address: &Option<String>) -> &str {
    mac_address
        .as_deref()
        .unwrap_or("controller without a MAC address")
}

fn audio_capture_status_message(state: AudioReactiveState) -> String {
    match state {
        AudioReactiveState::Stopped => "Audio-reactive haptics stopped".to_string(),
        AudioReactiveState::Running => "Audio-reactive haptics started from system audio".to_string(),
        AudioReactiveState::Unsupported => {
            "Audio-reactive haptics require macOS 14.2 or later".to_string()
        }
        AudioReactiveState::Failed(error) => format!(
            "System-audio capture failed ({error}); allow DualSenseTUI in Privacy & Security > Screen & System Audio Recording"
        ),
    }
}

fn trigger_effects_for(
    target: TriggerTarget,
    effect: AdaptiveTriggerEffect,
) -> (AdaptiveTriggerEffect, AdaptiveTriggerEffect) {
    let off = AdaptiveTriggerEffect::off();
    match target {
        TriggerTarget::Both => (effect, effect),
        TriggerTarget::Left => (effect, off),
        TriggerTarget::Right => (off, effect),
    }
}

fn adjust_trigger_field(profile: &mut Profile, field: TriggerField, delta: i16) {
    match field {
        TriggerField::Target => {
            if delta > 0 {
                profile.adaptive_triggers.target = profile.adaptive_triggers.target.next();
            } else if delta < 0 {
                profile.adaptive_triggers.target = profile.adaptive_triggers.target.previous();
            }
        }
        TriggerField::Mode => {
            if delta > 0 {
                profile.adaptive_triggers.mode = profile.adaptive_triggers.mode.next();
            } else if delta < 0 {
                profile.adaptive_triggers.mode = profile.adaptive_triggers.mode.previous();
            }
        }
        TriggerField::Intensity => {
            profile.adaptive_triggers.intensity =
                adjust_u8(profile.adaptive_triggers.intensity, delta);
        }
        TriggerField::StartPosition => {
            profile.adaptive_triggers.start_position =
                adjust_u8(profile.adaptive_triggers.start_position, delta).min(9);
            profile.adaptive_triggers.end_position = profile
                .adaptive_triggers
                .end_position
                .max(profile.adaptive_triggers.start_position)
                .min(9);
        }
        TriggerField::EndPosition => {
            profile.adaptive_triggers.end_position =
                adjust_u8(profile.adaptive_triggers.end_position, delta)
                    .clamp(profile.adaptive_triggers.start_position, 9);
        }
        TriggerField::Frequency => {
            profile.adaptive_triggers.frequency =
                adjust_u8(profile.adaptive_triggers.frequency, delta).max(1);
        }
        TriggerField::Presets => {
            if delta > 0 {
                profile.adaptive_triggers.preset = profile.adaptive_triggers.preset.next();
            } else if delta < 0 {
                profile.adaptive_triggers.preset = profile.adaptive_triggers.preset.previous();
            }
        }
    }
}

fn trigger_effect_description(profile: &crate::model::AdaptiveTriggerProfile) -> String {
    match profile.mode {
        crate::model::AdaptiveTriggerMode::Preset => profile.preset.label().to_string(),
        crate::model::AdaptiveTriggerMode::Resistance => format!(
            "Resistance {}-{}",
            profile.start_position, profile.end_position
        ),
        crate::model::AdaptiveTriggerMode::Vibration => {
            format!("Vibration {} Hz", profile.frequency)
        }
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn start() -> Result<Self> {
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        enable_raw_mode()?;

        if let Err(error) = execute!(terminal.backend_mut(), EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        if let Err(error) = terminal.clear() {
            let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_lightbar_reapply_uses_a_fast_burst_before_steady_state() {
        let (first_delay, first_index) = inactive_lightbar_reapply_delay(0);
        let (second_delay, second_index) = inactive_lightbar_reapply_delay(first_index);
        let (third_delay, third_index) = inactive_lightbar_reapply_delay(second_index);
        let (steady_delay, steady_index) = inactive_lightbar_reapply_delay(third_index);

        assert_eq!(first_delay, Duration::from_millis(50));
        assert_eq!(second_delay, Duration::from_millis(150));
        assert_eq!(third_delay, Duration::from_millis(400));
        assert_eq!(steady_delay, LIGHTBAR_INACTIVE_REAPPLY_INTERVAL);
        assert_eq!(steady_index, third_index);
    }

    #[test]
    fn trigger_field_order_matches_visual_layout() {
        let mut profile = Profile::default();
        let initial_target = profile.adaptive_triggers.target;
        let initial_preset = profile.adaptive_triggers.preset;
        let initial_intensity = profile.adaptive_triggers.intensity;

        adjust_trigger_field(&mut profile, TriggerField::Intensity, 5);
        assert_eq!(profile.adaptive_triggers.target, initial_target);
        assert_eq!(profile.adaptive_triggers.preset, initial_preset);
        assert_eq!(profile.adaptive_triggers.intensity, initial_intensity + 5);

        adjust_trigger_field(&mut profile, TriggerField::Presets, 1);
        assert_eq!(profile.adaptive_triggers.target, initial_target);
        assert_eq!(profile.adaptive_triggers.preset, initial_preset.next());
        assert_eq!(profile.adaptive_triggers.intensity, initial_intensity + 5);
    }

    #[test]
    fn selection_fields_follow_visual_order() {
        assert_eq!(HapticField::State.move_by(1), HapticField::Mode);
        assert_eq!(HapticField::Mode.move_by(1), HapticField::Strength);
        assert_eq!(HapticField::Strength.move_by(1), HapticField::ReactiveState);
        assert_eq!(
            HapticField::ReactiveState.move_by(1),
            HapticField::ReactiveSensitivity
        );
        assert_eq!(
            HapticField::ReactiveSensitivity.move_by(1),
            HapticField::ReactiveThreshold
        );
        assert_eq!(HapticField::ReactiveThreshold.move_by(1), HapticField::Demo);
        assert_eq!(HapticField::Demo.move_by(1), HapticField::State);

        assert_eq!(TriggerField::Target.move_by(1), TriggerField::Mode);
        assert_eq!(TriggerField::Mode.move_by(1), TriggerField::Intensity);
        assert_eq!(
            TriggerField::Intensity.move_by(1),
            TriggerField::StartPosition
        );
        assert_eq!(
            TriggerField::StartPosition.move_by(1),
            TriggerField::EndPosition
        );
        assert_eq!(
            TriggerField::EndPosition.move_by(1),
            TriggerField::Frequency
        );
        assert_eq!(TriggerField::Frequency.move_by(1), TriggerField::Presets);
        assert_eq!(TriggerField::Presets.move_by(1), TriggerField::Target);

        assert_eq!(MouseField::Enabled.move_by(1), MouseField::PointerSpeed);
        assert_eq!(MouseField::PointerSpeed.move_by(1), MouseField::Deadzone);
        assert_eq!(MouseField::Deadzone.move_by(1), MouseField::ScrollSpeed);
        assert_eq!(MouseField::ScrollSpeed.move_by(1), MouseField::Enabled);

        assert_eq!(
            MappingView::ControllerProfile.next(),
            MappingView::KeyboardOutput
        );
        assert_eq!(MappingView::KeyboardOutput.next(), MappingView::MouseOutput);
        assert_eq!(
            MappingView::MouseOutput.next(),
            MappingView::ControllerProfile
        );
    }

    #[test]
    fn zero_delta_does_not_change_cyclic_trigger_fields() {
        let mut profile = Profile::default();
        let target = profile.adaptive_triggers.target;
        let preset = profile.adaptive_triggers.preset;

        adjust_trigger_field(&mut profile, TriggerField::Target, 0);
        adjust_trigger_field(&mut profile, TriggerField::Presets, 0);

        assert_eq!(profile.adaptive_triggers.target, target);
        assert_eq!(profile.adaptive_triggers.preset, preset);
    }

    #[test]
    fn custom_trigger_positions_stay_in_protocol_range_and_order() {
        let mut profile = Profile::default();
        profile.adaptive_triggers.start_position = 8;
        profile.adaptive_triggers.end_position = 8;

        adjust_trigger_field(&mut profile, TriggerField::StartPosition, 20);
        assert_eq!(profile.adaptive_triggers.start_position, 9);
        assert_eq!(profile.adaptive_triggers.end_position, 9);

        adjust_trigger_field(&mut profile, TriggerField::EndPosition, -20);
        assert_eq!(profile.adaptive_triggers.end_position, 9);

        profile.adaptive_triggers.frequency = 1;
        adjust_trigger_field(&mut profile, TriggerField::Frequency, -10);
        assert_eq!(profile.adaptive_triggers.frequency, 1);
    }

    #[test]
    fn panel_layout_adjustments_are_clamped() {
        let mut layout = PanelLayout::default();

        layout.adjust_devices(-1000);
        layout.adjust_status(-1000);
        layout.adjust_controls(-1000);
        assert_eq!(layout.devices_size, 20);
        assert_eq!(layout.status_size, 24);
        assert_eq!(layout.controls_height, 4);

        layout.adjust_devices(1000);
        layout.adjust_status(1000);
        layout.adjust_controls(1000);
        assert_eq!(layout.devices_size, 48);
        assert_eq!(layout.status_size, 56);
        assert_eq!(layout.controls_height, 8);
    }
}
