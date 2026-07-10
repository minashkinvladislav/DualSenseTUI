use serde::Serialize;

use crate::model::{GamepadState, Rgb, SystemProfile};

mod effects;

pub use effects::{demo_frames, trigger_effect_for_profile};

pub const SONY_VENDOR_ID: u32 = 0x054c;
pub const DUALSENSE_PRODUCT_ID: u32 = 0x0ce6;
pub const DUALSENSE_EDGE_PRODUCT_ID: u32 = 0x0df2;
pub const DUALSENSE_PRODUCT_IDS: [u32; 2] = [DUALSENSE_PRODUCT_ID, DUALSENSE_EDGE_PRODUCT_ID];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct FirmwareInfo {
    pub hardware_version: u32,
    pub firmware_version: u32,
    pub feature_version: u16,
}

impl FirmwareInfo {
    pub fn hardware_label(self) -> String {
        format!("0x{:08x}", self.hardware_version)
    }

    pub fn firmware_label(self) -> String {
        format!("0x{:08x}", self.firmware_version)
    }

    pub fn feature_label(self) -> String {
        format!(
            "{}.{}",
            self.feature_version >> 8,
            self.feature_version & 0xff
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub vendor_id: u32,
    pub product_id: u32,
    pub transport: String,
    pub mac_address: Option<String>,
    pub firmware: Option<FirmwareInfo>,
    pub diagnostics_error: Option<String>,
}

impl DeviceInfo {
    pub fn is_bluetooth(&self) -> bool {
        self.transport.to_ascii_lowercase().contains("bluetooth")
    }

    pub const fn is_edge(&self) -> bool {
        self.product_id == DUALSENSE_EDGE_PRODUCT_ID
    }

    pub fn supports_usb_audio(&self) -> bool {
        !self.is_bluetooth()
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

/// One immediate haptics command, without a duration or blocking delay.
///
/// The fields retain the DualSense channel names used by the HID protocol.
/// User-facing output is normalized so both channels receive the same value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HapticOutput {
    pub heavy: u8,
    pub sharp: u8,
}

impl HapticOutput {
    pub const OFF: Self = Self { heavy: 0, sharp: 0 };

    pub const fn new(heavy: u8, sharp: u8) -> Self {
        Self { heavy, sharp }
    }

    pub const fn symmetric(strength: u8) -> Self {
        Self::new(strength, strength)
    }

    pub const fn is_symmetric(self) -> bool {
        self.heavy == self.sharp
    }

    /// Converts a legacy per-channel command to one shared motor level.
    pub fn symmetrized(self) -> Self {
        Self::symmetric(((self.heavy as u16 + self.sharp as u16).div_ceil(2)) as u8)
    }
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

    pub const fn symmetric(strength: u8, duration_ms: u64) -> Self {
        Self::new(strength, strength, duration_ms)
    }

    pub const fn is_symmetric(self) -> bool {
        self.left == self.right
    }

    pub fn symmetrized(self) -> Self {
        Self::symmetric(
            ((self.left as u16 + self.right as u16).div_ceil(2)) as u8,
            self.duration_ms,
        )
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
    /// Reapplies an already configured colour without restarting device setup.
    fn reapply_lightbar(&mut self, index: usize, color: Rgb) -> anyhow::Result<()> {
        self.set_lightbar(index, color)
    }
    fn pulse_haptics(
        &mut self,
        index: usize,
        left: u8,
        right: u8,
        audio_haptics: bool,
    ) -> anyhow::Result<()>;
    fn set_haptics(
        &mut self,
        index: usize,
        output: HapticOutput,
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
    fn set_system_controls(&mut self, index: usize, system: &SystemProfile) -> anyhow::Result<()>;
    fn pump_events(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn take_device_change(&mut self) -> bool {
        false
    }
    fn read_state(&mut self, index: usize) -> anyhow::Result<Option<GamepadState>>;
}

#[cfg(test)]
mod tests {
    use super::{HapticFrame, HapticOutput};

    #[test]
    fn legacy_haptic_values_are_converted_to_one_shared_level() {
        let output = HapticOutput::new(10, 21).symmetrized();
        let frame = HapticFrame::new(10, 21, 180).symmetrized();

        assert_eq!(output, HapticOutput::symmetric(16));
        assert_eq!(frame, HapticFrame::symmetric(16, 180));
        assert!(output.is_symmetric());
        assert!(frame.is_symmetric());
    }
}
