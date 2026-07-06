use crate::model::{GamepadState, Rgb};

pub const SONY_VENDOR_ID: u32 = 0x054c;
pub const DUALSENSE_PRODUCT_IDS: [u32; 2] = [0x0ce6, 0x0df2];

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub vendor_id: u32,
    pub product_id: u32,
    pub transport: String,
}

impl DeviceInfo {
    pub fn is_bluetooth(&self) -> bool {
        self.transport.to_ascii_lowercase().contains("bluetooth")
    }
}

pub fn is_supported_dualsense(vendor_id: u32, product_id: u32) -> bool {
    vendor_id == SONY_VENDOR_ID && DUALSENSE_PRODUCT_IDS.contains(&product_id)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HapticFrame {
    pub left: u8,
    pub right: u8,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdaptiveTriggerEffect {
    pub bytes: [u8; 11],
}

impl AdaptiveTriggerEffect {
    pub const fn off() -> Self {
        Self { bytes: [0; 11] }
    }

    pub const fn from_bytes(bytes: [u8; 11]) -> Self {
        Self { bytes }
    }
}

impl HapticFrame {
    pub const fn new(left: u8, right: u8, duration_ms: u64) -> Self {
        Self {
            left,
            right,
            duration_ms,
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_iokit;

#[cfg(target_os = "macos")]
pub use macos_iokit::DualSenseBackend;

#[cfg(not(target_os = "macos"))]
mod stub;

#[cfg(not(target_os = "macos"))]
pub use stub::DualSenseBackend;

pub trait DualSenseControl {
    fn refresh(&mut self) -> anyhow::Result<()>;
    fn devices(&self) -> Vec<DeviceInfo>;
    fn set_lightbar(&mut self, index: usize, color: Rgb) -> anyhow::Result<()>;
    fn pulse_haptics(
        &mut self,
        index: usize,
        left: u8,
        right: u8,
        audio_haptics: bool,
    ) -> anyhow::Result<()>;
    fn play_haptics(
        &mut self,
        index: usize,
        frames: &[HapticFrame],
        audio_haptics: bool,
    ) -> anyhow::Result<()>;
    fn set_adaptive_triggers(
        &mut self,
        index: usize,
        left: AdaptiveTriggerEffect,
        right: AdaptiveTriggerEffect,
    ) -> anyhow::Result<()>;
    fn read_state(&mut self, index: usize) -> anyhow::Result<Option<GamepadState>>;
}
