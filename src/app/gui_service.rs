//! JSON-over-stdio bridge between the native SwiftUI shell and Rust backend.
//!
//! The service keeps controller operations in Rust instead of exposing HID
//! operations through Swift. When the background daemon is present, it proxies
//! to that daemon so a GUI window never creates a second HID owner.

use std::{
    fs,
    io::{self, BufRead, BufReader, ErrorKind, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    dualsense::{DualSenseControl, HapticOutput},
    launch_agent,
    mapping::open_accessibility_settings,
    model::{
        AdaptiveTriggerMode, AdaptiveTriggerPreset, AudioRoute, Button, HapticDemo, KeyboardKey,
        MouseMappingProfile, PlayerIndicator, TriggerTarget,
    },
};

use super::ConfiguratorApp;

const SERVICE_TICK: Duration = Duration::from_millis(20);
const DAEMON_CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const DAEMON_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Deserialize)]
struct GuiRequest {
    #[serde(default)]
    id: u64,
    #[serde(flatten)]
    command: GuiCommand,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum GuiCommand {
    Snapshot,
    LiveState,
    Refresh,
    SelectDevice {
        index: usize,
    },
    SetLightbar {
        r: u8,
        g: u8,
        b: u8,
        #[serde(default)]
        apply: bool,
    },
    SetLightbarInactive {
        inactive: bool,
    },
    ReapplyLightbar,
    SetHaptics {
        enabled: bool,
        audio_haptics: bool,
        strength: u8,
        #[serde(default)]
        apply: bool,
    },
    SetAudioReactive {
        enabled: bool,
        sensitivity_percent: u8,
        threshold_percent: u8,
    },
    PlayHapticDemo {
        demo: HapticDemo,
    },
    SetTriggers {
        target: TriggerTarget,
        mode: AdaptiveTriggerMode,
        preset: AdaptiveTriggerPreset,
        intensity: u8,
        start_position: u8,
        end_position: u8,
        frequency: u8,
        #[serde(default)]
        apply: bool,
    },
    ResetTriggers,
    SetSystem {
        player_indicator: PlayerIndicator,
        microphone_muted: bool,
        speaker_volume: u8,
        microphone_volume: u8,
        audio_route: AudioRoute,
        #[serde(default)]
        apply: bool,
    },
    SetMouse {
        enabled: bool,
        pointer_speed: u8,
        deadzone_percent: u8,
        scroll_speed: u8,
    },
    SetControllerMapping {
        from: Button,
        to: Button,
    },
    SetKeyboardMapping {
        from: Button,
        to: KeyboardKey,
    },
    SetKeyboardOutput {
        enabled: bool,
    },
    RequestEventPostingAccess,
    OpenAccessibilitySettings,
    SaveProfile,
    SaveNamedProfile {
        name: String,
    },
    LoadNamedProfile {
        profile_id: String,
    },
    ResetProfile,
    InstallBackgroundAgent,
    UninstallBackgroundAgent,
    BackgroundStatus,
    Quit,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GuiResponse {
    id: u64,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot: Option<GuiSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    live_state: Option<GuiLiveState>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GuiSnapshot {
    app_name: &'static str,
    status: String,
    devices: Vec<crate::dualsense::DeviceInfo>,
    selected_device: usize,
    profile: crate::model::Profile,
    live_input: Option<crate::model::GamepadState>,
    input_status: String,
    keyboard_mapping_status: String,
    mouse_mapping_status: String,
    event_posting_granted: bool,
    event_posting_status: &'static str,
    profile_path: String,
    saved_profiles: Vec<crate::config::NamedProfile>,
    dirty: bool,
    audio_reactive: AudioReactiveSnapshot,
    background_agent: BackgroundAgentSnapshot,
}

/// Dynamic data polled by the desktop UI. Keeping it separate from the
/// complete profile avoids repeatedly serializing mappings and static settings
/// while the controller visualization or audio meter is open.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GuiLiveState {
    live_input: Option<GuiLiveInput>,
    input_status: String,
    audio_reactive: AudioReactiveSnapshot,
}

/// Fields rendered by the desktop Dashboard. Sensor, touch, and transport
/// diagnostics stay in the full snapshot because they do not change the live
/// visualization.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GuiLiveInput {
    left_stick: crate::model::StickState,
    right_stick: crate::model::StickState,
    left_trigger: u8,
    right_trigger: u8,
    buttons: Vec<Button>,
    battery_percent: Option<u8>,
    battery_status: crate::model::BatteryStatus,
    headset_connected: bool,
    microphone_connected: bool,
    microphone_muted: bool,
    packet_count: u64,
}

impl From<&crate::model::GamepadState> for GuiLiveInput {
    fn from(state: &crate::model::GamepadState) -> Self {
        Self {
            left_stick: state.left_stick,
            right_stick: state.right_stick,
            left_trigger: state.left_trigger,
            right_trigger: state.right_trigger,
            buttons: state.buttons.clone(),
            battery_percent: state.battery_percent,
            battery_status: state.battery_status,
            headset_connected: state.headset_connected,
            microphone_connected: state.microphone_connected,
            microphone_muted: state.microphone_muted,
            packet_count: state.packet_count,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioReactiveSnapshot {
    state: &'static str,
    running: bool,
    low: u16,
    high: u16,
}

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackgroundAgentSnapshot {
    installed: bool,
    loaded: bool,
}

#[derive(Clone, Copy)]
enum ResponsePayload {
    Snapshot,
    LiveState,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum GuiServiceMode {
    Local,
    Daemon,
}

struct DaemonRequest {
    line: String,
    response: mpsc::Sender<String>,
}

/// Local command endpoint owned by the LaunchAgent process.
///
/// GUI requests are serialized through the daemon's Rust thread. This keeps
/// IOKit output sequencing and controller ownership in one process while the
/// SwiftUI app is opened, unfocused, or closed.
pub(super) struct DaemonCommandServer {
    receiver: mpsc::Receiver<DaemonRequest>,
    background_agent: BackgroundAgentSnapshot,
    socket_path: PathBuf,
}

impl DaemonCommandServer {
    pub(super) fn start() -> Result<Self> {
        let socket_path = crate::config::daemon_socket_path();
        let listener = bind_daemon_socket(&socket_path)?;
        let (sender, receiver) = mpsc::channel();

        if let Err(error) = thread::Builder::new()
            .name("dualsense-gui-socket".to_string())
            .spawn(move || {
                if let Err(error) = serve_daemon_clients(listener, sender) {
                    eprintln!("DualSenseTUI daemon command socket stopped: {error:#}");
                }
            })
        {
            let _ = fs::remove_file(&socket_path);
            return Err(error).context("failed to start the daemon command socket");
        }

        Ok(Self {
            receiver,
            background_agent: read_background_agent_state(),
            socket_path,
        })
    }

    /// Wait for a command instead of sleeping the daemon loop. A GUI action
    /// wakes the controller thread immediately, while input still refreshes at
    /// the requested cadence when no command arrives.
    pub(super) fn wait_for_commands(&mut self, app: &mut ConfiguratorApp, timeout: Duration) {
        match self.receiver.recv_timeout(timeout) {
            Ok(request) => {
                self.respond(app, request);
                while let Ok(request) = self.receiver.try_recv() {
                    self.respond(app, request);
                }
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
        }
    }

    fn respond(&mut self, app: &mut ConfiguratorApp, request: DaemonRequest) {
        let (_, response) = response_for_line(
            app,
            &mut self.background_agent,
            &request.line,
            GuiServiceMode::Daemon,
        );
        let _ = request.response.send(serialize_response(&response));
    }
}

impl Drop for DaemonCommandServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
    }
}

pub(super) fn run() -> Result<()> {
    if try_run_daemon_proxy()? {
        return Ok(());
    }

    run_local_service()
}

fn run_local_service() -> Result<()> {
    let mut app = ConfiguratorApp::new()?;
    let (sender, receiver) = mpsc::channel::<String>();
    let _reader = thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut background_agent = read_background_agent_state();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut running = true;

    while running {
        app.update_input_state();
        app.tick_audio_reactive_haptics();

        match receiver.recv_timeout(SERVICE_TICK) {
            Ok(line) => {
                running = process_line(&mut app, &mut background_agent, &line, &mut stdout)?;
                while running {
                    match receiver.try_recv() {
                        Ok(line) => {
                            running =
                                process_line(&mut app, &mut background_agent, &line, &mut stdout)?;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            running = false;
                            break;
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    app.shutdown_haptics();
    Ok(())
}

/// Connect the stdio process to an already running daemon. The proxy is
/// intentionally synchronous: the Swift shell already sends line-delimited
/// commands, and one response per request preserves the existing protocol.
fn try_run_daemon_proxy() -> Result<bool> {
    let socket_path = crate::config::daemon_socket_path();
    let background_agent = read_background_agent_state();
    if !should_proxy_to_daemon(background_agent) {
        return Ok(false);
    }
    let retry_deadline = Instant::now() + DAEMON_CONNECT_TIMEOUT;

    loop {
        match UnixStream::connect(&socket_path) {
            Ok(stream) => {
                run_daemon_proxy(stream)?;
                return Ok(true);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::NotFound | ErrorKind::ConnectionRefused
                ) =>
            {
                if Instant::now() < retry_deadline {
                    thread::sleep(DAEMON_CONNECT_RETRY_INTERVAL);
                    continue;
                }
                if !should_proxy_to_daemon(read_background_agent_state()) {
                    return Ok(false);
                }

                bail!(
                    "Background service is installed but cannot accept GUI commands. Restart it from Profiles, then reopen DualSenseTUI."
                );
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to connect to the background service at {}",
                        socket_path.display()
                    )
                });
            }
        }
    }
}

fn run_daemon_proxy(mut stream: UnixStream) -> Result<()> {
    let response_stream = stream
        .try_clone()
        .context("failed to clone the background-service connection")?;
    let mut responses = BufReader::new(response_stream);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.context("failed to read a GUI command from stdin")?;
        // `quit` terminates this short-lived stdio proxy. It must never stop
        // the LaunchAgent, which owns persistent controller configuration.
        if is_quit_request(&line) {
            break;
        }

        stream
            .write_all(line.as_bytes())
            .context("failed to send a GUI command to the background service")?;
        stream
            .write_all(b"\n")
            .context("failed to terminate the background-service command")?;
        stream
            .flush()
            .context("failed to flush the background-service command")?;

        let mut response = String::new();
        if responses
            .read_line(&mut response)
            .context("failed to read the background-service response")?
            == 0
        {
            bail!("background service closed its command connection");
        }
        stdout
            .write_all(response.as_bytes())
            .context("failed to write the GUI response")?;
        stdout.flush().context("failed to flush the GUI response")?;
    }

    Ok(())
}

fn is_quit_request(line: &str) -> bool {
    serde_json::from_str::<GuiRequest>(line)
        .is_ok_and(|request| matches!(request.command, GuiCommand::Quit))
}

fn bind_daemon_socket(path: &Path) -> Result<UnixListener> {
    let parent = path
        .parent()
        .context("daemon command socket path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create daemon command socket directory {}",
            parent.display()
        )
    })?;

    let listener = match UnixListener::bind(path) {
        Ok(listener) => listener,
        Err(error) if error.kind() == ErrorKind::AddrInUse => match UnixStream::connect(path) {
            Ok(_) => bail!(
                "another background service already owns the command socket {}",
                path.display()
            ),
            Err(probe_error) if probe_error.kind() == ErrorKind::ConnectionRefused => {
                fs::remove_file(path).with_context(|| {
                    format!("failed to remove stale daemon socket {}", path.display())
                })?;
                UnixListener::bind(path).with_context(|| {
                    format!(
                        "failed to recreate daemon command socket {}",
                        path.display()
                    )
                })?
            }
            Err(probe_error) => {
                return Err(probe_error).with_context(|| {
                    format!(
                        "failed to inspect existing daemon socket {}",
                        path.display()
                    )
                });
            }
        },
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to bind daemon command socket {}", path.display())
            });
        }
    };

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to secure daemon command socket {}", path.display()))?;
    Ok(listener)
}

fn serve_daemon_clients(listener: UnixListener, sender: mpsc::Sender<DaemonRequest>) -> Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let sender = sender.clone();
                if let Err(error) = thread::Builder::new()
                    .name("dualsense-gui-command".to_string())
                    .spawn(move || {
                        if let Err(error) = serve_daemon_client(stream, &sender) {
                            eprintln!("DualSenseTUI GUI command connection closed: {error:#}");
                        }
                    })
                {
                    eprintln!("DualSenseTUI could not handle a GUI command connection: {error}");
                }
            }
            Err(error) => return Err(error).context("failed to accept GUI command connection"),
        }
    }
    Ok(())
}

fn serve_daemon_client(mut stream: UnixStream, sender: &mpsc::Sender<DaemonRequest>) -> Result<()> {
    let request_stream = stream
        .try_clone()
        .context("failed to clone GUI command connection")?;
    let mut requests = BufReader::new(request_stream);

    loop {
        let mut line = String::new();
        if requests
            .read_line(&mut line)
            .context("failed to read GUI command")?
            == 0
        {
            return Ok(());
        }

        let (response_sender, response_receiver) = mpsc::channel();
        sender
            .send(DaemonRequest {
                line,
                response: response_sender,
            })
            .map_err(|_| {
                anyhow::anyhow!("background service stopped before handling GUI command")
            })?;
        let response = response_receiver.recv().map_err(|_| {
            anyhow::anyhow!("background service stopped while handling GUI command")
        })?;
        stream
            .write_all(response.as_bytes())
            .context("failed to write GUI response")?;
        stream
            .write_all(b"\n")
            .context("failed to terminate GUI response")?;
        stream.flush().context("failed to flush GUI response")?;
    }
}

fn process_line(
    app: &mut ConfiguratorApp,
    background_agent: &mut BackgroundAgentSnapshot,
    line: &str,
    stdout: &mut impl Write,
) -> Result<bool> {
    let (should_quit, response) =
        response_for_line(app, background_agent, line, GuiServiceMode::Local);
    let serialized = serialize_response(&response);
    stdout.write_all(serialized.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(!should_quit)
}

fn response_for_line(
    app: &mut ConfiguratorApp,
    background_agent: &mut BackgroundAgentSnapshot,
    line: &str,
    mode: GuiServiceMode,
) -> (bool, GuiResponse) {
    let (id, ok, error, should_quit, payload) = match serde_json::from_str::<GuiRequest>(line) {
        Ok(request) => {
            let payload = match &request.command {
                GuiCommand::LiveState => ResponsePayload::LiveState,
                _ => ResponsePayload::Snapshot,
            };

            match apply_command(app, background_agent, request.command, mode) {
                Ok(should_quit) => (request.id, true, None, should_quit, payload),
                Err(error) => {
                    let error = format!("{error:#}");
                    app.status = format!("Action failed: {error}");
                    (
                        request.id,
                        false,
                        Some(error),
                        false,
                        ResponsePayload::Snapshot,
                    )
                }
            }
        }
        Err(error) => (
            0,
            false,
            Some(format!("invalid GUI request: {error}")),
            false,
            ResponsePayload::Snapshot,
        ),
    };

    let response = match payload {
        ResponsePayload::Snapshot => GuiResponse {
            id,
            ok,
            error,
            snapshot: Some(snapshot(app, *background_agent)),
            live_state: None,
        },
        ResponsePayload::LiveState => GuiResponse {
            id,
            ok,
            error,
            snapshot: None,
            live_state: Some(live_state(app)),
        },
    };
    (should_quit, response)
}

fn serialize_response(response: &GuiResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|error| {
        serde_json::json!({
            "id": response.id,
            "ok": false,
            "error": format!("failed to encode GUI response: {error}"),
        })
        .to_string()
    })
}

fn apply_command(
    app: &mut ConfiguratorApp,
    background_agent: &mut BackgroundAgentSnapshot,
    command: GuiCommand,
    mode: GuiServiceMode,
) -> Result<bool> {
    match command {
        GuiCommand::Snapshot | GuiCommand::LiveState => {}
        GuiCommand::Refresh => app.refresh_devices(),
        GuiCommand::SelectDevice { index } => select_device(app, index)?,
        GuiCommand::SetLightbar { r, g, b, apply } => {
            app.profile.lightbar = crate::model::Rgb::new(r, g, b);
            app.dirty = true;
            if apply {
                app.apply_lightbar();
            } else {
                app.status = format!("Lightbar staged: #{r:02x}{g:02x}{b:02x}");
            }
        }
        GuiCommand::SetLightbarInactive { inactive } => {
            app.set_lightbar_inactive(inactive);
        }
        GuiCommand::ReapplyLightbar => app.reapply_lightbar_after_app_resign(),
        GuiCommand::SetHaptics {
            enabled,
            audio_haptics,
            strength,
            apply,
        } => {
            app.profile.haptics.enabled = enabled;
            app.profile.haptics.audio_haptics = audio_haptics;
            app.profile.haptics.set_strength(strength);
            app.dirty = true;
            if apply {
                apply_haptics(app);
            } else {
                app.status = "Haptics staged".to_string();
            }
        }
        GuiCommand::SetAudioReactive {
            enabled,
            sensitivity_percent,
            threshold_percent,
        } => {
            let reactive = &mut app.profile.haptics.audio_reactive;
            reactive.sensitivity_percent = sensitivity_percent.clamp(25, 250);
            reactive.threshold_percent = threshold_percent.min(90);
            app.dirty = true;
            if enabled != app.audio_reactive.state().is_running() {
                app.toggle_audio_reactive_haptics();
            } else {
                app.status = "Audio-reactive haptics updated".to_string();
            }
        }
        GuiCommand::PlayHapticDemo { demo } => {
            app.selected_haptic_demo = demo;
            app.play_haptic_demo();
        }
        GuiCommand::SetTriggers {
            target,
            mode,
            preset,
            intensity,
            start_position,
            end_position,
            frequency,
            apply,
        } => {
            let triggers = &mut app.profile.adaptive_triggers;
            triggers.target = target;
            triggers.mode = mode;
            triggers.preset = preset;
            triggers.intensity = intensity;
            triggers.start_position = start_position.min(9);
            triggers.end_position = end_position.max(triggers.start_position).min(9);
            triggers.frequency = frequency.max(1);
            app.dirty = true;
            if apply {
                app.apply_adaptive_triggers();
            } else {
                app.status = "Adaptive triggers staged".to_string();
            }
        }
        GuiCommand::ResetTriggers => app.reset_adaptive_triggers(),
        GuiCommand::SetSystem {
            player_indicator,
            microphone_muted,
            speaker_volume,
            microphone_volume,
            audio_route,
            apply,
        } => {
            let system = &mut app.profile.system;
            system.player_indicator = player_indicator;
            system.microphone_muted = microphone_muted;
            system.speaker_volume = speaker_volume;
            system.microphone_volume = microphone_volume.min(0x40);
            system.audio_route = audio_route;
            app.dirty = true;
            if apply {
                app.apply_system_controls();
            } else {
                app.status = "System controls staged".to_string();
            }
        }
        GuiCommand::SetMouse {
            enabled,
            pointer_speed,
            deadzone_percent,
            scroll_speed,
        } => set_mouse_output(app, enabled, pointer_speed, deadzone_percent, scroll_speed),
        GuiCommand::SetControllerMapping { from, to } => {
            if let Some(mapping) = app
                .profile
                .mappings
                .iter_mut()
                .find(|mapping| mapping.from == from)
            {
                mapping.to = to;
            }
            app.profile.normalize_mappings();
            app.dirty = true;
            app.status = format!("Controller mapping updated: {}", from.label());
        }
        GuiCommand::SetKeyboardMapping { from, to } => {
            if let Some(binding) = app
                .profile
                .keyboard_mapping
                .bindings
                .iter_mut()
                .find(|binding| binding.from == from)
            {
                binding.to = to;
            }
            app.profile.keyboard_mapping.normalize_bindings();
            app.dirty = true;
            app.status = format!("Keyboard mapping updated: {}", from.label());
        }
        GuiCommand::SetKeyboardOutput { enabled } => set_keyboard_output(app, enabled),
        GuiCommand::RequestEventPostingAccess => request_event_posting_access(app),
        GuiCommand::OpenAccessibilitySettings => match open_accessibility_settings() {
            Ok(()) => app.status = "Accessibility settings opened".to_string(),
            Err(error) => app.status = format!("Could not open Accessibility settings: {error:#}"),
        },
        GuiCommand::SaveProfile => app.save_profile(),
        GuiCommand::SaveNamedProfile { name } => app.save_named_profile(&name),
        GuiCommand::LoadNamedProfile { profile_id } => app.load_named_profile(&profile_id),
        GuiCommand::ResetProfile => app.reset_profile_to_defaults(),
        GuiCommand::InstallBackgroundAgent if mode == GuiServiceMode::Daemon => {
            bail!("Background Service cannot modify its own LaunchAgent connection")
        }
        GuiCommand::InstallBackgroundAgent => {
            let status = launch_agent::install_current_executable()?;
            *background_agent = BackgroundAgentSnapshot {
                installed: status.installed,
                loaded: status.loaded,
            };
            app.status = "Background service enabled".to_string();
        }
        GuiCommand::UninstallBackgroundAgent if mode == GuiServiceMode::Daemon => {
            bail!("Background Service cannot modify its own LaunchAgent connection")
        }
        GuiCommand::UninstallBackgroundAgent => {
            launch_agent::uninstall()?;
            *background_agent = read_background_agent_state();
            app.status = "Background service disabled".to_string();
        }
        GuiCommand::BackgroundStatus => {
            *background_agent = read_background_agent_state();
            app.status = if background_agent.loaded {
                "Background service is active".to_string()
            } else if background_agent.installed {
                "Background service is installed but not loaded".to_string()
            } else {
                "Background service is disabled".to_string()
            };
        }
        GuiCommand::Quit => return Ok(true),
    }
    Ok(false)
}

fn select_device(app: &mut ConfiguratorApp, index: usize) -> Result<()> {
    if index >= app.devices.len() {
        bail!("controller index {index} is unavailable")
    }
    if index == app.selected_device {
        return Ok(());
    }

    let _ = app.stop_audio_reactive_haptics();
    app.release_output_mappings();
    app.selected_device = index;
    app.live_input = None;
    app.activate_selected_profile()?;
    app.schedule_auto_apply();
    app.status = "Selected controller profile".to_string();
    Ok(())
}

fn apply_haptics(app: &mut ConfiguratorApp) {
    if app.devices.is_empty() {
        app.status = "No DualSense selected".to_string();
        return;
    }

    let output = if app.profile.haptics.enabled {
        HapticOutput::symmetric(app.profile.haptics.strength())
    } else {
        HapticOutput::OFF
    };
    match app.backend.set_haptics(
        app.selected_device,
        output,
        app.profile.haptics.audio_haptics,
    ) {
        Ok(()) if app.profile.haptics.enabled => {
            app.status = format!("Haptics applied: {}", app.profile.haptics.strength());
        }
        Ok(()) => app.status = "Haptics disabled".to_string(),
        Err(error) => app.status = format!("Haptics failed: {error:#}"),
    }
}

fn set_mouse_output(
    app: &mut ConfiguratorApp,
    enabled: bool,
    pointer_speed: u8,
    deadzone_percent: u8,
    scroll_speed: u8,
) {
    let mouse = &mut app.profile.mouse_mapping;
    mouse.pointer_speed = pointer_speed.clamp(
        MouseMappingProfile::MIN_POINTER_SPEED,
        MouseMappingProfile::MAX_POINTER_SPEED,
    );
    mouse.deadzone_percent = deadzone_percent.min(MouseMappingProfile::MAX_DEADZONE_PERCENT);
    mouse.scroll_speed = scroll_speed.clamp(
        MouseMappingProfile::MIN_SCROLL_SPEED,
        MouseMappingProfile::MAX_SCROLL_SPEED,
    );

    if !enabled {
        mouse.enabled = false;
        app.release_mouse_mapping();
        app.mouse_mapping_status = format!(
            "Mouse output disabled ({})",
            app.mouse_mapper.permission_status()
        );
        app.status = "Mouse output disabled".to_string();
        app.dirty = true;
        return;
    }

    if !app.mouse_mapper.can_post_events() && !app.mouse_mapper.request_event_posting_access() {
        mouse.enabled = false;
        app.mouse_mapping_status = format!(
            "Mouse output unavailable ({})",
            app.mouse_mapper.permission_status()
        );
        app.status = "Grant Accessibility event-posting access to enable mouse output".to_string();
        return;
    }

    mouse.enabled = true;
    app.mouse_mapping_status = "Mouse output active; switch focus to the target app".to_string();
    app.status = "Mouse output enabled".to_string();
    app.dirty = true;
}

fn set_keyboard_output(app: &mut ConfiguratorApp, enabled: bool) {
    if !enabled {
        app.profile.keyboard_mapping.enabled = false;
        app.release_keyboard_mapping();
        app.mapping_status = format!(
            "Keyboard output disabled ({})",
            app.keyboard_mapper.permission_status()
        );
        app.status = "Keyboard output disabled".to_string();
        app.dirty = true;
        return;
    }

    if !app.keyboard_mapper.can_post_events() && !app.keyboard_mapper.request_event_posting_access()
    {
        app.profile.keyboard_mapping.enabled = false;
        app.mapping_status = format!(
            "Keyboard output unavailable ({})",
            app.keyboard_mapper.permission_status()
        );
        app.status =
            "Grant Accessibility event-posting access to enable keyboard output".to_string();
        return;
    }

    app.profile.keyboard_mapping.enabled = true;
    app.mapping_status = "Keyboard output active; switch focus to the target app".to_string();
    app.status = "Keyboard output enabled".to_string();
    app.dirty = true;
}

fn request_event_posting_access(app: &mut ConfiguratorApp) {
    if app.mouse_mapper.request_event_posting_access() {
        app.status = "Accessibility event-posting access granted".to_string();
    } else {
        app.status = "Accessibility event-posting access is required for keyboard and mouse output"
            .to_string();
    }
}

fn snapshot(app: &ConfiguratorApp, background_agent: BackgroundAgentSnapshot) -> GuiSnapshot {
    GuiSnapshot {
        app_name: "DualSenseTUI",
        status: app.status.clone(),
        devices: app.devices.clone(),
        selected_device: app.selected_device,
        profile: app.profile.clone(),
        live_input: app.live_input.clone(),
        input_status: app.input_status.clone(),
        keyboard_mapping_status: app.mapping_status.clone(),
        mouse_mapping_status: app.mouse_mapping_status.clone(),
        event_posting_granted: app.mouse_mapper.can_post_events(),
        event_posting_status: app.mouse_mapper.permission_status(),
        profile_path: app.profile_path.clone(),
        saved_profiles: crate::config::list_named_profiles().unwrap_or_default(),
        dirty: app.dirty,
        audio_reactive: audio_reactive_snapshot(app),
        background_agent,
    }
}

fn live_state(app: &ConfiguratorApp) -> GuiLiveState {
    GuiLiveState {
        live_input: app.live_input.as_ref().map(GuiLiveInput::from),
        input_status: app.input_status.clone(),
        audio_reactive: audio_reactive_snapshot(app),
    }
}

fn audio_reactive_snapshot(app: &ConfiguratorApp) -> AudioReactiveSnapshot {
    let audio_state = app.audio_reactive.state();
    let meter = app.audio_reactive.meter();
    AudioReactiveSnapshot {
        state: audio_state.label(),
        running: audio_state.is_running(),
        low: meter.low,
        high: meter.high,
    }
}

fn read_background_agent_state() -> BackgroundAgentSnapshot {
    launch_agent::status()
        .map(|status| BackgroundAgentSnapshot {
            installed: status.installed,
            loaded: status.loaded,
        })
        .unwrap_or_default()
}

const fn should_proxy_to_daemon(status: BackgroundAgentSnapshot) -> bool {
    status.loaded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lightbar_command() {
        let request: GuiRequest = serde_json::from_str(
            r#"{"id":7,"command":"set_lightbar","r":12,"g":34,"b":56,"apply":true}"#,
        )
        .unwrap();

        assert_eq!(request.id, 7);
        assert!(matches!(
            request.command,
            GuiCommand::SetLightbar {
                r: 12,
                g: 34,
                b: 56,
                apply: true
            }
        ));
    }

    #[test]
    fn parses_lightbar_inactive_command() {
        let request: GuiRequest =
            serde_json::from_str(r#"{"id":8,"command":"set_lightbar_inactive","inactive":true}"#)
                .unwrap();

        assert_eq!(request.id, 8);
        assert!(matches!(
            request.command,
            GuiCommand::SetLightbarInactive { inactive: true }
        ));
    }

    #[test]
    fn parses_lightbar_reapply_command() {
        let request: GuiRequest =
            serde_json::from_str(r#"{"id":9,"command":"reapply_lightbar"}"#).unwrap();

        assert_eq!(request.id, 9);
        assert!(matches!(request.command, GuiCommand::ReapplyLightbar));
    }

    #[test]
    fn parses_live_state_command() {
        let request: GuiRequest =
            serde_json::from_str(r#"{"id":11,"command":"live_state"}"#).unwrap();

        assert_eq!(request.id, 11);
        assert!(matches!(request.command, GuiCommand::LiveState));
    }

    #[test]
    fn live_input_snapshot_omits_unrendered_sensor_data() {
        let payload = GuiLiveInput::from(&crate::model::GamepadState::default());
        let json = serde_json::to_value(payload).unwrap();

        assert!(json.get("leftStick").is_some());
        assert!(json.get("packetCount").is_some());
        assert!(json.get("motion").is_none());
        assert!(json.get("touchPoints").is_none());
    }

    #[test]
    fn parses_trigger_command_with_internal_profile_enums() {
        let request: GuiRequest = serde_json::from_str(
            r#"{
                "id":3,
                "command":"set_triggers",
                "target":"Both",
                "mode":"Preset",
                "preset":"MachineGun",
                "intensity":200,
                "start_position":2,
                "end_position":9,
                "frequency":38,
                "apply":true
            }"#,
        )
        .unwrap();

        assert!(matches!(
            request.command,
            GuiCommand::SetTriggers {
                target: TriggerTarget::Both,
                mode: AdaptiveTriggerMode::Preset,
                preset: AdaptiveTriggerPreset::MachineGun,
                intensity: 200,
                start_position: 2,
                end_position: 9,
                frequency: 38,
                apply: true
            }
        ));
    }

    #[test]
    fn parses_profile_library_commands() {
        let save: GuiRequest =
            serde_json::from_str(r#"{"id":5,"command":"save_named_profile","name":"Racing"}"#)
                .unwrap();
        assert!(matches!(
            save.command,
            GuiCommand::SaveNamedProfile { name } if name == "Racing"
        ));

        let load: GuiRequest = serde_json::from_str(
            r#"{"id":6,"command":"load_named_profile","profile_id":"racing"}"#,
        )
        .unwrap();
        assert!(matches!(
            load.command,
            GuiCommand::LoadNamedProfile { profile_id } if profile_id == "racing"
        ));

        let reset: GuiRequest =
            serde_json::from_str(r#"{"id":7,"command":"reset_profile"}"#).unwrap();
        assert!(matches!(reset.command, GuiCommand::ResetProfile));
    }

    #[test]
    fn unloaded_agent_uses_the_local_gui_service() {
        assert!(!should_proxy_to_daemon(BackgroundAgentSnapshot {
            installed: true,
            loaded: false,
        }));
        assert!(should_proxy_to_daemon(BackgroundAgentSnapshot {
            installed: true,
            loaded: true,
        }));
    }

    #[test]
    fn daemon_client_relays_one_request_and_response() {
        use std::net::Shutdown;

        let (mut client, server) = UnixStream::pair().unwrap();
        let response_stream = client.try_clone().unwrap();
        let mut responses = BufReader::new(response_stream);
        let (sender, receiver) = mpsc::channel();
        let server_thread = thread::spawn(move || serve_daemon_client(server, &sender));

        client
            .write_all(b"{\"id\":12,\"command\":\"snapshot\"}\n")
            .unwrap();
        client.flush().unwrap();

        let request = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(request.line.contains("\"id\":12"));
        request
            .response
            .send("{\"id\":12,\"ok\":true}".to_string())
            .unwrap();

        let mut response = String::new();
        responses.read_line(&mut response).unwrap();
        assert_eq!(response, "{\"id\":12,\"ok\":true}\n");

        client.shutdown(Shutdown::Write).unwrap();
        drop(client);
        assert!(server_thread.join().unwrap().is_ok());
    }

    #[test]
    fn quit_is_kept_local_to_the_gui_proxy() {
        assert!(is_quit_request(r#"{"id":4,"command":"quit"}"#));
        assert!(!is_quit_request(r#"{"id":4,"command":"snapshot"}"#));
    }
}
