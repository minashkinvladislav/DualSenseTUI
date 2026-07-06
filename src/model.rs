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
            _ => &mut self.b,
        };
        *slot = (i16::from(*slot) + delta).clamp(0, 255) as u8;
    }
}

impl Default for Rgb {
    fn default() -> Self {
        Self::new(0, 96, 255)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
}

impl Button {
    pub const ALL: [Self; 19] = [
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GamepadState {
    pub left_stick: StickState,
    pub right_stick: StickState,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub buttons: Vec<Button>,
    pub battery_percent: Option<u8>,
    pub packet_count: u64,
}

impl GamepadState {
    pub fn is_pressed(&self, button: Button) -> bool {
        self.buttons.contains(&button)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HapticDemo {
    Click,
    Thump,
    Buzz,
    Heartbeat,
    Sweep,
    LeftTap,
    RightTap,
    Alternating,
}

impl HapticDemo {
    pub const ALL: [Self; 8] = [
        Self::Click,
        Self::Thump,
        Self::Buzz,
        Self::Heartbeat,
        Self::Sweep,
        Self::LeftTap,
        Self::RightTap,
        Self::Alternating,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Click => "Click",
            Self::Thump => "Thump",
            Self::Buzz => "Buzz",
            Self::Heartbeat => "Heartbeat",
            Self::Sweep => "Sweep",
            Self::LeftTap => "Heavy tap",
            Self::RightTap => "Sharp tap",
            Self::Alternating => "Alternating",
        }
    }

    pub const fn expected_effect(self) -> &'static str {
        match self {
            Self::Click => "short crisp tap",
            Self::Thump => "one heavy low punch",
            Self::Buzz => "fast high-frequency buzz",
            Self::Heartbeat => "two separated strong beats",
            Self::Sweep => "heavy low buzz fades into sharper high buzz",
            Self::LeftTap => "both motors, heavy-weighted",
            Self::RightTap => "both motors, sharp-weighted",
            Self::Alternating => "heavy and sharp pulses trade places",
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AdaptiveTriggerProfile {
    pub target: TriggerTarget,
    pub preset: AdaptiveTriggerPreset,
    pub intensity: u8,
}

impl Default for AdaptiveTriggerProfile {
    fn default() -> Self {
        Self {
            target: TriggerTarget::Both,
            preset: AdaptiveTriggerPreset::Bow,
            intensity: 180,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct HapticsProfile {
    pub enabled: bool,
    pub audio_haptics: bool,
    pub left_strength: u8,
    pub right_strength: u8,
}

impl Default for HapticsProfile {
    fn default() -> Self {
        Self {
            enabled: true,
            audio_haptics: true,
            left_strength: 96,
            right_strength: 96,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub lightbar: Rgb,
    pub haptics: HapticsProfile,
    pub adaptive_triggers: AdaptiveTriggerProfile,
    pub mappings: Vec<ButtonMapping>,
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
            mappings,
        }
    }
}
