use std::{
    io::{self, Stdout},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    config,
    dualsense::{
        AdaptiveTriggerEffect, DeviceInfo, DualSenseBackend, DualSenseControl, HapticFrame,
    },
    model::{AdaptiveTriggerPreset, GamepadState, HapticDemo, Profile, TriggerTarget},
    ui,
};

const HEAVY_HAPTIC_GAIN_PERCENT: u16 = 50;

pub fn run() -> Result<()> {
    let mut app = ConfiguratorApp::new()?;
    let mut terminal = TerminalSession::start()?;
    run_event_loop(&mut terminal.terminal, &mut app)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    Devices,
    Input,
    Lightbar,
    Haptics,
    Triggers,
    Mapping,
}

impl Tab {
    pub const ALL: [Self; 6] = [
        Self::Devices,
        Self::Input,
        Self::Lightbar,
        Self::Haptics,
        Self::Triggers,
        Self::Mapping,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Devices => "Devices",
            Self::Input => "Input",
            Self::Lightbar => "Lightbar",
            Self::Haptics => "Haptics",
            Self::Triggers => "Triggers",
            Self::Mapping => "Mapping",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Devices => 0,
            Self::Input => 1,
            Self::Lightbar => 2,
            Self::Haptics => 3,
            Self::Triggers => 4,
            Self::Mapping => 5,
        }
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index % Self::ALL.len()]
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
    pub profile: Profile,
    pub layout: PanelLayout,
    pub devices: Vec<DeviceInfo>,
    pub selected_device: usize,
    pub active_tab: Tab,
    pub selected_color_channel: usize,
    pub selected_haptic_field: usize,
    pub selected_haptic_demo: HapticDemo,
    pub selected_trigger_field: usize,
    pub selected_mapping: usize,
    pub live_input: Option<GamepadState>,
    pub input_status: String,
    pub status: String,
    pub profile_path: String,
    pub dirty: bool,
    pub should_quit: bool,
}

impl ConfiguratorApp {
    fn new() -> Result<Self> {
        let mut backend = DualSenseBackend::new()?;
        backend.refresh()?;

        let mut profile = config::load_profile()?;
        profile.normalize_mappings();
        let devices = backend.devices();
        let status = if devices.is_empty() {
            "No DualSense detected".to_string()
        } else {
            format!("Detected {} DualSense device(s)", devices.len())
        };

        Ok(Self {
            backend,
            profile,
            layout: PanelLayout::default(),
            devices,
            selected_device: 0,
            active_tab: Tab::Devices,
            selected_color_channel: 0,
            selected_haptic_field: 0,
            selected_haptic_demo: HapticDemo::Click,
            selected_trigger_field: 0,
            selected_mapping: 0,
            live_input: None,
            input_status: "Waiting for input reports".to_string(),
            status,
            profile_path: config::profile_path().display().to_string(),
            dirty: false,
            should_quit: false,
        })
    }

    fn refresh_devices(&mut self) {
        match self.backend.refresh() {
            Ok(()) => {
                self.devices = self.backend.devices();
                if self.selected_device >= self.devices.len() {
                    self.selected_device = self.devices.len().saturating_sub(1);
                }
                self.live_input = None;
                self.input_status = "Waiting for input reports".to_string();
                self.status = if self.devices.is_empty() {
                    "No DualSense detected".to_string()
                } else {
                    format!("Refreshed: {} DualSense device(s)", self.devices.len())
                };
            }
            Err(error) => {
                self.status = format!("Refresh failed: {error:#}");
            }
        }
    }

    fn update_input_state(&mut self) {
        if self.devices.is_empty() {
            self.live_input = None;
            self.input_status = "No DualSense selected".to_string();
            return;
        }

        match self.backend.read_state(self.selected_device) {
            Ok(Some(state)) => {
                self.input_status = format!("Live input packets: {}", state.packet_count);
                self.live_input = Some(state);
            }
            Ok(None) => {
                self.input_status = "Waiting for input reports".to_string();
            }
            Err(error) => {
                self.live_input = None;
                self.input_status = format!("Input unavailable: {error:#}");
            }
        }
    }

    fn save_profile(&mut self) {
        match config::save_profile(&self.profile) {
            Ok(path) => {
                self.dirty = false;
                self.profile_path = path.display().to_string();
                self.status = format!("Saved {}", path.display());
            }
            Err(error) => {
                self.status = format!("Save failed: {error:#}");
            }
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
                self.status = format!(
                    "Lightbar applied: #{:02x}{:02x}{:02x}",
                    color.r, color.g, color.b
                );
            }
            Err(error) => {
                self.status = format!("Lightbar failed: {error:#}");
            }
        }
    }

    fn pulse_haptics(&mut self) {
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }
        if !self.profile.haptics.enabled {
            self.status = "Haptics are disabled".to_string();
            return;
        }

        let frame = balanced_haptic_frame(HapticFrame::new(
            self.profile.haptics.left_strength,
            self.profile.haptics.right_strength,
            180,
        ));
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
        if self.devices.is_empty() {
            self.status = "No DualSense selected".to_string();
            return;
        }
        if !self.profile.haptics.enabled {
            self.status = "Haptics are disabled".to_string();
            return;
        }

        let demo = self.selected_haptic_demo;
        let frames: Vec<_> = demo_frames(demo)
            .into_iter()
            .map(balanced_haptic_frame)
            .collect();
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

        let preset = self.profile.adaptive_triggers.preset;
        let effect = trigger_effect_for(preset, self.profile.adaptive_triggers.intensity);
        let result = self.send_trigger_effect(effect);

        match result {
            Ok(()) => {
                self.status = format!(
                    "Adaptive triggers applied: {} on {}",
                    preset.label(),
                    self.profile.adaptive_triggers.target.label()
                );
            }
            Err(error) => {
                self.status = format!("Adaptive triggers failed: {error:#}");
            }
        }
    }

    fn send_trigger_effect(&mut self, effect: AdaptiveTriggerEffect) -> Result<()> {
        let off = AdaptiveTriggerEffect::off();
        let (left, right) = match self.profile.adaptive_triggers.target {
            TriggerTarget::Both => (effect, effect),
            TriggerTarget::Left => (effect, off),
            TriggerTarget::Right => (off, effect),
        };
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
                self.selected_device = move_index(self.selected_device, self.devices.len(), delta);
                self.live_input = None;
            }
            Tab::Input => {}
            Tab::Lightbar => {
                self.selected_color_channel = move_index(self.selected_color_channel, 3, delta);
            }
            Tab::Haptics => {
                self.selected_haptic_field = move_index(self.selected_haptic_field, 4, delta);
            }
            Tab::Triggers => {
                self.selected_trigger_field = move_index(self.selected_trigger_field, 3, delta);
            }
            Tab::Mapping => {
                self.selected_mapping =
                    move_index(self.selected_mapping, self.profile.mappings.len(), delta);
            }
        }
    }

    fn adjust_active(&mut self, delta: i16) {
        match self.active_tab {
            Tab::Devices => {
                self.move_selection(isize::from(delta.signum()));
            }
            Tab::Input => {}
            Tab::Lightbar => {
                self.profile
                    .lightbar
                    .adjust_channel(self.selected_color_channel, delta);
                self.dirty = true;
            }
            Tab::Haptics => match self.selected_haptic_field {
                0 => {
                    self.profile.haptics.left_strength =
                        adjust_u8(self.profile.haptics.left_strength, delta);
                    self.dirty = true;
                }
                1 => {
                    self.profile.haptics.right_strength =
                        adjust_u8(self.profile.haptics.right_strength, delta);
                    self.dirty = true;
                }
                _ => {
                    if self.selected_haptic_field == 2 {
                        if delta != 0 {
                            self.profile.haptics.audio_haptics =
                                !self.profile.haptics.audio_haptics;
                        }
                        self.dirty = true;
                    } else if delta > 0 {
                        self.selected_haptic_demo = self.selected_haptic_demo.next();
                    } else if delta < 0 {
                        self.selected_haptic_demo = self.selected_haptic_demo.previous();
                    }
                }
            },
            Tab::Triggers => {
                adjust_trigger_field(&mut self.profile, self.selected_trigger_field, delta);
                self.dirty = true;
            }
            Tab::Mapping => {
                if let Some(mapping) = self.profile.mappings.get_mut(self.selected_mapping) {
                    mapping.to = if delta >= 0 {
                        mapping.to.next()
                    } else {
                        mapping.to.previous()
                    };
                    self.dirty = true;
                }
            }
        }
    }

    fn toggle_active(&mut self) {
        match self.active_tab {
            Tab::Haptics => {
                if self.selected_haptic_field == 3 {
                    self.play_haptic_demo();
                } else if self.selected_haptic_field == 2 {
                    self.profile.haptics.audio_haptics = !self.profile.haptics.audio_haptics;
                    self.dirty = true;
                } else {
                    self.profile.haptics.enabled = !self.profile.haptics.enabled;
                    self.dirty = true;
                }
            }
            Tab::Triggers => self.apply_adaptive_triggers(),
            Tab::Mapping => {
                if let Some(mapping) = self.profile.mappings.get_mut(self.selected_mapping) {
                    mapping.to = mapping.from;
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    fn apply_active_panel(&mut self) {
        match self.active_tab {
            Tab::Lightbar => self.apply_lightbar(),
            Tab::Haptics => self.play_haptic_demo(),
            Tab::Triggers => self.apply_adaptive_triggers(),
            Tab::Devices | Tab::Input | Tab::Mapping => {
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

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut ConfiguratorApp,
) -> Result<()> {
    loop {
        app.update_input_state();
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
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
        KeyCode::Char('3') => app.select_tab(Tab::Lightbar),
        KeyCode::Char('4') => app.select_tab(Tab::Haptics),
        KeyCode::Char('5') => app.select_tab(Tab::Triggers),
        KeyCode::Char('6') => app.select_tab(Tab::Mapping),
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

fn adjust_u8(value: u8, delta: i16) -> u8 {
    (i16::from(value) + delta).clamp(0, 255) as u8
}

fn adjust_u16(value: u16, delta: i16, min: u16, max: u16) -> u16 {
    (i32::from(value) + i32::from(delta)).clamp(i32::from(min), i32::from(max)) as u16
}

fn adjust_trigger_field(profile: &mut Profile, field: usize, delta: i16) {
    match field {
        0 => {
            profile.adaptive_triggers.target = if delta >= 0 {
                profile.adaptive_triggers.target.next()
            } else {
                profile.adaptive_triggers.target.previous()
            };
        }
        1 => {
            profile.adaptive_triggers.intensity =
                adjust_u8(profile.adaptive_triggers.intensity, delta);
        }
        _ => {
            profile.adaptive_triggers.preset = if delta >= 0 {
                profile.adaptive_triggers.preset.next()
            } else {
                profile.adaptive_triggers.preset.previous()
            };
        }
    }
}

fn balanced_haptic_frame(frame: HapticFrame) -> HapticFrame {
    HapticFrame::new(
        scale_haptic_channel(frame.left, HEAVY_HAPTIC_GAIN_PERCENT),
        frame.right,
        frame.duration_ms,
    )
}

fn scale_haptic_channel(value: u8, percent: u16) -> u8 {
    ((u16::from(value) * percent + 50) / 100).min(u16::from(u8::MAX)) as u8
}

fn demo_frames(demo: HapticDemo) -> Vec<HapticFrame> {
    match demo {
        HapticDemo::Click => vec![HapticFrame::new(70, 70, 45)],
        HapticDemo::Thump => vec![HapticFrame::new(230, 120, 170)],
        HapticDemo::Buzz => vec![
            HapticFrame::new(0, 170, 35),
            HapticFrame::new(0, 0, 25),
            HapticFrame::new(0, 190, 35),
            HapticFrame::new(0, 0, 25),
            HapticFrame::new(0, 210, 45),
        ],
        HapticDemo::Heartbeat => vec![
            HapticFrame::new(210, 210, 110),
            HapticFrame::new(0, 0, 120),
            HapticFrame::new(255, 255, 150),
        ],
        HapticDemo::Sweep => vec![
            HapticFrame::new(210, 25, 85),
            HapticFrame::new(155, 75, 85),
            HapticFrame::new(95, 135, 85),
            HapticFrame::new(25, 210, 120),
        ],
        HapticDemo::LeftTap => vec![HapticFrame::new(255, 70, 140), HapticFrame::new(0, 0, 70)],
        HapticDemo::RightTap => vec![HapticFrame::new(80, 220, 140), HapticFrame::new(0, 0, 70)],
        HapticDemo::Alternating => vec![
            HapticFrame::new(220, 0, 90),
            HapticFrame::new(0, 0, 45),
            HapticFrame::new(0, 220, 90),
            HapticFrame::new(0, 0, 45),
            HapticFrame::new(220, 0, 90),
            HapticFrame::new(0, 0, 45),
            HapticFrame::new(0, 220, 90),
            HapticFrame::new(0, 0, 70),
        ],
    }
}

fn trigger_effect_for(preset: AdaptiveTriggerPreset, intensity: u8) -> AdaptiveTriggerEffect {
    let strength = trigger_strength(intensity);
    match preset {
        AdaptiveTriggerPreset::Off => AdaptiveTriggerEffect::off(),
        AdaptiveTriggerPreset::Bow => progressive_effect(2, 9, strength),
        AdaptiveTriggerPreset::MachineGun => vibration_effect(0, strength, 38),
        AdaptiveTriggerPreset::Pistol => weapon_effect(2, 5, strength),
        AdaptiveTriggerPreset::Rigid => resistance_effect(1, 9, strength),
        AdaptiveTriggerPreset::Brake => resistance_effect(5, 9, strength),
        AdaptiveTriggerPreset::Pulse => vibration_effect(1, strength, 16),
        AdaptiveTriggerPreset::Click => weapon_effect(1, 3, strength),
    }
}

fn trigger_strength(intensity: u8) -> u8 {
    let scaled = 1 + (u16::from(intensity) * 7 / 255) as u8;
    scaled.clamp(1, 8)
}

fn resistance_effect(start: u8, end: u8, strength: u8) -> AdaptiveTriggerEffect {
    let mut zones = [0; 10];
    for zone in start.min(9)..=end.min(9) {
        zones[usize::from(zone)] = strength.clamp(1, 8);
    }
    feedback_effect(zones)
}

fn progressive_effect(start: u8, end: u8, max_strength: u8) -> AdaptiveTriggerEffect {
    let mut zones = [0; 10];
    let start = start.min(9);
    let end = end.min(9);
    let span = u16::from(end.saturating_sub(start)).max(1);
    for zone in start..=end {
        let step = u16::from(zone - start + 1);
        let strength =
            ((step * u16::from(max_strength.clamp(1, 8))) / (span + 1)).clamp(1, 8) as u8;
        zones[usize::from(zone)] = strength;
    }
    feedback_effect(zones)
}

fn feedback_effect(zones: [u8; 10]) -> AdaptiveTriggerEffect {
    let mut active_mask = 0u16;
    let mut packed_strengths = 0u32;
    for (index, strength) in zones.into_iter().enumerate() {
        if strength == 0 {
            continue;
        }

        let normalized = strength.clamp(1, 8) - 1;
        active_mask |= 1 << index;
        packed_strengths |= u32::from(normalized) << (index * 3);
    }

    AdaptiveTriggerEffect::from_bytes([
        0x21,
        active_mask as u8,
        (active_mask >> 8) as u8,
        packed_strengths as u8,
        (packed_strengths >> 8) as u8,
        (packed_strengths >> 16) as u8,
        (packed_strengths >> 24) as u8,
        0,
        0,
        0,
        0,
    ])
}

fn weapon_effect(start: u8, end: u8, strength: u8) -> AdaptiveTriggerEffect {
    let start = start.clamp(2, 7);
    let end = end.clamp(start + 1, 8);
    let zones = (1u16 << start) | (1u16 << end);
    AdaptiveTriggerEffect::from_bytes([
        0x25,
        zones as u8,
        (zones >> 8) as u8,
        strength.clamp(1, 8) - 1,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ])
}

fn vibration_effect(start: u8, strength: u8, frequency: u8) -> AdaptiveTriggerEffect {
    let mut zones = [0; 10];
    for zone in start.min(9)..=9 {
        zones[usize::from(zone)] = strength.clamp(1, 8);
    }

    let mut effect = feedback_effect(zones);
    effect.bytes[0] = 0x26;
    effect.bytes[9] = frequency;
    effect
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn start() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vibration_named_trigger_presets_use_persistent_vibration_mode() {
        let machine_gun = trigger_effect_for(AdaptiveTriggerPreset::MachineGun, 180);
        let pulse = trigger_effect_for(AdaptiveTriggerPreset::Pulse, 180);
        let bow = trigger_effect_for(AdaptiveTriggerPreset::Bow, 180);

        assert_eq!(machine_gun.bytes[0], 0x26);
        assert_eq!(pulse.bytes[0], 0x26);
        assert_eq!(bow.bytes[0], 0x21);
        assert_ne!(machine_gun.bytes[9], 0);
        assert_ne!(pulse.bytes[9], 0);
        assert_ne!(&machine_gun.bytes[1..7], &[0, 0, 0, 0, 0, 0]);
        assert_ne!(&pulse.bytes[1..7], &[0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn vibration_effect_uses_zone_mask_strength_pack_and_frequency() {
        let effect = vibration_effect(8, 8, 38);

        assert_eq!(effect.bytes[0], 0x26);
        assert_eq!(effect.bytes[1], 0x00);
        assert_eq!(effect.bytes[2], 0x03);
        assert_eq!(effect.bytes[6], 0x3f);
        assert_eq!(effect.bytes[9], 38);
    }

    #[test]
    fn pistol_trigger_uses_weapon_zone_mask_and_zero_based_strength() {
        let effect = trigger_effect_for(AdaptiveTriggerPreset::Pistol, 255);

        assert_eq!(effect.bytes, [0x25, 0x24, 0x00, 0x07, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn alternating_haptic_demo_keeps_channels_separated() {
        let frames = demo_frames(HapticDemo::Alternating);
        let mut active_channels = Vec::new();

        for frame in &frames {
            assert!(
                frame.left == 0 || frame.right == 0,
                "alternating frames must never drive both motors"
            );
            if frame.left > 0 {
                active_channels.push("heavy");
            } else if frame.right > 0 {
                active_channels.push("sharp");
            }
        }

        for pair in frames.windows(3).step_by(2) {
            assert_eq!(pair[1], HapticFrame::new(0, 0, pair[1].duration_ms));
        }
        assert_eq!(active_channels, ["heavy", "sharp", "heavy", "sharp"]);
        assert_eq!(frames.last(), Some(&HapticFrame::new(0, 0, 70)));
    }

    #[test]
    fn haptic_output_balances_heavy_channel() {
        let balanced = balanced_haptic_frame(HapticFrame::new(220, 220, 140));

        assert_eq!(balanced, HapticFrame::new(110, 220, 140));
        assert_eq!(
            balanced_haptic_frame(HapticFrame::new(0, 220, 140)),
            HapticFrame::new(0, 220, 140)
        );
    }

    #[test]
    fn heavy_and_sharp_tap_demos_drive_both_motors() {
        for demo in [HapticDemo::LeftTap, HapticDemo::RightTap] {
            let frames: Vec<_> = demo_frames(demo)
                .into_iter()
                .map(balanced_haptic_frame)
                .collect();
            let first = frames[0];

            assert!(first.left > 0, "{} should drive heavy motor", demo.label());
            assert!(first.right > 0, "{} should drive sharp motor", demo.label());
            assert_eq!(frames.last(), Some(&HapticFrame::new(0, 0, 70)));
        }
    }

    #[test]
    fn trigger_field_order_matches_visual_layout() {
        let mut profile = Profile::default();
        let initial_target = profile.adaptive_triggers.target;
        let initial_preset = profile.adaptive_triggers.preset;
        let initial_intensity = profile.adaptive_triggers.intensity;

        adjust_trigger_field(&mut profile, 1, 5);
        assert_eq!(profile.adaptive_triggers.target, initial_target);
        assert_eq!(profile.adaptive_triggers.preset, initial_preset);
        assert_eq!(profile.adaptive_triggers.intensity, initial_intensity + 5);

        adjust_trigger_field(&mut profile, 2, 1);
        assert_eq!(profile.adaptive_triggers.target, initial_target);
        assert_eq!(profile.adaptive_triggers.preset, initial_preset.next());
        assert_eq!(profile.adaptive_triggers.intensity, initial_intensity + 5);
    }

    #[test]
    fn weapon_effect_clamps_to_valid_weapon_positions() {
        let effect = weapon_effect(1, 3, 8);

        assert_eq!(effect.bytes, [0x25, 0x0c, 0x00, 0x07, 0, 0, 0, 0, 0, 0, 0]);
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
