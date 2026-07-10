use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn adjust_channel(&mut self, index: usize, delta: i16) {
        let slot = match index {
            0 => &mut self.r,
            1 => &mut self.g,
            2 => &mut self.b,
            _ => return,
        };
        *slot = (i16::from(*slot) + delta).clamp(0, 255) as u8;
    }
}

impl Default for Rgb {
    fn default() -> Self {
        Self::new(0, 96, 255)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum Button {
    Cross,
    Circle,
    Square,
    Triangle,
    L1,
    R1,
    L2,
    R2,
    Create,
    Options,
    L3,
    R3,
    Ps,
    Touchpad,
    Mute,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Fn1,
    Fn2,
    LeftPaddle,
    RightPaddle,
}

impl Button {
    pub const ALL: [Self; 23] = [
        Self::Cross,
        Self::Circle,
        Self::Square,
        Self::Triangle,
        Self::L1,
        Self::R1,
        Self::L2,
        Self::R2,
        Self::Create,
        Self::Options,
        Self::L3,
        Self::R3,
        Self::Ps,
        Self::Touchpad,
        Self::Mute,
        Self::DpadUp,
        Self::DpadDown,
        Self::DpadLeft,
        Self::DpadRight,
        Self::Fn1,
        Self::Fn2,
        Self::LeftPaddle,
        Self::RightPaddle,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cross => "Cross",
            Self::Circle => "Circle",
            Self::Square => "Square",
            Self::Triangle => "Triangle",
            Self::L1 => "L1",
            Self::R1 => "R1",
            Self::L2 => "L2",
            Self::R2 => "R2",
            Self::Create => "Create",
            Self::Options => "Options",
            Self::L3 => "L3",
            Self::R3 => "R3",
            Self::Ps => "PS",
            Self::Touchpad => "Touchpad",
            Self::Mute => "Mute",
            Self::DpadUp => "DPad Up",
            Self::DpadDown => "DPad Down",
            Self::DpadLeft => "DPad Left",
            Self::DpadRight => "DPad Right",
            Self::Fn1 => "Fn 1",
            Self::Fn2 => "Fn 2",
            Self::LeftPaddle => "Left paddle",
            Self::RightPaddle => "Right paddle",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Cross => "X",
            Self::Circle => "CIR",
            Self::Square => "SQR",
            Self::Triangle => "TRI",
            Self::L1 => "L1",
            Self::R1 => "R1",
            Self::L2 => "L2",
            Self::R2 => "R2",
            Self::Create => "CRT",
            Self::Options => "OPT",
            Self::L3 => "L3",
            Self::R3 => "R3",
            Self::Ps => "PS",
            Self::Touchpad => "TP",
            Self::Mute => "MUT",
            Self::DpadUp => "UP",
            Self::DpadDown => "DN",
            Self::DpadLeft => "LT",
            Self::DpadRight => "RT",
            Self::Fn1 => "FN1",
            Self::Fn2 => "FN2",
            Self::LeftPaddle => "LP",
            Self::RightPaddle => "RP",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct StickState {
    pub x: u8,
    pub y: u8,
}

impl StickState {
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    pub fn normalized_x(self) -> f32 {
        normalize_axis(self.x)
    }

    pub fn normalized_y(self) -> f32 {
        normalize_axis(self.y)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TouchPoint {
    pub contact_id: u8,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MotionState {
    pub gyro: [i16; 3],
    pub accel: [i16; 3],
    pub sensor_timestamp: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub enum BatteryStatus {
    #[default]
    Unknown,
    Discharging,
    Charging,
    Full,
    Error,
}

impl BatteryStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Discharging => "discharging",
            Self::Charging => "charging",
            Self::Full => "full",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GamepadState {
    pub left_stick: StickState,
    pub right_stick: StickState,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub buttons: Vec<Button>,
    pub battery_percent: Option<u8>,
    pub battery_status: BatteryStatus,
    pub headset_connected: bool,
    pub microphone_connected: bool,
    pub microphone_muted: bool,
    pub touch_points: [Option<TouchPoint>; 2],
    pub motion: MotionState,
    pub report_sequence: u8,
    pub bluetooth_crc_valid: Option<bool>,
    pub packet_count: u64,
}

impl GamepadState {
    pub fn is_pressed(&self, button: Button) -> bool {
        self.buttons.contains(&button)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HapticDemo {
    Click,
    Thump,
    Buzz,
    Heartbeat,
    Sweep,
    Impact,
    Tap,
    PulseTrain,
}

impl HapticDemo {
    pub const ALL: [Self; 8] = [
        Self::Click,
        Self::Thump,
        Self::Buzz,
        Self::Heartbeat,
        Self::Sweep,
        Self::Impact,
        Self::Tap,
        Self::PulseTrain,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Click => "Click",
            Self::Thump => "Thump",
            Self::Buzz => "Buzz",
            Self::Heartbeat => "Heartbeat",
            Self::Sweep => "Sweep",
            Self::Impact => "Impact",
            Self::Tap => "Tap",
            Self::PulseTrain => "Pulse train",
        }
    }

    pub const fn expected_effect(self) -> &'static str {
        match self {
            Self::Click => "short crisp tap",
            Self::Thump => "one strong paired punch",
            Self::Buzz => "three short paired pulses",
            Self::Heartbeat => "two separated strong beats",
            Self::Sweep => "paired strength rises from light to strong",
            Self::Impact => "one full-strength paired impact",
            Self::Tap => "one strong paired tap",
            Self::PulseTrain => "four paired pulses alternating in strength",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|demo| *demo == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL.iter().position(|demo| *demo == self).unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum TriggerTarget {
    #[default]
    Both,
    Left,
    Right,
}

impl TriggerTarget {
    pub const ALL: [Self; 3] = [Self::Both, Self::Left, Self::Right];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Both => "Both",
            Self::Left => "L2",
            Self::Right => "R2",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|target| *target == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|target| *target == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdaptiveTriggerPreset {
    #[default]
    Off,
    Bow,
    MachineGun,
    Pistol,
    Rigid,
    Brake,
    Pulse,
    Click,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdaptiveTriggerMode {
    #[default]
    Preset,
    Resistance,
    Vibration,
}

impl AdaptiveTriggerMode {
    pub const ALL: [Self; 3] = [Self::Preset, Self::Resistance, Self::Vibration];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Preset => "Preset",
            Self::Resistance => "Resistance",
            Self::Vibration => "Vibration",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl AdaptiveTriggerPreset {
    pub const ALL: [Self; 8] = [
        Self::Off,
        Self::Bow,
        Self::MachineGun,
        Self::Pistol,
        Self::Rigid,
        Self::Brake,
        Self::Pulse,
        Self::Click,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Bow => "Bow",
            Self::MachineGun => "Machine gun",
            Self::Pistol => "Pistol",
            Self::Rigid => "Rigid",
            Self::Brake => "Brake",
            Self::Pulse => "Pulse",
            Self::Click => "Click",
        }
    }

    pub const fn expected_effect(self) -> &'static str {
        match self {
            Self::Off => "trigger motor disabled",
            Self::Bow => "resistance ramps up near the end of travel",
            Self::MachineGun => "rapid persistent trigger vibration",
            Self::Pistol => "short stiff wall with release after the break",
            Self::Rigid => "constant resistance after the start point",
            Self::Brake => "heavy late-travel resistance",
            Self::Pulse => "slow persistent trigger vibration",
            Self::Click => "small tactile bump early in travel",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdaptiveTriggerProfile {
    pub target: TriggerTarget,
    pub mode: AdaptiveTriggerMode,
    pub preset: AdaptiveTriggerPreset,
    pub intensity: u8,
    pub start_position: u8,
    pub end_position: u8,
    pub frequency: u8,
}

impl Default for AdaptiveTriggerProfile {
    fn default() -> Self {
        Self {
            target: TriggerTarget::Both,
            mode: AdaptiveTriggerMode::Preset,
            preset: AdaptiveTriggerPreset::Bow,
            intensity: 180,
            start_position: 2,
            end_position: 9,
            frequency: 32,
        }
    }
}

fn normalize_axis(value: u8) -> f32 {
    ((f32::from(value) - 128.0) / 127.0).clamp(-1.0, 1.0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ButtonMapping {
    pub from: Button,
    pub to: Button,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum KeyboardKey {
    #[default]
    Disabled,
    Space,
    Return,
    Escape,
    Tab,
    Up,
    Down,
    Left,
    Right,
    W,
    A,
    S,
    D,
    Q,
    E,
    R,
    F,
    Shift,
    Control,
    Option,
    Key1,
    Key2,
    Key3,
    Key4,
}

impl KeyboardKey {
    pub const ALL: [Self; 24] = [
        Self::Disabled,
        Self::Space,
        Self::Return,
        Self::Escape,
        Self::Tab,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::W,
        Self::A,
        Self::S,
        Self::D,
        Self::Q,
        Self::E,
        Self::R,
        Self::F,
        Self::Shift,
        Self::Control,
        Self::Option,
        Self::Key1,
        Self::Key2,
        Self::Key3,
        Self::Key4,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Space => "Space",
            Self::Return => "Return",
            Self::Escape => "Escape",
            Self::Tab => "Tab",
            Self::Up => "Up arrow",
            Self::Down => "Down arrow",
            Self::Left => "Left arrow",
            Self::Right => "Right arrow",
            Self::W => "W",
            Self::A => "A",
            Self::S => "S",
            Self::D => "D",
            Self::Q => "Q",
            Self::E => "E",
            Self::R => "R",
            Self::F => "F",
            Self::Shift => "Shift",
            Self::Control => "Control",
            Self::Option => "Option",
            Self::Key1 => "1",
            Self::Key2 => "2",
            Self::Key3 => "3",
            Self::Key4 => "4",
        }
    }

    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|key| *key == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL.iter().position(|key| *key == self).unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyboardBinding {
    pub from: Button,
    pub to: KeyboardKey,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardMappingProfile {
    pub enabled: bool,
    pub bindings: Vec<KeyboardBinding>,
}

impl KeyboardMappingProfile {
    pub fn normalize_bindings(&mut self) {
        let mut normalized = Vec::with_capacity(Button::ALL.len());
        for from in Button::ALL {
            let to = self
                .bindings
                .iter()
                .find(|binding| binding.from == from)
                .map(|binding| binding.to)
                .unwrap_or(KeyboardKey::Disabled);
            normalized.push(KeyboardBinding { from, to });
        }
        self.bindings = normalized;
    }
}

impl Default for KeyboardMappingProfile {
    fn default() -> Self {
        Self {
            enabled: false,
            bindings: Button::ALL
                .into_iter()
                .map(|from| KeyboardBinding {
                    from,
                    to: KeyboardKey::Disabled,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseMappingProfile {
    pub enabled: bool,
    pub pointer_speed: u8,
    pub deadzone_percent: u8,
    pub scroll_speed: u8,
}

impl MouseMappingProfile {
    pub const MIN_POINTER_SPEED: u8 = 1;
    pub const MAX_POINTER_SPEED: u8 = 40;
    pub const MAX_DEADZONE_PERCENT: u8 = 40;
    pub const MIN_SCROLL_SPEED: u8 = 1;
    pub const MAX_SCROLL_SPEED: u8 = 20;

    pub fn normalize(&mut self) {
        self.pointer_speed = self
            .pointer_speed
            .clamp(Self::MIN_POINTER_SPEED, Self::MAX_POINTER_SPEED);
        self.deadzone_percent = self.deadzone_percent.min(Self::MAX_DEADZONE_PERCENT);
        self.scroll_speed = self
            .scroll_speed
            .clamp(Self::MIN_SCROLL_SPEED, Self::MAX_SCROLL_SPEED);
    }
}

impl Default for MouseMappingProfile {
    fn default() -> Self {
        Self {
            enabled: false,
            pointer_speed: 14,
            deadzone_percent: 12,
            scroll_speed: 8,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HapticsProfile {
    pub enabled: bool,
    pub audio_haptics: bool,
    pub left_strength: u8,
    pub right_strength: u8,
    pub audio_reactive: AudioReactiveHapticsProfile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioReactiveHapticsProfile {
    /// Audio gain before the silence threshold is applied.
    pub sensitivity_percent: u8,
    /// Levels below this percentage are treated as silence.
    pub threshold_percent: u8,
}

impl AudioReactiveHapticsProfile {
    pub const MIN_SENSITIVITY_PERCENT: u8 = 25;
    pub const MAX_SENSITIVITY_PERCENT: u8 = 250;
    pub const MAX_THRESHOLD_PERCENT: u8 = 90;

    pub fn normalize(&mut self) {
        self.sensitivity_percent = self
            .sensitivity_percent
            .clamp(Self::MIN_SENSITIVITY_PERCENT, Self::MAX_SENSITIVITY_PERCENT);
        self.threshold_percent = self.threshold_percent.min(Self::MAX_THRESHOLD_PERCENT);
    }
}

impl Default for AudioReactiveHapticsProfile {
    fn default() -> Self {
        Self {
            sensitivity_percent: 150,
            threshold_percent: 4,
        }
    }
}

impl Default for HapticsProfile {
    fn default() -> Self {
        Self {
            enabled: true,
            audio_haptics: true,
            left_strength: 96,
            right_strength: 96,
            audio_reactive: AudioReactiveHapticsProfile::default(),
        }
    }
}

impl HapticsProfile {
    /// Returns the one level sent to both haptic motors.
    ///
    /// Older profiles stored one value per motor. Averaging those values once
    /// during normalization preserves their overall intensity while removing
    /// the directional bias.
    pub fn strength(&self) -> u8 {
        ((self.left_strength as u16 + self.right_strength as u16).div_ceil(2)) as u8
    }

    pub fn set_strength(&mut self, strength: u8) {
        self.left_strength = strength;
        self.right_strength = strength;
    }

    pub fn normalize(&mut self) {
        self.set_strength(self.strength());
        self.audio_reactive.normalize();
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AudioRoute {
    #[default]
    Unchanged,
    Headphones,
    Speaker,
}

impl AudioRoute {
    pub const ALL: [Self; 3] = [Self::Unchanged, Self::Headphones, Self::Speaker];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Unchanged => "Keep current",
            Self::Headphones => "Headphones",
            Self::Speaker => "Controller speaker",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|route| *route == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|route| *route == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum PlayerIndicator {
    Off,
    #[default]
    Player1,
    Player2,
    Player3,
    Player4,
    Player5,
}

impl PlayerIndicator {
    pub const ALL: [Self; 6] = [
        Self::Off,
        Self::Player1,
        Self::Player2,
        Self::Player3,
        Self::Player4,
        Self::Player5,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Player1 => "Player 1",
            Self::Player2 => "Player 2",
            Self::Player3 => "Player 3",
            Self::Player4 => "Player 4",
            Self::Player5 => "Player 5",
        }
    }

    pub const fn led_mask(self) -> u8 {
        match self {
            Self::Off => 0b00000,
            Self::Player1 => 0b00100,
            Self::Player2 => 0b01010,
            Self::Player3 => 0b10101,
            Self::Player4 => 0b11011,
            Self::Player5 => 0b11111,
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|indicator| *indicator == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|indicator| *indicator == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemProfile {
    pub player_indicator: PlayerIndicator,
    pub microphone_muted: bool,
    pub speaker_volume: u8,
    pub microphone_volume: u8,
    pub audio_route: AudioRoute,
}

impl Default for SystemProfile {
    fn default() -> Self {
        Self {
            player_indicator: PlayerIndicator::Player1,
            microphone_muted: false,
            speaker_volume: 100,
            microphone_volume: 32,
            audio_route: AudioRoute::Unchanged,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub lightbar: Rgb,
    pub haptics: HapticsProfile,
    pub adaptive_triggers: AdaptiveTriggerProfile,
    pub system: SystemProfile,
    pub mappings: Vec<ButtonMapping>,
    pub keyboard_mapping: KeyboardMappingProfile,
    pub mouse_mapping: MouseMappingProfile,
}

impl Profile {
    pub fn normalize_mappings(&mut self) {
        let mut normalized = Vec::with_capacity(Button::ALL.len());
        for from in Button::ALL {
            let to = self
                .mappings
                .iter()
                .find(|mapping| mapping.from == from)
                .map(|mapping| mapping.to)
                .unwrap_or(from);
            normalized.push(ButtonMapping { from, to });
        }
        self.mappings = normalized;
        self.keyboard_mapping.normalize_bindings();
        self.mouse_mapping.normalize();
        self.haptics.normalize();
    }
}

impl Default for Profile {
    fn default() -> Self {
        let mappings = Button::ALL
            .into_iter()
            .map(|button| ButtonMapping {
                from: button,
                to: button,
            })
            .collect();

        Self {
            lightbar: Rgb::default(),
            haptics: HapticsProfile::default(),
            adaptive_triggers: AdaptiveTriggerProfile::default(),
            system: SystemProfile::default(),
            mappings,
            keyboard_mapping: KeyboardMappingProfile::default(),
            mouse_mapping: MouseMappingProfile::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_adjustment_clamps_values_and_ignores_unknown_channels() {
        let mut color = Rgb::new(0, 255, 10);

        color.adjust_channel(0, -1);
        color.adjust_channel(1, 1);
        color.adjust_channel(2, 10);
        color.adjust_channel(9, 100);

        assert_eq!(color, Rgb::new(0, 255, 20));
    }

    #[test]
    fn normalize_mappings_keeps_first_mapping_and_fills_identity_rows() {
        let mut profile = Profile {
            mappings: vec![
                ButtonMapping {
                    from: Button::Cross,
                    to: Button::Circle,
                },
                ButtonMapping {
                    from: Button::Cross,
                    to: Button::Square,
                },
                ButtonMapping {
                    from: Button::Circle,
                    to: Button::Cross,
                },
            ],
            ..Profile::default()
        };

        profile.normalize_mappings();

        assert_eq!(profile.mappings.len(), Button::ALL.len());
        assert_eq!(profile.mappings[0].from, Button::Cross);
        assert_eq!(profile.mappings[0].to, Button::Circle);
        assert_eq!(profile.mappings[1].from, Button::Circle);
        assert_eq!(profile.mappings[1].to, Button::Cross);
        let triangle = profile
            .mappings
            .iter()
            .find(|mapping| mapping.from == Button::Triangle)
            .unwrap();
        assert_eq!(triangle.to, Button::Triangle);
    }

    #[test]
    fn old_profiles_receive_new_system_trigger_mapping_and_haptics_defaults() {
        let mut profile: Profile = serde_json::from_str(r#"{"mappings":[]}"#).unwrap();

        profile.normalize_mappings();

        assert_eq!(profile.adaptive_triggers.mode, AdaptiveTriggerMode::Preset);
        assert_eq!(profile.system, SystemProfile::default());
        assert!(!profile.keyboard_mapping.enabled);
        assert_eq!(profile.keyboard_mapping.bindings.len(), Button::ALL.len());
        assert!(profile
            .keyboard_mapping
            .bindings
            .iter()
            .all(|binding| binding.to == KeyboardKey::Disabled));
        assert_eq!(profile.mouse_mapping, MouseMappingProfile::default());
        assert_eq!(
            profile.haptics.audio_reactive,
            AudioReactiveHapticsProfile::default()
        );
    }

    #[test]
    fn old_haptics_profiles_become_symmetric_and_receive_audio_reactive_defaults() {
        let mut profile: Profile = serde_json::from_str(
            r#"{"haptics":{"enabled":false,"audio_haptics":false,"left_strength":10,"right_strength":20}}"#,
        )
        .unwrap();

        profile.normalize_mappings();

        assert_eq!(profile.haptics.left_strength, 15);
        assert_eq!(profile.haptics.right_strength, 15);
        assert_eq!(profile.haptics.strength(), 15);
        assert_eq!(
            profile.haptics.audio_reactive,
            AudioReactiveHapticsProfile::default()
        );
    }

    #[test]
    fn mouse_mapping_values_are_normalized_to_supported_ranges() {
        let mut mapping = MouseMappingProfile {
            enabled: true,
            pointer_speed: 0,
            deadzone_percent: u8::MAX,
            scroll_speed: u8::MAX,
        };

        mapping.normalize();

        assert_eq!(
            mapping.pointer_speed,
            MouseMappingProfile::MIN_POINTER_SPEED
        );
        assert_eq!(
            mapping.deadzone_percent,
            MouseMappingProfile::MAX_DEADZONE_PERCENT
        );
        assert_eq!(mapping.scroll_speed, MouseMappingProfile::MAX_SCROLL_SPEED);
    }
}
