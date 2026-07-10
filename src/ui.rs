use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

use crate::{
    app::{
        ConfiguratorApp, HapticField, LightbarField, MappingView, MouseField, SystemField, Tab,
        TriggerField,
    },
    model::{AdaptiveTriggerPreset, Button, GamepadState, HapticDemo, Rgb, StickState},
};

pub fn draw(frame: &mut Frame<'_>, app: &ConfiguratorApp) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(app.layout.controls_height),
        ])
        .split(frame.area());

    render_app_title(frame, root[0]);
    render_tabs(frame, root[1], app);

    if root[2].width < 96 {
        let status_height = scaled_panel_height(app.layout.status_size, 9, 4, 10);
        if app.active_tab == Tab::Devices {
            let devices_height = scaled_panel_height(app.layout.devices_size, 6, 3, 10);
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(devices_height),
                    Constraint::Min(3),
                    Constraint::Length(status_height),
                ])
                .split(root[2]);
            render_devices(frame, body[0], app);
            render_device_details(frame, body[1], app);
            render_status(frame, body[2], app);
        } else {
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(8), Constraint::Length(status_height)])
                .split(root[2]);
            render_editor(frame, body[0], app);
            render_status(frame, body[1], app);
        }
    } else {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(app.layout.devices_size),
                Constraint::Min(44),
                Constraint::Length(app.layout.status_size),
            ])
            .split(root[2]);
        render_devices(frame, body[0], app);
        render_editor(frame, body[1], app);
        render_status(frame, body[2], app);
    }

    render_footer(frame, root[3], app);
}

fn scaled_panel_height(size: u16, divisor: u16, min: u16, max: u16) -> u16 {
    (size / divisor).clamp(min, max)
}

fn render_app_title(frame: &mut Frame<'_>, area: Rect) {
    let title = Line::from(Span::styled(
        "DualSenseTUI",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    let paragraph = Paragraph::new(title).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" App ")
            .border_style(focus_style())
            .title_style(focus_style()),
    );
    frame.render_widget(paragraph, area);
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let titles = Tab::ALL
        .into_iter()
        .map(|tab| {
            let title = if area.width < 88 {
                tab.compact_title()
            } else {
                tab.title()
            };
            if tab == app.active_tab {
                Line::from(format!("> {title}"))
            } else {
                Line::from(title)
            }
        })
        .collect::<Vec<_>>();
    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .block(panel_block("Navigation", false))
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, area);
}

fn render_devices(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let focused = app.active_tab == Tab::Devices;
    let items = if app.devices.is_empty() {
        vec![ListItem::new("  No DualSense")]
    } else {
        app.devices
            .iter()
            .enumerate()
            .map(|(index, device)| {
                let selected = index == app.selected_device;
                let marker = if selected { ">" } else { " " };
                let line = Line::from(vec![
                    Span::raw(marker),
                    Span::raw(" "),
                    Span::styled(
                        device.name.clone(),
                        if selected && focused {
                            selected_row_style()
                        } else if selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::raw(format!(
                        " {:04x}:{:04x}",
                        device.vendor_id, device.product_id
                    )),
                ]);
                ListItem::new(line)
            })
            .collect()
    };

    let list = List::new(items).block(panel_block("Devices", focused));
    frame.render_widget(list, area);
}

fn render_editor(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    match app.active_tab {
        Tab::Devices => render_device_details(frame, area, app),
        Tab::Input => render_input(frame, area, app),
        Tab::Sensors => render_sensors(frame, area, app),
        Tab::Lightbar => render_lightbar(frame, area, app),
        Tab::Haptics => render_haptics(frame, area, app),
        Tab::Triggers => render_adaptive_triggers(frame, area, app),
        Tab::System => render_system(frame, area, app),
        Tab::Mapping => render_mapping(frame, area, app),
    }
}

fn render_input(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    let header = if let Some(state) = &app.live_input {
        let battery = state
            .battery_percent
            .map(|value| format!("{value}%"))
            .unwrap_or_else(|| "n/a".to_string());
        let headset = if state.headset_connected { "yes" } else { "no" };
        let microphone = if state.microphone_connected {
            if state.microphone_muted {
                "connected, muted"
            } else {
                "connected"
            }
        } else {
            "not connected"
        };
        Line::from(vec![
            Span::styled("Input: ", label_style()),
            Span::raw(&app.input_status),
            Span::raw("  "),
            Span::styled("Battery: ", label_style()),
            Span::raw(format!("{} {}", battery, state.battery_status.label())),
            Span::raw("  "),
            Span::styled("Headset: ", label_style()),
            Span::raw(headset),
            Span::raw("  "),
            Span::styled("Mic: ", label_style()),
            Span::raw(microphone),
        ])
    } else {
        Line::from(vec![
            Span::styled("Input: ", label_style()),
            Span::raw(&app.input_status),
        ])
    };
    frame.render_widget(
        Paragraph::new(header).block(panel_block("Live", app.active_tab == Tab::Input)),
        chunks[0],
    );

    if let Some(state) = &app.live_input {
        render_gamepad(frame, chunks[1], state);
        render_input_triggers(frame, chunks[2], state);
    } else {
        let paragraph = Paragraph::new("Move a stick or press any DualSense button.")
            .block(panel_block("Gamepad", app.active_tab == Tab::Input))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, chunks[1]);
    }
}

fn render_sensors(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(3),
        ])
        .split(area);

    let Some(state) = &app.live_input else {
        let paragraph = Paragraph::new("Waiting for a DualSense input report.")
            .block(panel_block("Sensors", app.active_tab == Tab::Sensors))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
        return;
    };

    let touchpad = vec![
        Line::from(vec![
            Span::styled("Touch 1: ", label_style()),
            Span::raw(touch_point_label(state.touch_points[0])),
        ]),
        Line::from(vec![
            Span::styled("Touch 2: ", label_style()),
            Span::raw(touch_point_label(state.touch_points[1])),
        ]),
        Line::from("Surface: 1920 x 1080"),
    ];
    frame.render_widget(
        Paragraph::new(touchpad).block(panel_block("Touchpad", app.active_tab == Tab::Sensors)),
        chunks[0],
    );

    let motion = &state.motion;
    let motion_lines = vec![
        Line::from(vec![
            Span::styled("Gyro:  ", label_style()),
            Span::raw(format!(
                "x {:>6}  y {:>6}  z {:>6} raw",
                motion.gyro[0], motion.gyro[1], motion.gyro[2]
            )),
        ]),
        Line::from(vec![
            Span::styled("Accel: ", label_style()),
            Span::raw(format!(
                "x {:>6}  y {:>6}  z {:>6} raw",
                motion.accel[0], motion.accel[1], motion.accel[2]
            )),
        ]),
        Line::from(format!("Timestamp: {} ticks", motion.sensor_timestamp)),
    ];
    frame.render_widget(
        Paragraph::new(motion_lines).block(panel_block("Six-axis", false)),
        chunks[1],
    );

    let crc = match state.bluetooth_crc_valid {
        Some(true) => "valid",
        Some(false) => "invalid",
        None => "not present",
    };
    let diagnostics = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Input sequence: ", label_style()),
            Span::raw(state.report_sequence.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Bluetooth CRC: ", label_style()),
            Span::raw(crc),
        ]),
        Line::from("Raw sensor units; calibration data remains in the controller feature report."),
    ])
    .block(panel_block("Diagnostics", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(diagnostics, chunks[2]);
}

const SYSTEM_SECTION_COUNT: usize = 5;

fn system_constraints() -> [Constraint; SYSTEM_SECTION_COUNT] {
    [
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(3),
    ]
}

fn render_system(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(system_constraints())
        .split(area);
    let system = &app.profile.system;

    let indicator_selected = app.selected_system_field == SystemField::PlayerIndicator;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Player LEDs: ", label_style()),
            Span::styled(
                system.player_indicator.label(),
                selected_value_style(indicator_selected),
            ),
        ]))
        .block(panel_block("Player indicator", indicator_selected)),
        chunks[0],
    );

    let mute_selected = app.selected_system_field == SystemField::MicrophoneMute;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Microphone: ", label_style()),
            Span::styled(
                if system.microphone_muted {
                    "Muted"
                } else {
                    "Enabled"
                },
                selected_value_style(mute_selected),
            ),
        ]))
        .block(panel_block("Mic mute and LED", mute_selected)),
        chunks[1],
    );

    render_system_volume(
        frame,
        chunks[2],
        "Controller speaker",
        system.speaker_volume,
        255,
        app.selected_system_field == SystemField::SpeakerVolume,
    );
    render_system_volume(
        frame,
        chunks[3],
        "Microphone level",
        system.microphone_volume,
        0x40,
        app.selected_system_field == SystemField::MicrophoneVolume,
    );

    let route_selected = app.selected_system_field == SystemField::AudioRoute;
    let transport = app
        .devices
        .get(app.selected_device)
        .map(|device| {
            if device.supports_usb_audio() {
                "USB: audio endpoint is managed by macOS CoreAudio"
            } else {
                "Bluetooth: controller audio endpoint is not exposed by this backend"
            }
        })
        .unwrap_or("No DualSense selected");
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Output route: ", label_style()),
                Span::styled(
                    system.audio_route.label(),
                    selected_value_style(route_selected),
                ),
            ]),
            Line::from(transport),
            Line::from("Apply writes player LEDs, mute, volumes, and the selected route."),
        ])
        .block(panel_block("Audio and output", route_selected))
        .wrap(Wrap { trim: true }),
        chunks[4],
    );
}

fn render_system_volume(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    value: u8,
    maximum: u8,
    selected: bool,
) {
    let gauge = Gauge::default()
        .block(panel_block(title, selected))
        .gauge_style(gauge_style(selected, Color::Blue))
        .ratio(f64::from(value) / f64::from(maximum))
        .label(if selected {
            format!("> {value:3} <")
        } else {
            format!("{value:3}")
        });
    frame.render_widget(gauge, area);
}

fn touch_point_label(point: Option<crate::model::TouchPoint>) -> String {
    match point {
        Some(point) => format!(
            "id {}  x {:>4}  y {:>4}",
            point.contact_id, point.x, point.y
        ),
        None => "inactive".to_string(),
    }
}

fn render_device_details(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let text = if let Some(device) = app.devices.get(app.selected_device) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name: ", label_style()),
                Span::raw(&device.name),
            ]),
            Line::from(vec![
                Span::styled("Vendor: ", label_style()),
                Span::raw(format!("0x{:04x}", device.vendor_id)),
            ]),
            Line::from(vec![
                Span::styled("Product: ", label_style()),
                Span::raw(format!("0x{:04x}", device.product_id)),
            ]),
            Line::from(vec![
                Span::styled("Transport: ", label_style()),
                Span::raw(&device.transport),
            ]),
            Line::from(vec![
                Span::styled("Model: ", label_style()),
                Span::raw(if device.is_edge() {
                    "DualSense Edge"
                } else {
                    "DualSense"
                }),
            ]),
            Line::from(vec![
                Span::styled("USB audio: ", label_style()),
                Span::raw(if device.supports_usb_audio() {
                    "available through CoreAudio"
                } else {
                    "not exposed over Bluetooth"
                }),
            ]),
        ];
        if let Some(address) = &device.mac_address {
            lines.push(Line::from(vec![
                Span::styled("MAC: ", label_style()),
                Span::raw(address),
            ]));
        }
        if let Some(firmware) = device.firmware {
            lines.push(Line::from(vec![
                Span::styled("Firmware: ", label_style()),
                Span::raw(firmware.firmware_label()),
                Span::raw("  "),
                Span::styled("Hardware: ", label_style()),
                Span::raw(firmware.hardware_label()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Feature level: ", label_style()),
                Span::raw(firmware.feature_label()),
            ]));
        }
        if let Some(error) = &device.diagnostics_error {
            lines.push(Line::from(vec![
                Span::styled("Diagnostics: ", Style::default().fg(Color::Yellow)),
                Span::raw(error),
            ]));
        }
        lines
    } else {
        vec![Line::from("Connect a DualSense over USB or Bluetooth.")]
    };

    let paragraph = Paragraph::new(text)
        .block(panel_block("Device", app.active_tab == Tab::Devices))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_lightbar(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let swatch_height = if area.height < 12 { 2 } else { 3 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(swatch_height),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let color = app.profile.lightbar;
    let swatch = Paragraph::new("")
        .style(Style::default().bg(to_color(color)))
        .block(panel_block(
            &format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b),
            false,
        ));
    frame.render_widget(swatch, chunks[0]);

    render_color_gauge(
        frame,
        chunks[1],
        "R",
        color.r,
        app.selected_lightbar_field == LightbarField::Red,
        Color::Red,
    );
    render_color_gauge(
        frame,
        chunks[2],
        "G",
        color.g,
        app.selected_lightbar_field == LightbarField::Green,
        Color::Green,
    );
    render_color_gauge(
        frame,
        chunks[3],
        "B",
        color.b,
        app.selected_lightbar_field == LightbarField::Blue,
        Color::Blue,
    );

    let body = Paragraph::new(Line::from(vec![
        Span::styled("Mode: ", label_style()),
        Span::raw("IOKit HID output report"),
    ]))
    .block(panel_block("Lightbar", false));
    frame.render_widget(body, chunks[4]);
}

fn render_color_gauge(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    value: u8,
    selected: bool,
    color: Color,
) {
    let gauge = Gauge::default()
        .block(panel_block(title, selected))
        .gauge_style(gauge_style(selected, color))
        .ratio(f64::from(value) / 255.0)
        .label(if selected {
            format!("> {value:3} <")
        } else {
            format!("{value:3}")
        });
    frame.render_widget(gauge, area);
}

fn render_haptics(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    if area.height <= 18 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(area);
        let settings = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);
        render_haptic_state(frame, settings[0], app);
        render_haptic_mode(frame, settings[1], app);
        render_haptic_gauge(
            frame,
            chunks[1],
            "Motor strength",
            app.profile.haptics.strength(),
            app.selected_haptic_field == HapticField::Strength,
        );
        render_audio_reactive_state(frame, chunks[2], app);
        render_audio_reactive_controls(frame, chunks[3], app);
        render_haptic_demos(frame, chunks[4], app);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Min(0),
        ])
        .split(area);

    render_haptic_state(frame, chunks[0], app);
    render_haptic_mode(frame, chunks[1], app);
    render_haptic_gauge(
        frame,
        chunks[2],
        "Motor strength",
        app.profile.haptics.strength(),
        app.selected_haptic_field == HapticField::Strength,
    );

    render_audio_reactive_state(frame, chunks[3], app);
    render_audio_reactive_controls(frame, chunks[4], app);
    render_audio_reactive_meter(frame, chunks[5], app);
    render_haptic_demos(frame, chunks[6], app);

    let demo = app.selected_haptic_demo;
    let body = Paragraph::new(Line::from(vec![
        Span::styled("Expected: ", label_style()),
        Span::raw(demo.expected_effect()),
        Span::raw("  "),
        Span::styled("Demo: ", label_style()),
        Span::raw(demo.label()),
    ]))
    .block(panel_block("Output", false));
    frame.render_widget(body, chunks[7]);
}

fn render_haptic_state(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let selected = app.selected_haptic_field == HapticField::State;
    let toggle = if app.profile.haptics.enabled {
        "enabled"
    } else {
        "disabled"
    };
    let paragraph = Paragraph::new(Line::from(Span::styled(
        toggle,
        selected_value_style(selected),
    )))
    .block(panel_block("State", selected));
    frame.render_widget(paragraph, area);
}

fn render_haptic_mode(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let selected = app.selected_haptic_field == HapticField::Mode;
    let mode = if app.profile.haptics.audio_haptics {
        "haptic-v2"
    } else {
        "legacy rumble"
    };
    let paragraph = Paragraph::new(Line::from(Span::styled(
        mode,
        selected_value_style(selected),
    )))
    .block(panel_block("Protocol", selected));
    frame.render_widget(paragraph, area);
}

fn render_audio_reactive_state(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let selected = app.selected_haptic_field == HapticField::ReactiveState;
    let state = app.audio_reactive.state();
    let body = Paragraph::new(Line::from(vec![
        Span::styled("System audio: ", label_style()),
        Span::styled(state.label(), selected_value_style(selected)),
        Span::raw("  "),
        Span::raw(if state.is_running() {
            "Space to stop"
        } else {
            "Space to start"
        }),
    ]))
    .block(panel_block("Audio reactive", selected));
    frame.render_widget(body, area);
}

fn render_audio_reactive_controls(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let reactive = &app.profile.haptics.audio_reactive;
    render_percent_gauge(
        frame,
        chunks[0],
        "Sensitivity",
        reactive.sensitivity_percent,
        250,
        app.selected_haptic_field == HapticField::ReactiveSensitivity,
    );
    render_percent_gauge(
        frame,
        chunks[1],
        "Noise gate",
        reactive.threshold_percent,
        90,
        app.selected_haptic_field == HapticField::ReactiveThreshold,
    );
}

fn render_audio_reactive_meter(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let meter = app.audio_reactive.meter();
    render_haptic_gauge(
        frame,
        chunks[0],
        "Bass input",
        scale_audio_level(meter.low),
        false,
    );
    render_haptic_gauge(
        frame,
        chunks[1],
        "Detail input",
        scale_audio_level(meter.high),
        false,
    );
}

fn render_percent_gauge(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    value: u8,
    maximum: u8,
    selected: bool,
) {
    let gauge = Gauge::default()
        .block(panel_block(title, selected))
        .gauge_style(gauge_style(selected, Color::Cyan))
        .ratio(f64::from(value) / f64::from(maximum))
        .label(if selected {
            format!("> {value}% <")
        } else {
            format!("{value}%")
        });
    frame.render_widget(gauge, area);
}

fn scale_audio_level(level: u16) -> u8 {
    ((u32::from(level) * u32::from(u8::MAX)) / u32::from(u16::MAX)) as u8
}

fn render_haptic_gauge(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    value: u8,
    selected: bool,
) {
    let gauge = Gauge::default()
        .block(panel_block(title, selected))
        .gauge_style(gauge_style(selected, Color::Magenta))
        .ratio(f64::from(value) / 255.0)
        .label(if selected {
            format!("> {value:3} <")
        } else {
            format!("{value:3}")
        });
    frame.render_widget(gauge, area);
}

fn render_mapping(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);
    let view = app.mapping_view;
    let heading = match view {
        MappingView::ControllerProfile => "Logical controller profile; save with s",
        MappingView::KeyboardOutput => app.mapping_status.as_str(),
        MappingView::MouseOutput => app.mouse_mapping_status.as_str(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("View: ", label_style()),
            Span::styled(
                view.label(),
                selected_value_style(app.active_tab == Tab::Mapping),
            ),
            Span::raw("  "),
            Span::raw(heading),
        ]))
        .block(panel_block("Mapping", app.active_tab == Tab::Mapping)),
        chunks[0],
    );

    match view {
        MappingView::ControllerProfile => render_controller_mapping_table(frame, chunks[1], app),
        MappingView::KeyboardOutput => render_keyboard_mapping_table(frame, chunks[1], app),
        MappingView::MouseOutput => render_mouse_mapping(frame, chunks[1], app),
    }
}

fn render_mouse_mapping(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let (settings_area, controls_area) = if area.height >= 11 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(7), Constraint::Length(3)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };
    let mouse = &app.profile.mouse_mapping;
    let settings = [
        (
            MouseField::Enabled,
            "Output",
            if mouse.enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
        ),
        (
            MouseField::PointerSpeed,
            "Pointer speed",
            format!("{} px/tick", mouse.pointer_speed),
        ),
        (
            MouseField::Deadzone,
            "Dead zone",
            format!("{}%", mouse.deadzone_percent),
        ),
        (
            MouseField::ScrollSpeed,
            "Scroll speed",
            format!("{} px/tick", mouse.scroll_speed),
        ),
    ];
    let selected_index = settings
        .iter()
        .position(|(field, _, _)| *field == app.selected_mouse_field)
        .unwrap_or(0);
    let visible_range = visible_list_range(
        settings.len(),
        selected_index,
        usize::from(settings_area.height.saturating_sub(3)),
    );
    let rows = settings[visible_range].iter().map(|(field, label, value)| {
        let style = if *field == app.selected_mouse_field {
            selected_row_style()
        } else {
            Style::default()
        };
        Row::new(vec![*label, value.as_str()]).style(style)
    });
    let table = Table::new(rows, [Constraint::Length(18), Constraint::Min(18)])
        .header(
            Row::new(vec!["Setting", "Value"]).style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(panel_block("Mouse output", app.active_tab == Tab::Mapping));
    frame.render_widget(table, settings_area);

    if let Some(controls_area) = controls_area {
        let controls = Paragraph::new(vec![
            Line::from("Left stick: pointer  |  Right stick Y: scroll"),
            Line::from("Cross: left click  |  Circle: right click  |  Square: middle click"),
        ])
        .block(panel_block("Mouse controls", false))
        .wrap(Wrap { trim: true });
        frame.render_widget(controls, controls_area);
    }
}

fn render_controller_mapping_table(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let mappings = &app.profile.mappings;
    let visible_range = visible_list_range(
        mappings.len(),
        app.selected_mapping,
        usize::from(area.height.saturating_sub(3)),
    );
    let range_start = visible_range.start;
    let rows = mappings[visible_range]
        .iter()
        .enumerate()
        .map(|(visible_index, mapping)| {
            let index = range_start + visible_index;
            let style = if index == app.selected_mapping {
                selected_row_style()
            } else {
                Style::default()
            };
            Row::new(vec![mapping.from.label(), "->", mapping.to.label()]).style(style)
        });

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(4),
            Constraint::Length(18),
        ],
    )
    .header(
        Row::new(vec!["Source", "", "Target"]).style(
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(panel_block(
        "Controller profile",
        app.active_tab == Tab::Mapping,
    ));
    frame.render_widget(table, area);
}

fn render_keyboard_mapping_table(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let bindings = &app.profile.keyboard_mapping.bindings;
    let visible_range = visible_list_range(
        bindings.len(),
        app.selected_keyboard_mapping,
        usize::from(area.height.saturating_sub(3)),
    );
    let range_start = visible_range.start;
    let rows = bindings[visible_range]
        .iter()
        .enumerate()
        .map(|(visible_index, binding)| {
            let index = range_start + visible_index;
            let style = if index == app.selected_keyboard_mapping {
                selected_row_style()
            } else {
                Style::default()
            };
            Row::new(vec![binding.from.label(), "->", binding.to.label()]).style(style)
        });
    let state = if app.profile.keyboard_mapping.enabled {
        "enabled"
    } else {
        "disabled"
    };
    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(4),
            Constraint::Length(18),
        ],
    )
    .header(
        Row::new(vec!["Source", "", "Key"]).style(
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(panel_block(
        &format!("Keyboard output ({state})"),
        app.active_tab == Tab::Mapping,
    ));
    frame.render_widget(table, area);
}

fn render_adaptive_triggers(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Min(0),
        ])
        .split(area);

    let target_selected = app.selected_trigger_field == TriggerField::Target;
    let target_style = selected_value_style(target_selected);
    let target_line = Line::from(vec![
        Span::styled("Target: ", label_style()),
        Span::styled(app.profile.adaptive_triggers.target.label(), target_style),
    ]);
    frame.render_widget(
        Paragraph::new(target_line).block(panel_block("Target", target_selected)),
        chunks[0],
    );

    let mode_selected = app.selected_trigger_field == TriggerField::Mode;
    let mode_line = Line::from(vec![
        Span::styled("Mode: ", label_style()),
        Span::styled(
            app.profile.adaptive_triggers.mode.label(),
            selected_value_style(mode_selected),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(mode_line).block(panel_block("Effect mode", mode_selected)),
        chunks[1],
    );

    let intensity = app.profile.adaptive_triggers.intensity;
    let intensity_selected = app.selected_trigger_field == TriggerField::Intensity;
    let gauge = Gauge::default()
        .block(panel_block("Intensity", intensity_selected))
        .gauge_style(gauge_style(intensity_selected, Color::Yellow))
        .ratio(f64::from(intensity) / 255.0)
        .label(if intensity_selected {
            format!("> {intensity:3} <")
        } else {
            format!("{intensity:3}")
        });
    frame.render_widget(gauge, chunks[2]);

    render_trigger_custom_controls(frame, chunks[3], app);

    render_trigger_presets(frame, chunks[4], app);

    let trigger = &app.profile.adaptive_triggers;
    let expected_text = match trigger.mode {
        crate::model::AdaptiveTriggerMode::Preset => trigger.preset.expected_effect().to_string(),
        crate::model::AdaptiveTriggerMode::Resistance => format!(
            "constant resistance from position {} to {}",
            trigger.start_position, trigger.end_position
        ),
        crate::model::AdaptiveTriggerMode::Vibration => format!(
            "persistent vibration from position {} at frequency {}",
            trigger.start_position, trigger.frequency
        ),
    };
    let expected = Paragraph::new(Line::from(vec![
        Span::styled("Expected: ", label_style()),
        Span::raw(expected_text),
    ]))
    .block(panel_block("Effect", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(expected, chunks[5]);
}

fn render_trigger_custom_controls(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);
    let trigger = &app.profile.adaptive_triggers;
    render_trigger_custom_value(
        frame,
        chunks[0],
        "Start",
        trigger.start_position,
        app.selected_trigger_field == TriggerField::StartPosition,
    );
    render_trigger_custom_value(
        frame,
        chunks[1],
        "End",
        trigger.end_position,
        app.selected_trigger_field == TriggerField::EndPosition,
    );
    render_trigger_custom_value(
        frame,
        chunks[2],
        "Frequency",
        trigger.frequency,
        app.selected_trigger_field == TriggerField::Frequency,
    );
}

fn render_trigger_custom_value(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    value: u8,
    selected: bool,
) {
    let paragraph = Paragraph::new(Line::from(Span::styled(
        value.to_string(),
        selected_value_style(selected),
    )))
    .alignment(Alignment::Center)
    .block(panel_block(title, selected));
    frame.render_widget(paragraph, area);
}

fn render_trigger_presets(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let presets = AdaptiveTriggerPreset::ALL;
    let selected_index = presets
        .iter()
        .position(|preset| *preset == app.profile.adaptive_triggers.preset)
        .unwrap_or(0);
    let visible_rows = usize::from(area.height.saturating_sub(3));
    let visible_range = visible_list_range(presets.len(), selected_index, visible_rows);
    let rows = presets[visible_range].iter().copied().map(|preset| {
        let selected = app.selected_trigger_field == TriggerField::Presets
            && preset == app.profile.adaptive_triggers.preset;
        let style = if selected {
            selected_row_style()
        } else {
            Style::default()
        };
        Row::new(vec![preset.label(), preset.expected_effect()]).style(style)
    });

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Min(20)])
        .header(
            Row::new(vec!["Preset", "Feel"]).style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(panel_block(
            "Presets",
            app.selected_trigger_field == TriggerField::Presets,
        ));
    frame.render_widget(table, area);
}

fn render_haptic_demos(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let demos = HapticDemo::ALL;
    let visible_rows = usize::from(area.height.saturating_sub(3));
    let selected_index = demos
        .iter()
        .position(|demo| *demo == app.selected_haptic_demo)
        .unwrap_or(0);
    let visible_range = visible_list_range(demos.len(), selected_index, visible_rows);
    let rows = demos[visible_range].iter().copied().map(|demo| {
        let selected =
            app.selected_haptic_field == HapticField::Demo && demo == app.selected_haptic_demo;
        let style = if selected {
            selected_row_style()
        } else {
            Style::default()
        };
        Row::new(vec![demo.label(), demo.expected_effect()]).style(style)
    });

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Min(20)])
        .header(
            Row::new(vec!["Demo", "Effect"]).style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(panel_block(
            "Demos",
            app.selected_haptic_field == HapticField::Demo,
        ));
    frame.render_widget(table, area);
}

fn visible_list_range(
    item_count: usize,
    selected_index: usize,
    visible_rows: usize,
) -> std::ops::Range<usize> {
    let visible_rows = visible_rows.min(item_count);
    if visible_rows == 0 {
        return 0..0;
    }

    let selected_index = selected_index.min(item_count - 1);
    let start = selected_index
        .saturating_sub(visible_rows / 2)
        .min(item_count - visible_rows);
    start..start + visible_rows
}

fn render_gamepad(frame: &mut Frame<'_>, area: Rect, state: &GamepadState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),
            Constraint::Min(24),
            Constraint::Length(22),
        ])
        .split(area);

    let left = vec![
        button_line(state, &[Button::L1, Button::L2, Button::Create]),
        Line::from(""),
        button_line(state, &[Button::DpadUp]),
        button_line(state, &[Button::DpadLeft, Button::DpadRight]),
        button_line(state, &[Button::DpadDown]),
        Line::from(""),
    ]
    .into_iter()
    .chain(stick_lines("L", state.left_stick))
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(left).block(panel_block("Left", false)),
        columns[0],
    );

    let center = vec![
        button_line(state, &[Button::Touchpad]),
        Line::from(touch_summary(state)),
        Line::from(""),
        button_line(state, &[Button::Ps, Button::Mute]),
        button_line(state, &[Button::Fn1, Button::Fn2]),
        button_line(state, &[Button::LeftPaddle, Button::RightPaddle]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Pressed: ", label_style()),
            Span::raw(pressed_summary(state)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(center).block(panel_block("Center", false)),
        columns[1],
    );

    let right = vec![
        button_line(state, &[Button::Options, Button::R2, Button::R1]),
        Line::from(""),
        button_line(state, &[Button::Triangle]),
        button_line(state, &[Button::Square, Button::Circle]),
        button_line(state, &[Button::Cross]),
        Line::from(""),
    ]
    .into_iter()
    .chain(stick_lines("R", state.right_stick))
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(right).block(panel_block("Right", false)),
        columns[2],
    );
}

fn render_input_triggers(frame: &mut Frame<'_>, area: Rect, state: &GamepadState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    render_trigger(frame, columns[0], "L2 analog", state.left_trigger);
    render_trigger(frame, columns[1], "R2 analog", state.right_trigger);
}

fn render_trigger(frame: &mut Frame<'_>, area: Rect, title: &'static str, value: u8) {
    let gauge = Gauge::default()
        .block(panel_block(title, false))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black))
        .ratio(f64::from(value) / 255.0)
        .label(format!("{value:3}"));
    frame.render_widget(gauge, area);
}

fn button_line(state: &GamepadState, buttons: &[Button]) -> Line<'static> {
    let mut spans = Vec::with_capacity(buttons.len() * 2);
    for (index, button) in buttons.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(button_span(state, *button));
    }
    Line::from(spans)
}

fn button_span(state: &GamepadState, button: Button) -> Span<'static> {
    let style = if state.is_pressed(button) {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    Span::styled(format!(" {:^3} ", button.short_label()), style)
}

fn stick_lines(label: &'static str, stick: StickState) -> Vec<Line<'static>> {
    const WIDTH: usize = 11;
    const HEIGHT: usize = 7;
    let pointer_x = ((usize::from(stick.x) * (WIDTH - 1)) / 255).min(WIDTH - 1);
    let pointer_y = ((usize::from(stick.y) * (HEIGHT - 1)) / 255).min(HEIGHT - 1);
    let center_x = WIDTH / 2;
    let center_y = HEIGHT / 2;

    let mut lines = Vec::with_capacity(HEIGHT + 1);
    for y in 0..HEIGHT {
        let mut spans = Vec::with_capacity(WIDTH);
        for x in 0..WIDTH {
            let (text, style) = if x == pointer_x && y == pointer_y {
                (
                    label,
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else if x == center_x && y == center_y {
                ("+", Style::default().fg(Color::DarkGray))
            } else {
                (".", Style::default().fg(Color::DarkGray))
            };
            spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(format!(
        "x {:+.2} y {:+.2}",
        stick.normalized_x(),
        stick.normalized_y()
    )));
    lines
}

fn pressed_summary(state: &GamepadState) -> String {
    if state.buttons.is_empty() {
        return "none".to_string();
    }

    state
        .buttons
        .iter()
        .map(|button| button.short_label())
        .collect::<Vec<_>>()
        .join(" ")
}

fn touch_summary(state: &GamepadState) -> String {
    let active = state
        .touch_points
        .iter()
        .flatten()
        .map(|point| format!("{}:{},{}", point.contact_id, point.x, point.y))
        .collect::<Vec<_>>();
    if active.is_empty() {
        "Touch: inactive".to_string()
    } else {
        format!("Touch: {}", active.join(" "))
    }
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let dirty = if app.dirty { "modified" } else { "saved" };
    let device = app
        .devices
        .get(app.selected_device)
        .map(|device| device.name.as_str())
        .unwrap_or("none");

    let text = if area.height <= 4 {
        vec![
            Line::from(vec![
                Span::styled("Panel: ", label_style()),
                Span::raw(app.active_tab.title()),
            ]),
            Line::from(vec![
                Span::styled("Status: ", label_style()),
                Span::raw(&app.status),
            ]),
        ]
    } else if area.height <= 10 {
        vec![
            Line::from(vec![
                Span::styled("Panel: ", label_style()),
                Span::raw(app.active_tab.title()),
            ]),
            Line::from(vec![
                Span::styled("Status: ", label_style()),
                Span::raw(&app.status),
            ]),
            Line::from(vec![
                Span::styled("Device: ", label_style()),
                Span::raw(device),
            ]),
            Line::from(vec![
                Span::styled("Profile: ", label_style()),
                Span::raw(dirty),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Panel: ", label_style()),
                Span::raw(app.active_tab.title()),
            ]),
            Line::from(vec![
                Span::styled("Status: ", label_style()),
                Span::raw(&app.status),
            ]),
            Line::from(vec![
                Span::styled("Device: ", label_style()),
                Span::raw(device),
            ]),
            Line::from(vec![
                Span::styled("Profile: ", label_style()),
                Span::raw(dirty),
            ]),
            Line::from(vec![
                Span::styled("Input: ", label_style()),
                Span::raw(app.input_status.as_str()),
            ]),
            Line::from(vec![
                Span::styled("Path: ", label_style()),
                Span::raw(app.profile_path.as_str()),
            ]),
        ]
    };
    let paragraph = Paragraph::new(text)
        .block(panel_block("Status", false))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn footer_control_lines(app: &ConfiguratorApp) -> Vec<Line<'static>> {
    match app.active_tab {
        Tab::Devices => vec![
            control_group_line(
                "Devices",
                &[("Up/Down", "select device"), ("r", "refresh / reapply")],
            ),
            Line::from(""),
        ],
        Tab::Input => vec![
            control_group_line(
                "Input",
                &[("Gamepad", "move sticks / press buttons / touchpad")],
            ),
            control_group_line("Actions", &[("r", "refresh / reapply profiles")]),
        ],
        Tab::Sensors => vec![
            control_group_line(
                "Sensors",
                &[("Touchpad", "two contacts"), ("Motion", "gyro + accel")],
            ),
            control_group_line("Actions", &[("r", "refresh / reapply profiles")]),
        ],
        Tab::Lightbar => vec![
            control_group_line(
                "Lightbar",
                &[
                    ("Up/Down", "select R/G/B"),
                    ("Left/Right", "change by 5"),
                    ("+/-", "change by 1"),
                ],
            ),
            control_group_line("Actions", &[("a/Enter", "apply color")]),
        ],
        Tab::Haptics => vec![
            control_group_line(
                "Haptics",
                &[
                    ("Up/Down", "select field"),
                    ("Left/Right", "adjust/toggle"),
                    ("Space", "toggle / start audio reactive / play"),
                ],
            ),
            control_group_line(
                "Actions",
                &[
                    ("d", "play demo"),
                    ("a/Enter", "play demo / toggle audio reactive"),
                    ("p", "pulse"),
                ],
            ),
        ],
        Tab::Triggers => vec![
            control_group_line(
                "Triggers",
                &[
                    (
                        "Up/Down",
                        "Target > Mode > Intensity > Start > End > Frequency > Presets",
                    ),
                    ("Left/Right", "change field"),
                    ("+/-", "fine tune"),
                ],
            ),
            control_group_line("Actions", &[("a/Enter", "apply preset"), ("x", "reset")]),
        ],
        Tab::System => vec![
            control_group_line(
                "System",
                &[
                    ("Up/Down", "select field"),
                    ("Left/Right", "change value"),
                    ("Space", "toggle mic mute"),
                ],
            ),
            control_group_line("Actions", &[("a/Enter", "apply HID controls")]),
        ],
        Tab::Mapping => match app.mapping_view {
            MappingView::ControllerProfile => vec![
                control_group_line(
                    "Controller",
                    &[
                        ("Up/Down", "select source"),
                        ("Left/Right", "change target"),
                        ("Space", "reset row"),
                    ],
                ),
                control_group_line(
                    "Views",
                    &[
                        ("m", "keyboard output"),
                        ("o", "Accessibility settings"),
                        ("s", "save profile"),
                    ],
                ),
            ],
            MappingView::KeyboardOutput => vec![
                control_group_line(
                    "Keyboard",
                    &[
                        ("Up/Down", "select source"),
                        ("Left/Right", "change key"),
                        ("Space", "disable row"),
                    ],
                ),
                control_group_line(
                    "Actions",
                    &[
                        ("k or a/Enter", "toggle output"),
                        ("o", "Accessibility settings"),
                        ("m", "mouse output"),
                    ],
                ),
            ],
            MappingView::MouseOutput => vec![
                control_group_line(
                    "Mouse",
                    &[
                        ("Left stick", "pointer"),
                        ("Right stick Y", "scroll"),
                        ("Cross/Circle/Square", "left/right/middle"),
                    ],
                ),
                control_group_line(
                    "Settings",
                    &[
                        ("Up/Down", "select setting"),
                        ("Left/Right", "adjust value"),
                        ("Space", "toggle output"),
                    ],
                ),
                control_group_line(
                    "Actions",
                    &[
                        ("k or a/Enter", "toggle output"),
                        ("o", "Accessibility settings"),
                        ("m", "controller profile"),
                    ],
                ),
            ],
        },
    }
}

fn control_group_line(
    title: &'static str,
    controls: &[(&'static str, &'static str)],
) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(Span::styled(format!("{title}: "), label_style()));
    for (index, (key, action)) in controls.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(*key, key_style()));
        spans.push(Span::raw(format!(" {action}")));
    }
    Line::from(spans)
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let mut lines = footer_control_lines(app);
    lines.push(control_group_line(
        "Global",
        &[
            ("Tab/Shift+Tab", "panel"),
            ("1-8", "jump"),
            ("s", "save"),
            ("q/Esc", "quit"),
        ],
    ));
    lines.push(control_group_line(
        "Layout",
        &[
            ("[/]", "devices"),
            ("{/}", "status"),
            ("</>", "controls"),
            ("0", "reset"),
        ],
    ));
    let paragraph = Paragraph::new(lines)
        .block(panel_block("Controls", false))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn panel_block(title: &str, focused: bool) -> Block<'static> {
    let title = if focused {
        format!("> {title} ")
    } else {
        format!(" {title} ")
    };
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if focused {
            focus_style()
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .title_style(if focused {
            focus_style()
        } else {
            label_style()
        })
}

fn gauge_style(selected: bool, color: Color) -> Style {
    if selected {
        Style::default()
            .fg(Color::Cyan)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color).bg(Color::Black)
    }
}

fn selected_value_style(selected: bool) -> Style {
    if selected {
        selected_row_style()
    } else {
        Style::default()
    }
}

fn selected_row_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn focus_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn label_style() -> Style {
    Style::default()
        .fg(Color::Gray)
        .add_modifier(Modifier::BOLD)
}

fn key_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn to_color(color: Rgb) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use ratatui::layout::{Direction, Layout, Rect};

    use super::{system_constraints, visible_list_range, SYSTEM_SECTION_COUNT};

    #[test]
    fn visible_list_range_keeps_last_selected_item_visible() {
        assert_eq!(visible_list_range(8, 7, 1), 7..8);
        assert_eq!(visible_list_range(8, 7, 3), 5..8);
    }

    #[test]
    fn visible_list_range_centers_middle_selection_when_possible() {
        assert_eq!(visible_list_range(8, 4, 3), 3..6);
        assert_eq!(visible_list_range(8, 0, 3), 0..3);
    }

    #[test]
    fn system_layout_provides_every_rendered_section() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(system_constraints())
            .split(Rect::new(0, 0, 80, 24));

        assert_eq!(chunks.len(), SYSTEM_SECTION_COUNT);
    }
}
