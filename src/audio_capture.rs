//! Small FFI wrapper around the native Core Audio Tap bridge.
//!
//! The native bridge owns the real-time Core Audio callback. It only publishes
//! the latest low/high-band levels through atomics; all HID I/O remains on the
//! Rust application thread.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AudioBands {
    /// Low-frequency RMS amplitude, encoded from `0` to `u16::MAX`.
    pub low: u16,
    /// High-frequency RMS amplitude, encoded from `0` to `u16::MAX`.
    pub high: u16,
    /// Monotonically increasing callback heartbeat.
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemAudioCaptureState {
    Idle,
    Running,
    Unsupported,
    Failed(i32),
}

#[derive(Default)]
pub struct SystemAudioCapture;

impl SystemAudioCapture {
    pub fn start(&mut self) -> SystemAudioCaptureState {
        #[cfg(target_os = "macos")]
        unsafe {
            ds_audio_capture_start();
        }
        self.state()
    }

    pub fn stop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            ds_audio_capture_stop();
        }
    }

    pub fn state(&self) -> SystemAudioCaptureState {
        #[cfg(target_os = "macos")]
        unsafe {
            match ds_audio_capture_state() {
                0 => SystemAudioCaptureState::Idle,
                1 => SystemAudioCaptureState::Running,
                2 => SystemAudioCaptureState::Unsupported,
                _ => SystemAudioCaptureState::Failed(ds_audio_capture_last_error()),
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            SystemAudioCaptureState::Unsupported
        }
    }

    pub fn snapshot(&self) -> AudioBands {
        #[cfg(target_os = "macos")]
        unsafe {
            let packed = ds_audio_capture_levels();
            AudioBands {
                low: ((packed >> 16) & u64::from(u16::MAX)) as u16,
                high: (packed & u64::from(u16::MAX)) as u16,
                sequence: ds_audio_capture_sequence(),
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            AudioBands::default()
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn ds_audio_capture_start() -> i32;
    fn ds_audio_capture_stop();
    fn ds_audio_capture_state() -> i32;
    fn ds_audio_capture_last_error() -> i32;
    fn ds_audio_capture_levels() -> u64;
    fn ds_audio_capture_sequence() -> u64;
}
