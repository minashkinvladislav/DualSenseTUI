use anyhow::{bail, Result};

use crate::{
    dualsense::{AdaptiveTriggerEffect, DeviceInfo, DualSenseControl, HapticFrame},
    model::{GamepadState, Rgb},
};

pub struct DualSenseBackend;

impl DualSenseBackend {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl DualSenseControl for DualSenseBackend {
    fn refresh(&mut self) -> Result<()> {
        Ok(())
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        Vec::new()
    }

    fn set_lightbar(&mut self, _index: usize, _color: Rgb) -> Result<()> {
        bail!("IOKit backend is available only on macOS")
    }

    fn pulse_haptics(
        &mut self,
        _index: usize,
        _left: u8,
        _right: u8,
        _audio_haptics: bool,
    ) -> Result<()> {
        bail!("IOKit backend is available only on macOS")
    }

    fn play_haptics(
        &mut self,
        _index: usize,
        _frames: &[HapticFrame],
        _audio_haptics: bool,
    ) -> Result<()> {
        bail!("IOKit backend is available only on macOS")
    }

    fn set_adaptive_triggers(
        &mut self,
        _index: usize,
        _left: AdaptiveTriggerEffect,
        _right: AdaptiveTriggerEffect,
    ) -> Result<()> {
        bail!("IOKit backend is available only on macOS")
    }

    fn read_state(&mut self, _index: usize) -> Result<Option<GamepadState>> {
        Ok(None)
    }
}
