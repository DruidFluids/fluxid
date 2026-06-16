//! Secondary windows: Tools, Alerts, Game Mode, Help, Utilities, the Window
//! Picker, and the widget right-click context menu.

use fluid_core::settings::{AppSettings, RemoteDevice, SnapPosition, WarnMetric};
use iced::widget::{button, checkbox, column, container, mouse_area, pick_list, row, scrollable, slider, text, text_editor, text_input, toggler, Space};
use iced::{window, Border, Color, Element, Length};
use crate::style::Palette;
use crate::Message;

// ── Shared chrome ──────────────────────────────────────────────────────────

fn caption<'a>(title: &str, win_id: window::Id, p: Palette) -> Element<'a, Message> {
    let close = crate::style::with_tip(button(
        text("\u{2715}").size(15).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).padding([2, 8]).style(|_, _| button::Style { background: None, ..Default::default() })
        .on_press(Message::ClosePopup(win_id)), "Close", p);

    mouse_area(
        container(row![
            text(title.to_string()).size(11)
                .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
            Space::with_width(Length::Fill),
            close,
        ].align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .padding(iced::Padding { top: 3.0, right: 4.0, bottom: 1.0, left: 6.0 })
    ).on_press(Message::DragWindow(win_id)).into()
}

fn section_header<'a>(label: &str, p: Palette) -> Element<'a, Message> {
    text(label.to_string()).size(10)
        .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
        .into()
}

fn label<'a>(t: &str, p: Palette) -> Element<'a, Message> {
    text(t.to_string()).size(11)
        .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
        .into()
}

fn toggle_row<'a>(label_text: &str, on: bool, msg: fn(bool) -> Message, p: Palette) -> Element<'a, Message> {
    row![
        toggler(on).size(14).on_toggle(msg).style(crate::style::toggler_style(p)),
        text(label_text.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
    ].spacing(6).align_y(iced::Alignment::Center).into()
}

fn pill<'a>(label_text: String, active: bool, msg: Message, p: Palette) -> Element<'a, Message> {
    button(text(label_text).size(11).font(iced::Font::with_name("Segoe UI Symbol")))
        .padding([4, 12])
        .style(move |_: &iced::Theme, _: button::Status| button::Style {
            background: Some(iced::Background::Color(if active { p.accent } else { p.tile })),
            text_color: if active { Color::WHITE } else { p.text },
            border: Border { radius: 4.0.into(), ..Border::default() },
            ..Default::default()
        })
        .on_press(msg).into()
}

fn primary_btn<'a>(label_text: &str, msg: Message, p: Palette) -> Element<'a, Message> {
    button(text(label_text.to_string()).size(11)
        .style(move |_| iced::widget::text::Style { color: Some(Color::WHITE) }))
        .padding([6, 20])
        .style(move |_, _| button::Style {
            background: Some(iced::Background::Color(p.accent)),
            border: Border { radius: 6.0.into(), ..Border::default() },
            ..Default::default()
        })
        .on_press(msg).into()
}

fn shell<'a>(title: &str, win_id: window::Id, p: Palette, body: Element<'a, Message>) -> Element<'a, Message> {
    // Match the Settings window's "Soft Premium" frame: a slightly darkened
    // window backdrop, a soft accent-tinted hairline border, and a large radius.
    let window_bg = Color { r: p.bg.r * 0.88, g: p.bg.g * 0.88, b: p.bg.b * 0.88, ..p.bg };
    let accent_border = crate::style::lerp(window_bg, p.accent, 0.45);
    // Caption is flush in the top-left corner; only the body is inset.
    container(column![
        caption(title, win_id, p),
        container(body).width(Length::Fill).height(Length::Fill)
            .padding(iced::Padding { top: 4.0, right: 16.0, bottom: 12.0, left: 16.0 }),
    ])
        .width(Length::Fill).height(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(window_bg)),
            border: Border { radius: crate::style::win_radius(16.0).into(), width: 1.5, color: accent_border },
            ..Default::default()
        })
        .into()
}

// ── Widget right-click context menu (C# Window.ContextMenu) ──────────────────

pub const WIDGET_MENU_SIZE: iced::Size = iced::Size::new(150.0, 70.0);

pub fn widget_menu_view<'a>(p: Palette) -> Element<'a, Message> {
    let item = |label: &str, msg: Message| -> Element<'a, Message> {
        button(
            text(label.to_string()).size(12)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) })
        )
        .width(Length::Fill)
        .padding(iced::Padding { top: 5.0, right: 12.0, bottom: 5.0, left: 12.0 })
        .style(move |_, status| {
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: if hover { Some(iced::Background::Color(p.accent)) } else { None },
                text_color: if hover { Color::WHITE } else { p.text },
                border: Border { radius: 4.0.into(), ..Border::default() },
                ..Default::default()
            }
        })
        .on_press(msg).into()
    };
    let divider = container(Space::new(Length::Fill, 1))
        .padding(iced::Padding { top: 0.0, right: 6.0, bottom: 0.0, left: 6.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color { a: 0.3, ..p.muted })),
            ..Default::default()
        });

    container(column![
        crate::style::with_tip(item("Settings\u{2026}", Message::WidgetMenuSettings), "Open settings", p),
        divider,
        crate::style::with_tip(item("Exit", Message::WidgetMenuExit), "Quit fluxid", p),
    ].spacing(2))
    .width(Length::Fill).height(Length::Fill)
    .padding(4)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(p.bg)),
        border: Border { radius: 8.0.into(), width: 1.0, color: Color { a: 0.5, ..p.muted } },
        ..Default::default()
    })
    .into()
}

// A colour swatch preview + hex input, shared by the regular and popout alert
// editors so users can pick flash and gradient colours.
fn color_field<'a, F>(hex: &str, p: Palette, on_input: F) -> Element<'a, Message>
where
    F: Fn(String) -> Message + 'a,
{
    let c = crate::style::parse_hex(hex, p.muted);
    row![
        container(Space::new(16, 16)).style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(c)),
            border: Border { radius: 3.0.into(), width: 1.0, color: Color { a: 0.4, ..p.muted } },
            ..Default::default()
        }),
        Space::with_width(6),
        text_input("#AARRGGBB", hex).size(10).width(112)
            .font(iced::Font::with_name("Consolas"))
            .on_input(on_input)
            .style(crate::style::dark_input_style(p)),
    ].align_y(iced::Alignment::Center).into()
}

// ── Tile Alerts (Warnings) ───────────────────────────────────────────────────

pub const ALERTS_SIZE: iced::Size = iced::Size::new(460.0, 560.0);

fn warn_card<'a>(settings: &AppSettings, kind: &str, p: Palette) -> Element<'a, Message> {
    let w = settings.warn(kind).cloned().unwrap_or_default();
    let enabled = w.enabled;
    let k1 = kind.to_string();
    let k2 = kind.to_string();
    let k3 = kind.to_string();
    let k4 = kind.to_string();
    let k5 = kind.to_string();
    let k6 = kind.to_string();
    let display = format!("{} Tile", kind);

    let metrics = vec!["Temperature".to_string(), "Load".to_string()];
    let sel_metric = match w.metric {
        WarnMetric::Load => "Load".to_string(),
        _ => "Temperature".to_string(),
    };
    let unit_label = if matches!(w.metric, WarnMetric::Load) { " %" } else { " \u{00B0}C" };

    // Dim the whole config when the alert is off (faithful to the C# DataTrigger
    // opacity). iced 0.13 containers can't set opacity, so we fade the colours the
    // card's controls draw with by reducing their alpha — a real visual cue that
    // the section is inactive (the previous no-op container left it full-strength).
    let bp = if enabled {
        p
    } else {
        Palette {
            text: iced::Color { a: p.text.a * 0.4, ..p.text },
            muted: iced::Color { a: p.muted.a * 0.4, ..p.muted },
            accent: iced::Color { a: p.accent.a * 0.4, ..p.accent },
            ..p
        }
    };
    let grad_color_row: Element<'a, Message> = if w.gradient_mode {
        row![
            text("Gradient color".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(bp.muted) }),
            Space::with_width(Length::Fill),
            color_field(&w.gradient_color, bp, move |s| Message::SetWarnGradientColor(k6.clone(), s)),
        ].spacing(6).align_y(iced::Alignment::Center).into()
    } else {
        Space::with_height(0).into()
    };
    let body = column![
        // Threshold
        row![
            label("Threshold", bp), Space::with_width(Length::Fill),
            text_input("", &format!("{}", w.threshold as i64)).size(11).width(70)
                .on_input(move |s| Message::SetWarnThresholdStr(k1.clone(), s))
                .style(crate::style::dark_input_style(bp)),
            text(unit_label.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(bp.muted) }),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Metric
        row![
            label("Metric", bp), Space::with_width(Length::Fill),
            pick_list(metrics, Some(sel_metric), move |s: String| {
                let m = if s == "Load" { WarnMetric::Load } else { WarnMetric::Temperature };
                Message::SetWarnMetric(k2.clone(), m)
            }).text_size(11).width(140).style(crate::style::pick_list_style(bp)),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Flash + flash colour swatch
        row![
            toggler(w.flash_enabled).size(14).on_toggle(move |on| Message::SetWarnFlash(k3.clone(), on)).style(crate::style::toggler_style(bp)),
            text("Flash".to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(bp.text) }),
            Space::with_width(Length::Fill),
            color_field(&w.flash_color, bp, move |s| Message::SetWarnFlashColor(k4.clone(), s)),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Gradient + (when on) the gradient hot-colour swatch
        row![
            toggler(w.gradient_mode).size(14).on_toggle(move |on| Message::SetWarnGradient(k5.clone(), on)).style(crate::style::toggler_style(bp)),
            text("Gradient mode \u{2014} unit color shifts blue \u{2192} your color".to_string()).size(10)
                .style(move |_| iced::widget::text::Style { color: Some(bp.text) }),
        ].spacing(6).align_y(iced::Alignment::Center),
        grad_color_row,
    ].spacing(8);

    let config: Element<'a, Message> = body.into();

    let ek = kind.to_string();
    container(column![
        row![
            toggler(enabled).size(16).on_toggle(move |on| Message::SetWarnEnabled(ek.clone(), on)).style(crate::style::toggler_style(p)),
            text(display).size(13)
                .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(8).align_y(iced::Alignment::Center),
        Space::with_height(8),
        config,
    ]).padding(iced::Padding { top: 10.0, right: 12.0, bottom: 10.0, left: 12.0 })
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.tile)),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..Default::default()
        })
        .into()
}

pub fn alerts_view<'a>(settings: &AppSettings, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let intro = text(
        "When the threshold is crossed, the tile background flashes. Gradient mode shifts the \
         unit color from dark-blue (cool) to bright-red (hot). Temperature thresholds are in \u{00B0}C."
            .to_string()
    ).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) });

    let list = column![
        intro,
        Space::with_height(12),
        warn_card(settings, "CPU", p),
        Space::with_height(8),
        warn_card(settings, "GPU", p),
    ];

    let body = column![
        scrollable(list).height(Length::Fill),
        row![Space::with_width(Length::Fill), primary_btn("Save & Close", Message::ClosePopup(win_id), p)]
            .padding(iced::Padding { top: 8.0, right: 0.0, bottom: 0.0, left: 0.0 }),
    ];
    shell("TILE ALERTS", win_id, p, body.into())
}

// ── Game Mode ────────────────────────────────────────────────────────────────

pub const GAME_MODE_SIZE: iced::Size = iced::Size::new(460.0, 640.0);

fn pos_cell<'a>(settings: &AppSettings, pos: SnapPosition, glyph: &str, label_text: &str, p: Palette) -> Element<'a, Message> {
    let active = settings.game_mode_position == pos;
    // Fills its grid column so the 3x3 position grid lines up (the old
    // content-width pills left the rows ragged and pushed "Right" to the edge).
    button(
        container(text(format!("{} {}", glyph, label_text)).size(11).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(if active { Color::WHITE } else { p.text }) }))
            .center_x(Length::Fill)
    )
    .width(Length::FillPortion(1)).padding([5, 6])
    .style(move |_: &iced::Theme, _: button::Status| button::Style {
        background: Some(iced::Background::Color(if active { p.accent } else { p.tile })),
        border: Border { radius: 4.0.into(), ..Border::default() },
        ..Default::default()
    })
    .on_press(Message::SetGameModePosition(pos)).into()
}

pub fn game_mode_view<'a>(settings: &AppSettings, p: Palette, win_id: window::Id, capturing: bool) -> Element<'a, Message> {
    let s = settings;
    let intro = text(
        "Press the hotkey to instantly snap the widget to your primary monitor. Press again to \
         return it. Works system-wide, even while gaming.".to_string()
    ).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) });

    let hotkey_row = row![
        crate::settings_panel::hotkey_field(&s.game_mode_hotkey, capturing, 180.0,
            Message::ArmHotkey(crate::hotkeys::HotkeyTarget::GameMode), p),
        button(text("Clear".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
            .padding([4, 10])
            .style(move |_, _| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() })
            .on_press(Message::ClearHotkey(crate::hotkeys::HotkeyTarget::GameMode)),
    ].spacing(6).align_y(iced::Alignment::Center);

    let empty: Element<'a, Message> = Space::with_width(Length::FillPortion(1)).into();
    let pos_grid = column![
        row![
            pos_cell(s, SnapPosition::TopLeft, "\u{2196}", "Top L", p),
            pos_cell(s, SnapPosition::TopCenter, "\u{2191}", "Top C", p),
            pos_cell(s, SnapPosition::TopRight, "\u{2197}", "Top R", p),
        ].spacing(6),
        row![
            pos_cell(s, SnapPosition::LeftCenter, "\u{2190}", "Left", p),
            empty,
            pos_cell(s, SnapPosition::RightCenter, "\u{2192}", "Right", p),
        ].spacing(6),
        row![
            pos_cell(s, SnapPosition::BottomLeft, "\u{2199}", "Bot L", p),
            pos_cell(s, SnapPosition::BottomCenter, "\u{2193}", "Bot C", p),
            pos_cell(s, SnapPosition::BottomRight, "\u{2198}", "Bot R", p),
        ].spacing(6),
    ].spacing(6);

    let orient_pills = row![
        pill("Use current".into(), s.game_mode_orientation == "Current", Message::SetGameModeOrientation("Current".into()), p),
        Space::with_width(6),
        pill("Horizontal".into(), s.game_mode_orientation == "Horizontal", Message::SetGameModeOrientation("Horizontal".into()), p),
        Space::with_width(6),
        pill("Vertical".into(), s.game_mode_orientation == "Vertical", Message::SetGameModeOrientation("Vertical".into()), p),
    ];

    // Tile toggles (6) — internal names; "Disk" displays as "Storage".
    let tiles: [(&str, &str); 6] = [
        ("CPU", "CPU"), ("GPU", "GPU"), ("RAM", "RAM"),
        ("Network", "Network"), ("Disk", "Storage"), ("Clock", "Clock"),
    ];
    let mut row0 = Vec::<Element<'a, Message>>::new();
    let mut row1 = Vec::<Element<'a, Message>>::new();
    for (i, (internal, display)) in tiles.iter().enumerate() {
        let on = s.game_mode_tiles.iter().any(|t| t == internal);
        let name = internal.to_string();
        let el: Element<'a, Message> = row![
            toggler(on).size(14).on_toggle(move |v| Message::ToggleGameModeTile(name.clone(), v)).style(crate::style::toggler_style(p)),
            text(display.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)).into();
        if i < 3 { row0.push(el); } else { row1.push(el); }
    }
    let tiles_grid = column![row(row0).spacing(8), row(row1).spacing(8)].spacing(6);

    let content = column![
        intro,
        Space::with_height(10),
        toggle_row("Enable Game Mode", s.game_mode_enabled, Message::SetGameModeEnabled, p),
        Space::with_height(8),
        label("Hotkey", p),
        hotkey_row,
        Space::with_height(10),
        section_header("SNAP POSITION", p),
        label("Always snaps to the primary monitor.", p),
        Space::with_height(6),
        pos_grid,
        Space::with_height(10),
        section_header("APPEARANCE WHEN ACTIVE", p),
        row![label("Opacity", p), Space::with_width(Length::Fill),
            text(format!("{:.0}%", s.game_mode_opacity * 100.0)).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) })],
        crate::settings_panel::marked_slider(0.1, 1.0, s.game_mode_opacity, 0.01, 0.7, p, Message::SetGameModeOpacity),
        Space::with_height(6),
        label("Orientation", p),
        orient_pills,
        Space::with_height(8),
        toggle_row("Enable click-through while in Game Mode", s.game_mode_click_through, Message::SetGameModeClickThrough, p),
        Space::with_height(10),
        section_header("TILES WHEN ACTIVE", p),
        Space::with_height(4),
        tiles_grid,
    ].spacing(2);

    let body = column![
        scrollable(content).height(Length::Fill),
        row![Space::with_width(Length::Fill), primary_btn("Save & Close", Message::ClosePopup(win_id), p)]
            .padding(iced::Padding { top: 8.0, right: 0.0, bottom: 0.0, left: 0.0 }),
    ];
    shell("GAME MODE", win_id, p, body.into())
}

// ── Help ─────────────────────────────────────────────────────────────────────

pub const HELP_SIZE: iced::Size = iced::Size::new(520.0, 600.0);

const HELP_SECTIONS: [(&str, &str); 9] = [
    ("Tiles", "Toggle CPU, GPU, RAM, Network, Storage and Clock tiles from Settings. Drag the widget anywhere; it remembers its position."),
    ("Layout", "Switch between horizontal and vertical orientation. Adjust tile width/height and overall UI scale to taste."),
    ("Themes & Skins", "57 built-in color themes and 16 skins. Cycle with the arrows, roll the dice for a random pick, or hand-edit the five theme colors."),
    ("Tile Alerts", "Set per-tile temperature or load thresholds. When crossed the tile flashes; gradient mode shifts the unit color from blue (cool) to red (hot)."),
    ("Game Mode", "Bind a hotkey to instantly snap the widget to a corner of your primary monitor with a custom opacity and tile set — ideal while gaming."),
    ("Behavior", "Always-on-top, click-through, snap-to-edges and snap-to-windows. Optionally launch fluxid at Windows sign-in."),
    ("Network & Disk", "Pick which adapter and disk to monitor. Choose how the traffic indicator animates and how disk drives are labelled."),
    ("Fonts", "Choose primary, secondary and indicator fonts. Sync keeps all three in step. Per-element size offsets fine-tune the look."),
    ("Updates", "Auto, Manual or Off. Check for new releases on demand from the Updates section in Settings."),
];

pub fn help_view<'a>(_settings: &AppSettings, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let mut col = column![].spacing(14);
    for (title, desc) in HELP_SECTIONS {
        col = col.push(column![
            text(title.to_string()).size(13)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
            text(desc.to_string()).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(3));
    }
    // Footer: the project name + how to say it.
    col = col.push(
        container(
            column![
                text("fluxid".to_string()).size(12)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
                text("pronounced like \u{201C}fluid\u{201D} \u{2014} the x is silent".to_string()).size(10)
                    .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            ].spacing(1)
        ).padding(iced::Padding { top: 6.0, right: 0.0, bottom: 0.0, left: 0.0 })
    );
    let body = scrollable(container(col).padding(iced::Padding { top: 4.0, right: 6.0, bottom: 8.0, left: 0.0 })).height(Length::Fill);
    shell("HELP", win_id, p, body.into())
}

// ── Optional CPU sensor driver (PawnIO) ─────────────────────────────────────
//
// Mirrors the C# CpuTempDialog: a pitch + "More info" + Install, a progress
// panel during the download/verify/elevated install, and a result panel that
// either confirms success or offers a manual-download fallback. When the driver
// is already present the primary panel becomes a "remove" manager instead.

pub const CPU_DRIVER_SIZE: iced::Size = iced::Size::new(470.0, 420.0);

pub fn cpu_driver_view<'a>(
    stage: &crate::CpuDriverStage,
    installed: bool,
    p: Palette,
    win_id: window::Id,
) -> Element<'a, Message> {
    use crate::CpuDriverStage as S;
    let heading = |t: &str, color: Color| -> Element<'a, Message> {
        text(t.to_string()).size(14)
            .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(color) }).into()
    };
    let para = |t: &str| -> Element<'a, Message> {
        text(t.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }).into()
    };
    let muted = |t: &str| -> Element<'a, Message> {
        text(t.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) }).into()
    };
    let ibtn = |lbl: &str, msg: Message| crate::style::inline_btn(lbl, msg, p);
    let red = Color::from_rgb(0.90, 0.33, 0.24);

    let body: Element<'a, Message> = match stage {
        S::Primary if installed => column![
            heading("CPU temperature sensor", p.accent),
            para("The optional sensor driver (PawnIO) is installed, so your CPU temperature reads directly from the hardware."),
            Space::with_height(6),
            muted("You can remove it at any time \u{2014} the rest of the widget is unaffected."),
            Space::with_height(Length::Fill),
            row![
                ibtn("Remove driver", Message::CpuDriverUninstall),
                Space::with_width(Length::Fill),
                primary_btn("Close", Message::ClosePopup(win_id), p),
            ].align_y(iced::Alignment::Center),
        ].spacing(6).height(Length::Fill).into(),

        S::Primary => column![
            heading("Turn on CPU temperature", p.accent),
            para("Reading the CPU's die temperature needs a small hardware-sensor driver. fluxid uses PawnIO \u{2014} a free, open-source, Microsoft-signed driver built specifically for safe sensor access."),
            Space::with_height(4),
            muted("fluxid never bundles the driver. It downloads the official signed installer, verifies its signature, then runs it. You'll see one Windows permission prompt (driver installs require it). Everything else on the widget works without this."),
            Space::with_height(Length::Fill),
            row![
                ibtn("More info", Message::CpuDriverMoreInfo),
                Space::with_width(Length::Fill),
                ibtn("Cancel", Message::ClosePopup(win_id)),
                Space::with_width(8),
                primary_btn("Install", Message::CpuDriverInstall, p),
            ].align_y(iced::Alignment::Center),
        ].spacing(6).height(Length::Fill).into(),

        S::Info => column![
            heading("About the sensor driver", p.accent),
            para("PawnIO is an open-source kernel driver for hardware monitoring. Unlike older sensor drivers, it only runs cryptographically-signed, sandboxed modules \u{2014} which makes it far safer than the legacy alternatives."),
            Space::with_height(6),
            muted("Review it yourself \u{2014} these open in your browser:"),
            row![
                ibtn("Home page", Message::OpenUrl(crate::cpu_driver::HOME_PAGE_URL.into())),
                ibtn("Source code", Message::OpenUrl(crate::cpu_driver::SOURCE_URL.into())),
                ibtn("Direct download", Message::OpenUrl(crate::cpu_driver::DOWNLOAD_URL.into())),
            ].spacing(8),
            Space::with_height(Length::Fill),
            row![
                ibtn("Back", Message::CpuDriverBack),
                Space::with_width(Length::Fill),
                primary_btn("Close", Message::ClosePopup(win_id), p),
            ].align_y(iced::Alignment::Center),
        ].spacing(6).height(Length::Fill).into(),

        S::Progress(msg) => column![
            Space::with_height(Length::Fill),
            container(text(msg.clone()).size(12)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }))
                .center_x(Length::Fill),
            Space::with_height(8),
            container(muted("Follow the Windows permission prompt if it appears."))
                .center_x(Length::Fill),
            Space::with_height(Length::Fill),
        ].into(),

        S::Done { ok, title, body: b, show_fallback } => {
            let mut col = column![
                heading(title, if *ok { p.accent } else { red }),
                para(b),
            ].spacing(8);
            if *show_fallback {
                col = col.push(Space::with_height(4));
                col = col.push(muted("If automatic setup keeps failing, you can download and run the official installer yourself, then reopen this dialog."));
                col = col.push(row![
                    ibtn("Open download", Message::OpenUrl(crate::cpu_driver::DOWNLOAD_URL.into())),
                    ibtn("Home page", Message::OpenUrl(crate::cpu_driver::HOME_PAGE_URL.into())),
                ].spacing(8));
            }
            col = col.push(Space::with_height(Length::Fill));
            col = col.push(row![
                Space::with_width(Length::Fill),
                primary_btn("Done", Message::ClosePopup(win_id), p),
            ]);
            col.height(Length::Fill).into()
        }
    };

    shell("CPU TEMPERATURE", win_id, p, body)
}

// ── Utilities (Tweaks) ───────────────────────────────────────────────────────

pub const UTILITIES_SIZE: iced::Size = iced::Size::new(460.0, 430.0);
pub const WINDOW_PICKER_SIZE: iced::Size = iced::Size::new(420.0, 460.0);

pub fn utilities_view<'a>(blocklist: &'a text_editor::Content, status: &str, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    // C# InlineBtn: tile fill, 1px border, radius 6; hover accents.
    let ibtn = |lbl: &str, msg: Message| crate::style::inline_btn(lbl, msg, p);
    let card = |title: &str, desc: &str, action: Element<'a, Message>| -> Element<'a, Message> {
        container(column![
            text(title.to_string()).size(13)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            text(desc.to_string()).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            action,
        ].spacing(8))
        .width(Length::Fill).padding(iced::Padding { top: 10.0, right: 14.0, bottom: 10.0, left: 14.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.tile)),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..Default::default()
        })
        .into()
    };

    let ct = card(
        "Chris Titus Win Utility",
        "Debloat Windows, manage updates, install apps, optimize performance. \
         Opens the official site so you can review and run it yourself.",
        row![
            ibtn("Open website", Message::OpenUrl("https://christitus.com/windows-tool/".into())),
            Space::with_width(Length::Fill),
        ].into(),
    );

    let editor = text_editor(blocklist)
        .height(Length::Fixed(72.0))
        .padding(6)
        .on_action(Message::BlocklistAction)
        .style(move |_t: &iced::Theme, _s| iced::widget::text_editor::Style {
            background: iced::Background::Color(crate::style::field_bg(p)),
            border: Border { radius: 4.0.into(), width: 1.0, color: Color { a: 0.5, ..p.muted } },
            icon: p.muted,
            placeholder: Color { a: 0.5, ..p.muted },
            value: p.text,
            selection: p.accent,
        });
    let blocklist_card = card(
        "Window snap blocklist",
        "Windows with titles matching any line below won't be used as snap targets (substring match, case-insensitive).",
        column![
            editor,
            row![
                ibtn("Save blocklist", Message::SaveBlocklist),
                ibtn("Pick window", Message::PickWindow),
                text(status.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            ].spacing(8).align_y(iced::Alignment::Center),
        ].spacing(6).into(),
    );

    let disclaimer = container(
        text("Disclaimer: Third-party tools linked here are not bundled with, vetted by, or endorsed by fluxid. fluxid only opens their official website \u{2014} review anything you download or run yourself. Use at your own risk.".to_string())
            .size(10).style(move |_| iced::widget::text::Style { color: Some(Color::from_rgb(1.0, 0.90, 0.84)) })
    )
    .width(Length::Fill).padding(iced::Padding { top: 8.0, right: 10.0, bottom: 8.0, left: 10.0 })
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba8(0xC0, 0x40, 0x18, 0.28))),
        border: Border { radius: 6.0.into(), width: 1.0, color: Color::from_rgba8(0xE0, 0x6A, 0x40, 0.55) },
        ..Default::default()
    });

    let body = scrollable(
        column![ct, blocklist_card, disclaimer].spacing(8)
            .padding(iced::Padding { top: 4.0, right: 6.0, bottom: 4.0, left: 0.0 })
    ).height(Length::Fill);
    shell("UTILITIES", win_id, p, body.into())
}

pub const REMOTE_SIZE: iced::Size = iced::Size::new(480.0, 640.0);

pub fn remote_view<'a>(
    remote: crate::settings_panel::RemoteView,
    settings: &AppSettings,
    p: Palette,
    win_id: window::Id,
) -> Element<'a, Message> {
    let ibtn = |lbl: String, msg: Message| crate::style::inline_btn(lbl, msg, p);
    let fl = |t: &str| label(t, p);
    let sh = move |title: &str, sub: &str| -> Element<'a, Message> {
        column![
            text(title.to_uppercase()).size(12)
                .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
            text(sub.to_string()).size(10)
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        ].spacing(2).into()
    };
    let dot = move |connected: bool| -> Element<'a, Message> {
        let c = if connected { Color::from_rgb8(0x3D, 0xC9, 0x8A) } else { Color::from_rgb8(0xCD, 0x5C, 0x5C) };
        container(Space::new(6, 6)).style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(c)),
            border: Border { radius: 3.0.into(), ..Border::default() },
            ..Default::default()
        }).into()
    };

    let feed_toggle = row![
        toggler(remote.feed_on).size(14).on_toggle(Message::SetTcpFeedEnabled).style(crate::style::toggler_style(p)),
        text("Enable TCP sensor feed (port 5199)").size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
    ].spacing(6).align_y(iced::Alignment::Center);

    let key_row = row![
        text_input("", &remote.handshake_key).size(10).width(280).style(crate::style::dark_input_style(p)),
        ibtn("Copy".into(), Message::CopyHandshakeKey),
    ].spacing(8).align_y(iced::Alignment::Center);

    let mut col = column![
        sh("Host", "Share this machine's sensor data over your local network. Others connect using the key below."),
        feed_toggle,
        fl("Handshake key"),
        key_row,
        row![ibtn("Regenerate Key\u{2026}".into(), Message::RegenerateKey), Space::with_width(Length::Fill)]
            .spacing(8).align_y(iced::Alignment::Center),
        text("\u{26A0} Regenerating disconnects all remote devices.").size(11)
            .style(move |_| iced::widget::text::Style { color: Some(Color { a: 0.45, ..p.muted }) }),
        Space::with_height(8),
        sh("Remote Devices", "Monitor other machines running fluxid. Add them using their IP and handshake key."),
        fl(&format!("{} / 5 devices configured", remote.devices.len())),
    ].spacing(6);

    for d in &remote.devices {
        let connected = remote.conn.get(&d.id).copied().unwrap_or(false);
        let id_popout = d.id.clone();
        let id_config = d.id.clone();
        let id_remove = d.id.clone();
        let gear = crate::style::with_tip(
            button(text("\u{2699}").size(13).font(crate::style::ICONS)
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding(iced::Padding { top: 2.0, right: 6.0, bottom: 2.0, left: 6.0 })
                .style(move |_, _| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 6.0.into(), width: 1.0, color: p.muted }, ..Default::default() })
                .on_press(Message::OpenPopoutConfig(id_config)),
            "Configure this device's popout appearance (colors, tiles, labels).", p);
        let row_el = container(row![
            dot(connected),
            Space::with_width(6),
            text(d.name.clone()).size(12)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            Space::with_width(Length::Fill),
            text(d.host.clone()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            Space::with_width(8),
            gear,
            ibtn("Popout".into(), Message::OpenPopout(id_popout)),
            button(text("\u{2715}").size(11).font(iced::Font::with_name("Segoe UI Symbol"))
                .style(move |_| iced::widget::text::Style { color: Some(Color::from_rgb8(0xCD, 0x5C, 0x5C)) }))
                .padding(iced::Padding { top: 2.0, right: 4.0, bottom: 2.0, left: 4.0 })
                .style(|_, _| button::Style { background: None, ..Default::default() })
                .on_press(Message::RemoveDevice(id_remove)),
        ].align_y(iced::Alignment::Center).spacing(2))
        .padding(iced::Padding { top: 6.0, right: 10.0, bottom: 6.0, left: 10.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.tile)),
            border: Border { radius: 4.0.into(), ..Border::default() },
            ..Default::default()
        });
        col = col.push(row_el);
    }

    if remote.add_open {
        let status_color = if remote.test_ok { p.accent } else { Color::from_rgb8(0xCD, 0x5C, 0x5C) };
        let status_text = remote.test_status.clone();
        let add_panel = container(column![
            text("Add remote device").size(12)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            Space::with_height(8),
            row![
                column![fl("Name"),
                    text_input("", &remote.new_name).size(11).on_input(Message::SetNewDeviceName).style(crate::style::dark_input_style(p)),
                ].spacing(2).width(Length::FillPortion(1)),
                Space::with_width(10),
                column![fl("IP address"),
                    text_input("", &remote.new_ip).size(11).on_input(Message::SetNewDeviceIp).style(crate::style::dark_input_style(p)),
                ].spacing(2).width(Length::FillPortion(1)),
            ],
            Space::with_height(6),
            fl("Handshake key"),
            text_input("", &remote.new_key).size(11).on_input(Message::SetNewDeviceKey).style(crate::style::dark_input_style(p)),
            Space::with_height(8),
            row![
                ibtn("Test".into(), Message::TestDevice),
                ibtn("Save".into(), Message::SaveDevice),
                ibtn("Cancel".into(), Message::CancelAddDevice),
                text(status_text).size(11).style(move |_| iced::widget::text::Style { color: Some(status_color) }),
            ].spacing(4).align_y(iced::Alignment::Center),
        ].spacing(2))
        .padding(iced::Padding { top: 8.0, right: 12.0, bottom: 8.0, left: 12.0 })
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.tile)),
            border: Border { radius: 4.0.into(), ..Border::default() },
            ..Default::default()
        });
        col = col.push(Space::with_height(3));
        col = col.push(add_panel);
    } else if remote.devices.len() < 5 {
        col = col.push(Space::with_height(6));
        col = col.push(row![ibtn("+ Add Device".into(), Message::ShowAddDevice), Space::with_width(Length::Fill)]);
    }

    col = col.push(Space::with_height(8));
    col = col.push(row![
        toggler(settings.show_remote_status_dot).size(14)
            .on_toggle(Message::SetShowRemoteStatusDot).style(crate::style::toggler_style(p)),
        text("Show a green/red status dot on the widget's device tabs").size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
    ].spacing(6).align_y(iced::Alignment::Center));

    let body = scrollable(col.padding(iced::Padding { top: 4.0, right: 6.0, bottom: 4.0, left: 0.0 }))
        .height(Length::Fill);
    shell("REMOTE MONITORING", win_id, p, body.into())
}

pub fn window_picker_view<'a>(titles: Vec<String>, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let mut col = column![
        text("Click a window to add its title to the blocklist.".to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_height(4),
    ].spacing(3);
    if titles.is_empty() {
        col = col.push(text("No open windows found.".to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) }));
    }
    for t in titles {
        let title = t.clone();
        col = col.push(
            button(text(title).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }))
                .width(Length::Fill)
                .padding(iced::Padding { top: 5.0, right: 10.0, bottom: 5.0, left: 10.0 })
                .style(move |_: &iced::Theme, status: button::Status| {
                    let hover = matches!(status, button::Status::Hovered);
                    button::Style {
                        background: Some(iced::Background::Color(if hover { p.accent } else { p.tile })),
                        text_color: if hover { Color::WHITE } else { p.text },
                        border: Border { radius: 4.0.into(), ..Border::default() },
                        ..Default::default()
                    }
                })
                .on_press(Message::PickWindowChosen(t))
        );
    }
    let body = scrollable(container(col).padding(iced::Padding { top: 4.0, right: 6.0, bottom: 8.0, left: 0.0 })).height(Length::Fill);
    shell("PICK WINDOW", win_id, p, body.into())
}

// ── Per-device Popout settings editor ────────────────────────────────────────

pub const POPOUT_CONFIG_SIZE: iced::Size = iced::Size::new(360.0, 540.0);

pub fn popout_config_view<'a>(dev: Option<&'a RemoteDevice>, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let dev = match dev {
        Some(d) => d,
        None => return shell("POPOUT", win_id, p,
            text("Device not found.".to_string()).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }).into()),
    };
    let id = dev.id.clone();
    let po = &dev.popout;

    let section = |t: &str| -> Element<'a, Message> {
        text(t.to_string()).size(13)
            .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(p.accent) }).into()
    };

    let mut col = column![
        text(format!("Popout appearance for \u{201C}{}\u{201D}", dev.name)).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_height(4),
        section("Colors"),
    ].spacing(8);

    // Sync toggle
    let sid = id.clone();
    col = col.push(row![
        toggler(po.sync_colors).size(14).on_toggle(move |b| Message::PopoutSyncColors(sid.clone(), b)).style(crate::style::toggler_style(p)),
        text("Use the widget's theme colors").size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
    ].spacing(6).align_y(iced::Alignment::Center));

    // Per-colour rows (only when not synced)
    if !po.sync_colors {
        let color_row = |slot: u8, name: &str, hex: &str| -> Element<'a, Message> {
            let c = crate::style::parse_hex(hex, p.muted);
            let cid = id.clone();
            row![
                text(name.to_string()).size(11).width(Length::Fixed(80.0))
                    .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
                container(Space::new(16, 16)).style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(c)),
                    border: Border { radius: 3.0.into(), width: 1.0, color: Color { a: 0.3, ..p.muted } }, ..Default::default()
                }),
                Space::with_width(6),
                text_input("#AARRGGBB", hex).size(11).width(Length::Fixed(150.0))
                    .on_input(move |s| Message::PopoutColor(cid.clone(), slot, s))
                    .style(crate::style::dark_input_style(p)),
            ].spacing(4).align_y(iced::Alignment::Center).into()
        };
        col = col.push(color_row(0, "Background", &po.bg));
        col = col.push(color_row(1, "Tile", &po.tile));
        col = col.push(color_row(2, "Accent", &po.accent));
        col = col.push(color_row(3, "Text", &po.text));
        col = col.push(color_row(4, "Muted", &po.muted));
    }

    // Opacity
    let oid = id.clone();
    col = col.push(section("Opacity"));
    col = col.push(row![
        slider(0.3..=1.0, po.opacity, move |v| Message::PopoutOpacity(oid.clone(), v)).step(0.05).width(Length::Fill).style(crate::style::slider_style(p)),
        Space::with_width(8),
        text(format!("{:.0}%", po.opacity * 100.0)).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
    ].spacing(4).align_y(iced::Alignment::Center));

    // Tiles
    col = col.push(section("Tiles"));
    let tile_toggle = |name: &'static str, on: bool| -> Element<'a, Message> {
        let tid = id.clone();
        row![
            toggler(on).size(14).on_toggle(move |b| Message::PopoutTile(tid.clone(), name.to_string(), b)).style(crate::style::toggler_style(p)),
            text(name.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)).into()
    };
    col = col.push(row![tile_toggle("CPU", po.show_cpu), tile_toggle("GPU", po.show_gpu), tile_toggle("RAM", po.show_ram)].spacing(4));
    col = col.push(row![tile_toggle("Network", po.show_network), tile_toggle("Storage", po.show_storage), Space::with_width(Length::FillPortion(1))].spacing(4));

    // Labels
    col = col.push(section("Tile labels"));
    let cl = id.clone();
    col = col.push(row![
        text("CPU".to_string()).size(11).width(Length::Fixed(40.0)).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        text_input("Auto", &po.cpu_label).size(11).on_input(move |s| Message::PopoutLabel(cl.clone(), 0, s)).style(crate::style::dark_input_style(p)),
    ].spacing(6).align_y(iced::Alignment::Center));
    let gl = id.clone();
    col = col.push(row![
        text("GPU".to_string()).size(11).width(Length::Fixed(40.0)).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        text_input("Auto", &po.gpu_label).size(11).on_input(move |s| Message::PopoutLabel(gl.clone(), 1, s)).style(crate::style::dark_input_style(p)),
    ].spacing(6).align_y(iced::Alignment::Center));

    // Alerts (per-device): CPU/GPU temperature or load thresholds that flash
    // this popout's tile, independent of the local widget's alerts.
    col = col.push(section("Alerts"));
    let warn_block = |kind: &'static str| -> Element<'a, Message> {
        let w = po.warn(kind).cloned().unwrap_or_default();
        let metric_label = if matches!(w.metric, WarnMetric::Load) { "Load" } else { "Temperature" };
        let unit = if matches!(w.metric, WarnMetric::Load) { " %" } else { " \u{00B0}C" };
        let (ke, km, kt, kf, kfc, kg, kgc) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
        let metrics = vec!["Temperature".to_string(), "Load".to_string()];
        let gradient_row: Element<'a, Message> = if w.gradient_mode {
            row![
                text("Gradient color".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
                Space::with_width(Length::Fill),
                color_field(&w.gradient_color, p, move |s| Message::PopoutWarnGradientColor(kgc.clone(), kind.to_string(), s)),
            ].spacing(6).align_y(iced::Alignment::Center).into()
        } else {
            Space::with_height(0).into()
        };
        column![
            row![
                toggler(w.enabled).size(14).on_toggle(move |b| Message::PopoutWarnEnabled(ke.clone(), kind.to_string(), b)).style(crate::style::toggler_style(p)),
                text(format!("{} alert", kind)).size(11).font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT }).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            ].spacing(6).align_y(iced::Alignment::Center),
            row![
                pick_list(metrics, Some(metric_label.to_string()), move |s: String| {
                    let m = if s == "Load" { WarnMetric::Load } else { WarnMetric::Temperature };
                    Message::PopoutWarnMetric(km.clone(), kind.to_string(), m)
                }).text_size(11).width(Length::Fixed(130.0)).style(crate::style::pick_list_style(p)),
                Space::with_width(Length::Fill),
                text_input("", &format!("{}", w.threshold as i64)).size(11).width(Length::Fixed(56.0))
                    .on_input(move |s| Message::PopoutWarnThreshold(kt.clone(), kind.to_string(), s))
                    .style(crate::style::dark_input_style(p)),
                text(unit.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            ].spacing(6).align_y(iced::Alignment::Center),
            row![
                toggler(w.flash_enabled).size(14).on_toggle(move |b| Message::PopoutWarnFlash(kf.clone(), kind.to_string(), b)).style(crate::style::toggler_style(p)),
                text("Flash".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
                Space::with_width(Length::Fill),
                color_field(&w.flash_color, p, move |s| Message::PopoutWarnFlashColor(kfc.clone(), kind.to_string(), s)),
            ].spacing(6).align_y(iced::Alignment::Center),
            row![
                toggler(w.gradient_mode).size(14).on_toggle(move |b| Message::PopoutWarnGradient(kg.clone(), kind.to_string(), b)).style(crate::style::toggler_style(p)),
                text("Gradient mode".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            ].spacing(6).align_y(iced::Alignment::Center),
            gradient_row,
        ].spacing(4).into()
    };
    col = col.push(warn_block("CPU"));
    col = col.push(warn_block("GPU"));

    let body = scrollable(
        container(col.width(Length::Fill)).width(Length::Fill)
            .padding(iced::Padding { top: 4.0, right: 8.0, bottom: 8.0, left: 0.0 })
    ).width(Length::Fill).height(Length::Fill);
    shell("POPOUT", win_id, p, body.into())
}

// ── Theme Store (bundled game theme packs) ───────────────────────────────────

pub const THEME_STORE_SIZE: iced::Size = iced::Size::new(460.0, 600.0);

// A rounded colour chip (12px, matches the C# store swatches).
fn chip<'a>(hex: &str, p: Palette) -> Element<'a, Message> {
    let c = crate::style::parse_hex(hex, p.muted);
    container(Space::new(12, 12)).style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(c)),
        border: Border { radius: 3.0.into(), ..Border::default() },
        ..Default::default()
    }).into()
}

// An on-brand checkbox for multi-selecting themes to "Install selected".
fn cbox<'a>(checked: bool, p: Palette, on_toggle: impl Fn(bool) -> Message + 'a) -> Element<'a, Message> {
    checkbox("", checked).size(16).on_toggle(on_toggle)
        .style(move |_t: &iced::Theme, status: checkbox::Status| {
            let on = matches!(status,
                checkbox::Status::Active { is_checked: true }
                | checkbox::Status::Hovered { is_checked: true }
                | checkbox::Status::Disabled { is_checked: true });
            checkbox::Style {
                background: iced::Background::Color(if on { p.accent } else { p.tile }),
                icon_color: Color::WHITE,
                border: Border { radius: 4.0.into(), width: 1.0, color: if on { p.accent } else { Color { a: 0.5, ..p.muted } } },
                text_color: None,
            }
        }).into()
}

// A status pill: "Installed" (accent) or "Available" (muted) — or a partial
// "n / m" badge for packs.
fn status_pill<'a>(label: String, accent: bool, p: Palette) -> Element<'a, Message> {
    let c = if accent { p.accent } else { p.muted };
    container(text(label).size(9)
        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(c) }))
        .padding(iced::Padding { top: 2.0, right: 7.0, bottom: 2.0, left: 7.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color { a: 0.14, ..c })),
            border: Border { radius: 5.0.into(), width: 1.0, color: Color { a: 0.4, ..c } },
            ..Default::default()
        }).into()
}

// A small Install (accent fill) / Remove (red outline) action button.
fn action_btn<'a>(installed: bool, msg: Message, p: Palette) -> Element<'a, Message> {
    let red = Color::from_rgb8(0xCD, 0x5C, 0x5C);
    let (label, fg) = if installed { ("Remove", red) } else { ("Install", Color::WHITE) };
    button(text(label.to_string()).size(11)
        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(fg) }))
        .padding(iced::Padding { top: 4.0, right: 12.0, bottom: 4.0, left: 12.0 })
        .style(move |_: &iced::Theme, st: button::Status| {
            let hover = matches!(st, button::Status::Hovered);
            if installed {
                button::Style {
                    background: Some(iced::Background::Color(if hover { Color { a: 0.18, ..red } } else { Color::TRANSPARENT })),
                    border: Border { radius: 6.0.into(), width: 1.0, color: red },
                    ..Default::default()
                }
            } else {
                button::Style {
                    background: Some(iced::Background::Color(if hover { Color { a: 0.85, ..p.accent } } else { p.accent })),
                    border: Border { radius: 6.0.into(), ..Border::default() },
                    ..Default::default()
                }
            }
        })
        .on_press(msg).into()
}

/// Theme Store — a 2-column grid of franchise cards (matching the C#
/// ThemeStoreWindow). `franchise` selects the drill-in theme list for a pack.
pub fn theme_store_view<'a>(franchise: Option<usize>, settings: &AppSettings, sel: &std::collections::HashSet<String>, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let packs = crate::style::theme_packs();
    match franchise {
        Some(pi) if pi < packs.len() => franchise_detail(pi, settings, sel, p, win_id),
        _ => store_grid(settings, p, win_id),
    }
}

fn store_grid<'a>(settings: &AppSettings, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let packs = crate::style::theme_packs();
    let total: usize = packs.iter().map(|p| p.themes.len()).sum();
    let is_installed = |name: &str| settings.installed_themes.iter().any(|t| t.name == name);

    let summary = text(format!("{} packs \u{00B7} {} themes \u{00B7} {} installed", packs.len(), total, settings.installed_themes.len()))
        .size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) });
    let hint = text("Open a pack to install or remove its themes. Installed themes appear in Choose a Theme.")
        .size(10).style(move |_| iced::widget::text::Style { color: Some(Color { a: 0.7, ..p.muted }) });

    let card = |pi: usize| -> Element<'a, Message> {
        let pack = &packs[pi];
        let installed_n = pack.themes.iter().filter(|t| is_installed(&t.name)).count();
        let status: Element<'a, Message> = if installed_n == 0 {
            status_pill("Available".into(), false, p)
        } else if installed_n == pack.themes.len() {
            status_pill("Installed".into(), true, p)
        } else {
            status_pill(format!("{} / {}", installed_n, pack.themes.len()), true, p)
        };
        let mut sw = row![].spacing(3);
        for t in pack.themes.iter().take(6) { sw = sw.push(chip(&t.accent, p)); }
        button(column![
            row![
                text(pack.franchise.clone()).size(11)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                Space::with_width(Length::Fill),
                status,
            ].align_y(iced::Alignment::Center),
            text(format!("{} themes", pack.themes.len())).size(9)
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            Space::with_height(6),
            sw,
        ].spacing(2))
        .width(Length::Fill)
        .padding(10)
        .style(move |_: &iced::Theme, status: button::Status| {
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(p.tile)),
                border: Border { radius: 8.0.into(), width: 1.0, color: if hover { p.accent } else { Color::TRANSPARENT } },
                ..Default::default()
            }
        })
        .on_press(Message::ThemeStoreOpenFranchise(pi))
        .into()
    };

    let mut grid = column![].spacing(6);
    let mut i = 0;
    while i < packs.len() {
        let left = card(i);
        let right: Element<'a, Message> = if i + 1 < packs.len() { card(i + 1) } else { Space::with_width(Length::Fill).into() };
        grid = grid.push(row![left, right].spacing(6));
        i += 2;
    }

    let body = column![
        summary,
        hint,
        Space::with_height(6),
        scrollable(container(grid).padding(iced::Padding { top: 0.0, right: 8.0, bottom: 8.0, left: 0.0 })).height(Length::Fill),
    ].spacing(2);
    shell("THEME STORE", win_id, p, body.into())
}

fn franchise_detail<'a>(pi: usize, settings: &AppSettings, sel: &std::collections::HashSet<String>, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let pack = &crate::style::theme_packs()[pi];
    let is_installed = |name: &str| settings.installed_themes.iter().any(|t| t.name == name);
    let installed_n = pack.themes.iter().filter(|t| is_installed(&t.name)).count();
    let any = installed_n > 0;
    let all = installed_n == pack.themes.len() && !pack.themes.is_empty();
    let red = Color::from_rgb8(0xCD, 0x5C, 0x5C);
    // Themes ticked for "Install selected" (only ones not already installed).
    let sel_n = pack.themes.iter().filter(|t| !is_installed(&t.name) && sel.contains(&t.name)).count();

    let back = button(text("\u{2039} All packs".to_string()).size(11)
        .style(move |_| iced::widget::text::Style { color: Some(p.accent) }))
        .padding(iced::Padding { top: 3.0, right: 8.0, bottom: 3.0, left: 0.0 })
        .style(|_, _| button::Style { background: None, ..Default::default() })
        .on_press(Message::ThemeStoreBack);

    let header = row![
        back,
        Space::with_width(Length::Fill),
        text(format!("{} \u{00B7} {} / {} installed", pack.franchise, installed_n, pack.themes.len())).size(10)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
    ].align_y(iced::Alignment::Center);

    // Pack-wide install/remove.
    let install_all = button(text("Install all".to_string()).size(11)
        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(if all { Color { a: 0.45, ..p.muted } } else { Color::WHITE }) }))
        .padding(iced::Padding { top: 4.0, right: 12.0, bottom: 4.0, left: 12.0 })
        .style(move |_: &iced::Theme, _| button::Style {
            background: Some(iced::Background::Color(if all { Color { a: 0.2, ..p.accent } } else { p.accent })),
            border: Border { radius: 6.0.into(), ..Border::default() }, ..Default::default()
        })
        .on_press_maybe((!all).then_some(Message::ThemeStoreTogglePack(pi, true)));
    let remove_all = button(text("Remove all".to_string()).size(11)
        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(if any { red } else { Color { a: 0.4, ..p.muted } }) }))
        .padding(iced::Padding { top: 4.0, right: 12.0, bottom: 4.0, left: 12.0 })
        .style(move |_: &iced::Theme, _| button::Style {
            background: None,
            border: Border { radius: 6.0.into(), width: 1.0, color: if any { red } else { Color { a: 0.3, ..p.muted } } }, ..Default::default()
        })
        .on_press_maybe(any.then_some(Message::ThemeStoreTogglePack(pi, false)));
    let install_sel_label = if sel_n > 0 { format!("Install selected ({sel_n})") } else { "Install selected".to_string() };
    let install_sel = button(text(install_sel_label).size(11)
        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(if sel_n > 0 { Color::WHITE } else { Color { a: 0.45, ..p.muted } }) }))
        .padding(iced::Padding { top: 4.0, right: 12.0, bottom: 4.0, left: 12.0 })
        .style(move |_: &iced::Theme, st: button::Status| {
            let hover = matches!(st, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(if sel_n == 0 { Color { a: 0.2, ..p.accent } } else if hover { Color { a: 0.85, ..p.accent } } else { p.accent })),
                border: Border { radius: 6.0.into(), ..Border::default() }, ..Default::default()
            }
        })
        .on_press_maybe((sel_n > 0).then_some(Message::ThemeStoreInstallSelected(pi)));
    let actions = row![install_all, install_sel, remove_all, Space::with_width(Length::Fill)].spacing(6).align_y(iced::Alignment::Center);

    let mut rows = column![].spacing(2);
    for (ti, t) in pack.themes.iter().enumerate() {
        let installed = is_installed(&t.name);
        let selected = sel.contains(&t.name);
        let label = t.name.strip_prefix(&format!("{} ", pack.franchise)).unwrap_or(&t.name).to_string();
        // Installed rows show a spacer where the select box would be.
        let lead: Element<'a, Message> = if installed {
            Space::with_width(16).into()
        } else {
            cbox(selected, p, move |b| Message::ThemeStoreToggleSelect(pi, ti, b))
        };
        rows = rows.push(
            container(row![
                lead,
                Space::with_width(8),
                chip(&t.bg, p), chip(&t.tile, p), chip(&t.accent, p), chip(&t.text, p), chip(&t.muted, p),
                Space::with_width(10),
                text(label).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                Space::with_width(Length::Fill),
                status_pill(if installed { "Installed".into() } else { "Available".into() }, installed, p),
                Space::with_width(8),
                action_btn(installed, Message::ThemeStoreToggleTheme(pi, ti, !installed), p),
            ].spacing(3).align_y(iced::Alignment::Center))
            .width(Length::Fill)
            .padding(iced::Padding { top: 5.0, right: 6.0, bottom: 5.0, left: 8.0 })
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(if installed { Color { a: 0.08, ..p.accent } } else if selected { Color { a: 0.05, ..p.accent } } else { Color::TRANSPARENT })),
                border: Border { radius: 4.0.into(), ..Border::default() },
                ..Default::default()
            }),
        );
    }

    let body = column![
        header,
        Space::with_height(4),
        actions,
        Space::with_height(4),
        scrollable(container(rows).padding(iced::Padding { top: 0.0, right: 8.0, bottom: 8.0, left: 0.0 })).height(Length::Fill),
    ];
    shell("THEME STORE", win_id, p, body.into())
}

// ── Theme / Skin pickers (click the name in Appearance to browse all) ─────────

pub const PICKER_SIZE: iced::Size = iced::Size::new(470.0, 560.0);
pub const CONFIRM_DELETE_SIZE: iced::Size = iced::Size::new(340.0, 160.0);

/// A small box rendering a skin's rough look (radius + border), like the colors
/// dot but for skins.
fn skin_preview<'a>(name: &str, p: Palette, w: f32, h: f32) -> Element<'a, Message> {
    let s = crate::style::skin_style(name);
    let bc = s.border_color(&p);
    container(Space::new(Length::Fixed(w), Length::Fixed(h)))
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.tile)),
            border: Border { radius: (s.tile_radius * 0.5).into(), width: s.tile_border.max(s.widget_border).min(2.0), color: bc },
            ..Default::default()
        })
        .into()
}

pub fn picker_view<'a>(skins: bool, settings: &AppSettings, installed_open: bool, open_games: &std::collections::HashSet<String>, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let chip = |hex: &str| -> Element<'a, Message> {
        let c = crate::style::parse_hex(hex, p.muted);
        container(Space::new(Length::Fixed(14.0), Length::Fixed(14.0))).style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(c)),
            border: Border { radius: 3.0.into(), width: 1.0, color: Color { a: 0.3, ..p.muted } }, ..Default::default()
        }).into()
    };
    let card_style = move |sel: bool| move |_: &iced::Theme, status: button::Status| {
        let hover = matches!(status, button::Status::Hovered);
        button::Style {
            background: Some(iced::Background::Color(if sel { Color { a: 0.18, ..p.accent } } else { p.tile })),
            border: Border { radius: 8.0.into(), width: 1.0, color: if sel || hover { p.accent } else { Color { a: 0.5, ..p.muted } } },
            ..Default::default()
        }
    };

    // A single browsable list (one row per item), matching the C# layout.
    let mut col = column![].spacing(4);
    if skins {
        let active = settings.active_skin.clone();
        for name in crate::style::skin_names() {
            let sel = name == active;
            let nm = name.to_string();
            col = col.push(
                button(row![
                    skin_preview(&name, p, 40.0, 24.0),
                    Space::with_width(10),
                    text(name.clone()).size(12).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                    Space::with_width(Length::Fill),
                ].align_y(iced::Alignment::Center))
                .width(Length::Fill).padding(iced::Padding { top: 7.0, right: 10.0, bottom: 7.0, left: 8.0 }).style(card_style(sel))
                .on_press(Message::ApplySkin(nm))
            );
        }
    } else {
        let cur = crate::style::match_preset(settings);
        for (i, t) in crate::style::THEME_PRESETS.iter().enumerate() {
            let sel = cur == Some(i);
            col = col.push(
                button(row![
                    text(t.0.to_string()).size(12)
                        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                        .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                    Space::with_width(Length::Fill),
                    chip(t.1), chip(t.2), chip(t.3), chip(t.4), chip(t.5),
                ].spacing(4).align_y(iced::Alignment::Center))
                .width(Length::Fill).padding(iced::Padding { top: 7.0, right: 10.0, bottom: 7.0, left: 10.0 }).style(card_style(sel))
                .on_press(Message::ApplyThemePreset(i))
            );
        }
        // Installed game-pack themes get their own collapsible folder, with a
        // sub-folder per game so the list stays tidy. Each row has an X to remove.
        let installed = &settings.installed_themes;
        if !installed.is_empty() {
            let preset_match = cur.is_some();
            let red = Color::from_rgb8(0xCD, 0x5C, 0x5C);

            // One installed-theme row: apply on the left, X (remove) on the right.
            let theme_row = |i: usize, t: &fluid_core::settings::PresetSlot| -> Element<'a, Message> {
                let sel = !preset_match && crate::style::colors_match(settings, &t.bg, &t.tile, &t.accent, &t.text, &t.muted);
                let apply = button(row![
                    text(t.name.clone()).size(12)
                        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                        .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                    Space::with_width(Length::Fill),
                    chip(&t.bg), chip(&t.tile), chip(&t.accent), chip(&t.text), chip(&t.muted),
                ].spacing(4).align_y(iced::Alignment::Center))
                .width(Length::Fill).padding(iced::Padding { top: 7.0, right: 10.0, bottom: 7.0, left: 10.0 }).style(card_style(sel))
                .on_press(Message::ApplyInstalledTheme(i));
                let x = crate::style::with_tip(
                    button(text("\u{2715}").size(12).font(iced::Font::with_name("Segoe UI Symbol"))
                        .style(move |_| iced::widget::text::Style { color: Some(red) }))
                        .padding(iced::Padding { top: 6.0, right: 9.0, bottom: 6.0, left: 9.0 })
                        .style(move |_: &iced::Theme, status: button::Status| {
                            let hover = matches!(status, button::Status::Hovered);
                            button::Style {
                                background: Some(iced::Background::Color(if hover { Color { a: 0.18, ..red } } else { p.tile })),
                                border: Border { radius: 8.0.into(), width: 1.0, color: Color { a: 0.4, ..p.muted } },
                                ..Default::default()
                            }
                        })
                        .on_press(Message::RemoveInstalledTheme(i)),
                    "Remove this installed theme", p);
                row![apply, Space::with_width(4), x].align_y(iced::Alignment::Center).into()
            };

            // Top "Installed themes" folder.
            let chev = if installed_open { "\u{25BE}" } else { "\u{25B8}" };
            col = col.push(
                button(row![
                    text(chev.to_string()).size(11).font(iced::Font::with_name("Segoe UI Symbol"))
                        .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
                    Space::with_width(8),
                    text(format!("Installed themes ({})", installed.len())).size(12)
                        .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                        .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
                    Space::with_width(Length::Fill),
                ].align_y(iced::Alignment::Center))
                .width(Length::Fill).padding(iced::Padding { top: 8.0, right: 10.0, bottom: 8.0, left: 8.0 })
                .style(move |_: &iced::Theme, status: button::Status| {
                    let hover = matches!(status, button::Status::Hovered);
                    button::Style {
                        background: Some(iced::Background::Color(if hover { p.tile } else { Color { a: 0.5, ..p.tile } })),
                        border: Border { radius: 8.0.into(), width: 1.0, color: Color { a: 0.4, ..p.accent } },
                        ..Default::default()
                    }
                })
                .on_press(Message::ToggleThemePickerInstalled)
            );

            if installed_open {
                // Group installed themes by game (franchise), in pack order.
                let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
                for pack in crate::style::theme_packs() {
                    let idxs: Vec<usize> = installed.iter().enumerate()
                        .filter(|(_, t)| pack.themes.iter().any(|pt| pt.name == t.name))
                        .map(|(i, _)| i).collect();
                    if !idxs.is_empty() { groups.push((pack.franchise.clone(), idxs)); }
                }
                let other: Vec<usize> = installed.iter().enumerate()
                    .filter(|(_, t)| !crate::style::theme_packs().iter().any(|pk| pk.themes.iter().any(|pt| pt.name == t.name)))
                    .map(|(i, _)| i).collect();
                if !other.is_empty() { groups.push(("Other".to_string(), other)); }

                for (game, idxs) in groups {
                    let gopen = open_games.contains(&game);
                    let gchev = if gopen { "\u{25BE}" } else { "\u{25B8}" };
                    let g2 = game.clone();
                    col = col.push(
                        container(button(row![
                            text(gchev.to_string()).size(10).font(iced::Font::with_name("Segoe UI Symbol"))
                                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                            Space::with_width(8),
                            text(format!("{} ({})", game, idxs.len())).size(11)
                                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                            Space::with_width(Length::Fill),
                        ].align_y(iced::Alignment::Center))
                        .width(Length::Fill).padding(iced::Padding { top: 6.0, right: 10.0, bottom: 6.0, left: 8.0 })
                        .style(move |_: &iced::Theme, status: button::Status| {
                            let hover = matches!(status, button::Status::Hovered);
                            button::Style {
                                background: Some(iced::Background::Color(if hover { p.tile } else { Color::TRANSPARENT })),
                                border: Border { radius: 6.0.into(), width: 1.0, color: Color { a: 0.25, ..p.muted } },
                                ..Default::default()
                            }
                        })
                        .on_press(Message::ToggleThemePickerGame(g2)))
                        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 12.0 })
                    );
                    if gopen {
                        for i in idxs {
                            col = col.push(
                                container(theme_row(i, &installed[i]))
                                    .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 24.0 })
                            );
                        }
                    }
                }
            }
        }
    }
    let title = if skins { "CHOOSE A SKIN" } else { "CHOOSE A THEME" };
    // Stable id per list so the scroll position is kept while you click around
    // (and across re-opens within a session).
    let sid = iced::widget::scrollable::Id::new(if skins { "fluxid-skin-picker" } else { "fluxid-theme-picker" });
    let body = scrollable(container(col).padding(iced::Padding { top: 4.0, right: 8.0, bottom: 8.0, left: 0.0 }))
        .id(sid).height(Length::Fill);
    shell(title, win_id, p, body.into())
}

pub fn confirm_delete_view<'a>(slot: Option<u8>, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let n = slot.map(|s| s as u16 + 1).unwrap_or(0);
    let red = Color::from_rgb(0.90, 0.33, 0.24);
    let delete = button(text("Delete").size(12).style(move |_| iced::widget::text::Style { color: Some(Color::WHITE) }))
        .padding([6, 18])
        .style(move |_, _| button::Style { background: Some(iced::Background::Color(red)), border: Border { radius: 6.0.into(), ..Border::default() }, ..Default::default() })
        .on_press(Message::DeletePresetConfirmed);
    let body = column![
        text("Delete saved theme?").size(14)
            .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        text(format!("Saved theme slot {n} will be removed. This can't be undone."))
            .size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_height(Length::Fill),
        row![
            Space::with_width(Length::Fill),
            crate::style::inline_btn("Cancel", Message::ClosePopup(win_id), p),
            Space::with_width(8),
            delete,
        ].align_y(iced::Alignment::Center),
    ].spacing(8).height(Length::Fill);
    shell("CONFIRM", win_id, p, body.into())
}
