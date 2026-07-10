//! Audio-to-haptics mapping and lifecycle management.
//!
//! System-audio capture is intentionally a producer only. This module polls
//! its latest levels from the normal application loop, which keeps IOKit HID
//! writes and Bluetooth output-report sequencing on their original thread.

use std::time::{Duration, Instant};

use crate::{
    audio_capture::{AudioBands, SystemAudioCapture, SystemAudioCaptureState},
    dualsense::HapticOutput,
    model::AudioReactiveHapticsProfile,
};

const AUDIO_HEARTBEAT_TIMEOUT: Duration = Duration::from_millis(250);
const ATTACK: f32 = 0.66;
const RELEASE: f32 = 0.18;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioReactiveState {
    Stopped,
    Running,
    Unsupported,
    Failed(i32),
}

impl AudioReactiveState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Unsupported => "requires macOS 14.2+",
            Self::Failed(_) => "permission/error",
        }
    }

    pub const fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}

impl From<SystemAudioCaptureState> for AudioReactiveState {
    fn from(value: SystemAudioCaptureState) -> Self {
        match value {
            SystemAudioCaptureState::Idle => Self::Stopped,
            SystemAudioCaptureState::Running => Self::Running,
            SystemAudioCaptureState::Unsupported => Self::Unsupported,
            SystemAudioCaptureState::Failed(error) => Self::Failed(error),
        }
    }
}

#[derive(Default)]
struct HapticEnvelope {
    strength: f32,
}

impl HapticEnvelope {
    fn reset(&mut self) {
        self.strength = 0.0;
    }

    fn update(
        &mut self,
        bands: AudioBands,
        profile: &AudioReactiveHapticsProfile,
        maximum: u8,
    ) -> HapticOutput {
        let target = normalized_band(bands.low.max(bands.high), profile) * f32::from(maximum);
        self.strength = smooth(self.strength, target);

        HapticOutput::symmetric(to_u8(self.strength))
    }
}

fn normalized_band(value: u16, profile: &AudioReactiveHapticsProfile) -> f32 {
    let amplitude = f32::from(value) / f32::from(u16::MAX);
    let gained = (amplitude * f32::from(profile.sensitivity_percent) / 100.0).min(1.0);
    let threshold = f32::from(profile.threshold_percent) / 100.0;

    if gained <= threshold || threshold >= 1.0 {
        0.0
    } else {
        ((gained - threshold) / (1.0 - threshold)).clamp(0.0, 1.0)
    }
}

fn smooth(current: f32, target: f32) -> f32 {
    let factor = if target > current { ATTACK } else { RELEASE };
    current + (target - current) * factor
}

fn to_u8(value: f32) -> u8 {
    value.round().clamp(0.0, f32::from(u8::MAX)) as u8
}

pub struct AudioReactiveHaptics {
    capture: SystemAudioCapture,
    state: AudioReactiveState,
    envelope: HapticEnvelope,
    last_sequence: u64,
    last_audio_at: Option<Instant>,
    last_output: HapticOutput,
    meter: AudioBands,
}

impl Default for AudioReactiveHaptics {
    fn default() -> Self {
        Self {
            capture: SystemAudioCapture,
            state: AudioReactiveState::Stopped,
            envelope: HapticEnvelope::default(),
            last_sequence: 0,
            last_audio_at: None,
            last_output: HapticOutput::OFF,
            meter: AudioBands::default(),
        }
    }
}

impl AudioReactiveHaptics {
    pub fn start(&mut self, now: Instant) -> AudioReactiveState {
        self.reset_processing();
        self.state = self.capture.start().into();
        self.last_audio_at = Some(now);
        self.state
    }

    /// Stops capture and returns whether a zero motor frame must be sent.
    pub fn stop(&mut self) -> bool {
        let was_running = self.state.is_running();
        let had_output = self.last_output != HapticOutput::OFF;
        self.capture.stop();
        self.state = AudioReactiveState::Stopped;
        self.reset_processing();
        was_running || had_output
    }

    pub fn state(&self) -> AudioReactiveState {
        self.state
    }

    pub fn meter(&self) -> AudioBands {
        self.meter
    }

    pub fn tick(
        &mut self,
        now: Instant,
        profile: &AudioReactiveHapticsProfile,
        maximum: u8,
    ) -> Option<HapticOutput> {
        let source_state: AudioReactiveState = self.capture.state().into();
        if source_state != AudioReactiveState::Running {
            let was_active = self.state.is_running() || self.last_output != HapticOutput::OFF;
            self.state = source_state;
            self.envelope.reset();
            self.meter = AudioBands::default();
            if was_active {
                self.last_output = HapticOutput::OFF;
                return Some(HapticOutput::OFF);
            }
            return None;
        }

        self.state = AudioReactiveState::Running;
        let snapshot = self.capture.snapshot();
        if snapshot.sequence != self.last_sequence {
            self.last_sequence = snapshot.sequence;
            self.last_audio_at = Some(now);
            self.meter = snapshot;
        }

        let source_bands = if self.last_audio_at.is_some_and(|last_audio_at| {
            now.duration_since(last_audio_at) <= AUDIO_HEARTBEAT_TIMEOUT
        }) {
            self.meter
        } else {
            AudioBands::default()
        };
        let output = self.envelope.update(source_bands, profile, maximum);

        if output == self.last_output {
            None
        } else {
            self.last_output = output;
            Some(output)
        }
    }

    fn reset_processing(&mut self) {
        self.envelope.reset();
        self.last_sequence = self.capture.snapshot().sequence;
        self.last_audio_at = None;
        self.last_output = HapticOutput::OFF;
        self.meter = AudioBands::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> AudioReactiveHapticsProfile {
        AudioReactiveHapticsProfile {
            sensitivity_percent: 100,
            threshold_percent: 0,
        }
    }

    #[test]
    fn threshold_silences_small_audio_levels() {
        let profile = AudioReactiveHapticsProfile {
            sensitivity_percent: 100,
            threshold_percent: 20,
        };

        assert_eq!(normalized_band(10_000, &profile), 0.0);
    }

    #[test]
    fn bass_or_detail_drives_the_same_level_on_both_motors() {
        let mut envelope = HapticEnvelope::default();
        let low = envelope.update(
            AudioBands {
                low: u16::MAX,
                high: 0,
                sequence: 1,
            },
            &profile(),
            200,
        );
        assert!(low.heavy > 0);
        assert!(low.is_symmetric());

        envelope.reset();
        let high = envelope.update(
            AudioBands {
                low: 0,
                high: u16::MAX,
                sequence: 2,
            },
            &profile(),
            200,
        );
        assert!(high.sharp > 0);
        assert!(high.is_symmetric());
        assert_eq!(low, high);
    }

    #[test]
    fn envelope_respects_the_shared_maximum() {
        let mut envelope = HapticEnvelope::default();
        let output = envelope.update(
            AudioBands {
                low: u16::MAX,
                high: u16::MAX,
                sequence: 1,
            },
            &profile(),
            80,
        );

        assert!(output.heavy <= 80);
        assert!(output.sharp <= 80);
        assert!(output.is_symmetric());
    }
}
