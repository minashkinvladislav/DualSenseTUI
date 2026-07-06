use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

use crate::{
    app::{ConfiguratorApp, Tab},
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
            if tab == app.active_tab {
                Line::from(format!("> {}", tab.title()))
            } else {
                Line::from(tab.title())
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
        Tab::Lightbar => render_lightbar(frame, area, app),
        Tab::Haptics => render_haptics(frame, area, app),
        Tab::Triggers => render_adaptive_triggers(frame, area, app),
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
        Line::from(vec![
            Span::styled("Input: ", label_style()),
            Span::raw(&app.input_status),
            Span::raw("  "),
            Span::styled("Battery: ", label_style()),
            Span::raw(battery),
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

fn render_device_details(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let text = if let Some(device) = app.devices.get(app.selected_device) {
        vec![
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
        ]
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
        app.selected_color_channel == 0,
        Color::Red,
    );
    render_color_gauge(
        frame,
        chunks[2],
        "G",
        color.g,
        app.selected_color_channel == 1,
        Color::Green,
    );
    render_color_gauge(
        frame,
        chunks[3],
        "B",
        color.b,
        app.selected_color_channel == 2,
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
    if area.height <= 15 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(area);
        let gauges = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        render_haptic_gauge(
            frame,
            gauges[0],
            "Heavy",
            app.profile.haptics.left_strength,
            app.selected_haptic_field == 0,
        );
        render_haptic_gauge(
            frame,
            gauges[1],
            "Sharp",
            app.profile.haptics.right_strength,
            app.selected_haptic_field == 1,
        );
        render_haptic_mode(frame, chunks[1], app);
        render_haptic_demos(frame, chunks[2], app);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Min(0),
        ])
        .split(area);

    render_haptic_gauge(
        frame,
        chunks[0],
        "Heavy",
        app.profile.haptics.left_strength,
        app.selected_haptic_field == 0,
    );
    render_haptic_gauge(
        frame,
        chunks[1],
        "Sharp",
        app.profile.haptics.right_strength,
        app.selected_haptic_field == 1,
    );

    render_haptic_mode(frame, chunks[2], app);
    render_haptic_demos(frame, chunks[3], app);

    let demo = app.selected_haptic_demo;
    let body = Paragraph::new(Line::from(vec![
        Span::styled("Expected: ", label_style()),
        Span::raw(demo.expected_effect()),
        Span::raw("  "),
        Span::styled("Demo: ", label_style()),
        Span::raw(demo.label()),
    ]))
    .block(panel_block("Output", false));
    frame.render_widget(body, chunks[4]);
}

fn render_haptic_mode(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let selected = app.selected_haptic_field == 2;
    let selected_style = selected_value_style(selected);
    let mode = if app.profile.haptics.audio_haptics {
        "audio-haptics"
    } else {
        "compat rumble"
    };
    let toggle = if app.profile.haptics.enabled {
        "enabled"
    } else {
        "disabled"
    };
    let line = Line::from(vec![
        Span::styled("State: ", label_style()),
        Span::raw(toggle),
        Span::raw("  "),
        Span::styled("Mode: ", label_style()),
        Span::styled(mode, selected_style),
    ]);
    let paragraph = Paragraph::new(line).block(panel_block("Haptics", selected));
    frame.render_widget(paragraph, area);
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
    let rows = app
        .profile
        .mappings
        .iter()
        .enumerate()
        .map(|(index, mapping)| {
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
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["Source", "", "Target"]).style(
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(panel_block("Mapping", app.active_tab == Tab::Mapping));
    frame.render_widget(table, area);
}

fn render_adaptive_triggers(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let target_selected = app.selected_trigger_field == 0;
    let target_style = selected_value_style(target_selected);
    let target_line = Line::from(vec![
        Span::styled("Target: ", label_style()),
        Span::styled(app.profile.adaptive_triggers.target.label(), target_style),
        Span::raw("  "),
        Span::styled("Preset: ", label_style()),
        Span::styled(
            app.profile.adaptive_triggers.preset.label(),
            selected_value_style(app.selected_trigger_field == 2),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(target_line).block(panel_block("Target", target_selected)),
        chunks[0],
    );

    let intensity = app.profile.adaptive_triggers.intensity;
    let intensity_selected = app.selected_trigger_field == 1;
    let gauge = Gauge::default()
        .block(panel_block("Intensity", intensity_selected))
        .gauge_style(gauge_style(intensity_selected, Color::Yellow))
        .ratio(f64::from(intensity) / 255.0)
        .label(if intensity_selected {
            format!("> {intensity:3} <")
        } else {
            format!("{intensity:3}")
        });
    frame.render_widget(gauge, chunks[1]);

    render_trigger_presets(frame, chunks[2], app);

    let preset = app.profile.adaptive_triggers.preset;
    let expected = Paragraph::new(Line::from(vec![
        Span::styled("Expected: ", label_style()),
        Span::raw(preset.expected_effect()),
    ]))
    .block(panel_block("Effect", false));
    frame.render_widget(expected, chunks[3]);

    let body = Paragraph::new(
        "Apply sends a persistent adaptive trigger report. Reset sets both triggers to Off.",
    )
    .block(panel_block("Output", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(body, chunks[4]);
}

fn render_trigger_presets(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let rows = AdaptiveTriggerPreset::ALL.into_iter().map(|preset| {
        let selected =
            app.selected_trigger_field == 2 && preset == app.profile.adaptive_triggers.preset;
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
        .block(panel_block("Presets", app.selected_trigger_field == 2));
    frame.render_widget(table, area);
}

fn render_haptic_demos(frame: &mut Frame<'_>, area: Rect, app: &ConfiguratorApp) {
    let demos = HapticDemo::ALL;
    let visible_rows = usize::from(area.height.saturating_sub(3));
    let selected_index = demos
        .iter()
        .position(|demo| *demo == app.selected_haptic_demo)
        .unwrap_or(0);
    let start = if visible_rows == 0 || visible_rows >= demos.len() {
        0
    } else {
        selected_index
            .saturating_sub(visible_rows / 2)
            .min(demos.len() - visible_rows)
    };
    let end = if visible_rows == 0 {
        0
    } else {
        (start + visible_rows).min(demos.len())
    };
    let rows = demos[start..end].iter().copied().map(|demo| {
        let selected = app.selected_haptic_field == 3 && demo == app.selected_haptic_demo;
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
        .block(panel_block("Demos", app.selected_haptic_field == 3));
    frame.render_widget(table, area);
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
        Line::from(""),
        button_line(state, &[Button::Ps, Button::Mute]),
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
            control_group_line("Devices", &[("Up/Down", "select device"), ("r", "refresh")]),
            Line::from(""),
        ],
        Tab::Input => vec![
            control_group_line("Input", &[("Gamepad", "move sticks / press buttons")]),
            control_group_line("Actions", &[("r", "refresh devices")]),
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
                    ("Left/Right", "adjust value/demo"),
                    ("Space", "toggle"),
                ],
            ),
            control_group_line("Actions", &[("d or a", "play demo"), ("p", "pulse")]),
        ],
        Tab::Triggers => vec![
            control_group_line(
                "Triggers",
                &[
                    ("Up/Down", "Target > Intensity > Presets"),
                    ("Left/Right", "change field"),
                    ("+/-", "fine tune"),
                ],
            ),
            control_group_line("Actions", &[("a/Enter", "apply preset"), ("x", "reset")]),
        ],
        Tab::Mapping => vec![
            control_group_line(
                "Mapping",
                &[
                    ("Up/Down", "select source"),
                    ("Left/Right", "change target"),
                    ("Space", "reset row"),
                ],
            ),
            Line::from(""),
        ],
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
            ("1-6", "jump"),
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
