//! Secondary windows: Tools, Alerts, Game Mode, Help, Utilities, the Window
//! Picker, and the widget right-click context menu.

use fluid_core::settings::{AppSettings, SnapPosition, WarnMetric};
use iced::widget::{button, column, container, mouse_area, pick_list, row, scrollable, text, text_editor, text_input, toggler, Space};
use iced::{window, Border, Color, Element, Length};
use crate::style::Palette;
use crate::Message;

// ── Shared chrome ──────────────────────────────────────────────────────────

fn caption<'a>(title: &str, win_id: window::Id, p: Palette) -> Element<'a, Message> {
    let close = button(
        text("\u{2715}").size(15).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).padding([2, 8]).style(|_, _| button::Style { background: None, ..Default::default() })
        .on_press(Message::ClosePopup(win_id));

    mouse_area(
        container(row![
            text(title.to_string()).size(11)
                .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
            Space::with_width(Length::Fill),
            close,
        ].align_y(iced::Alignment::Center)).width(Length::Fill).height(32).padding([0, 6])
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
    container(column![caption(title, win_id, p), body])
        .width(Length::Fill).height(Length::Fill)
        .padding(iced::Padding { top: 0.0, right: 16.0, bottom: 12.0, left: 16.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.bg)),
            border: Border { radius: 10.0.into(), width: 1.0, color: Color { a: 0.4, ..p.muted } },
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
        item("Settings\u{2026}", Message::WidgetMenuSettings),
        divider,
        item("Exit", Message::WidgetMenuExit),
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

// ── Tools ───────────────────────────────────────────────────────────────────

pub const TOOLS_SIZE: iced::Size = iced::Size::new(380.0, 220.0);

fn tool_card<'a>(icon: &str, icon_color: Color, title: &str, subtitle: &str, msg: Message, p: Palette) -> Element<'a, Message> {
    let icon_box = container(
        text(icon.to_string()).size(20).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(icon_color) })
    ).width(42).height(42).center_x(42).center_y(42)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color { a: 0.12, ..icon_color })),
            border: Border { radius: 10.0.into(), ..Border::default() },
            ..Default::default()
        });

    let content = column![
        icon_box,
        Space::with_height(8),
        text(title.to_string()).size(11)
            .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        text(subtitle.to_string()).size(9)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
    ].align_x(iced::Alignment::Center).spacing(2);

    button(container(content).center_x(Length::Fill).padding([14, 8]))
        .width(Length::FillPortion(1))
        .style(move |_, status| button::Style {
            background: Some(iced::Background::Color(match status {
                button::Status::Hovered => Color { a: p.tile.a, ..p.tile },
                _ => Color { a: p.tile.a * 0.6, ..p.tile },
            })),
            border: Border { radius: 8.0.into(), width: 1.0, color: Color { a: 0.2, ..p.muted } },
            ..Default::default()
        })
        .on_press(msg).into()
}

pub fn tools_view<'a>(_settings: &AppSettings, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let cards = row![
        tool_card("\u{26A0}", Color::from_rgb8(0xE0, 0x60, 0x40), "Alerts", "Thresholds", Message::OpenAlerts, p),
        Space::with_width(10),
        tool_card("\u{1F3AE}", Color::from_rgb8(0x6A, 0x9F, 0xD8), "Game Mode", "Hotkey snap", Message::OpenGameMode, p),
        Space::with_width(10),
        tool_card("\u{1F527}", Color::from_rgb8(0x88, 0xAA, 0x55), "Utilities", "System tools", Message::OpenUtilities, p),
    ];
    let body = container(cards).padding(iced::Padding { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 });
    shell("TOOLS", win_id, p, body.into())
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
    let display = format!("{} Tile", kind);

    let metrics = vec!["Temperature".to_string(), "Load".to_string()];
    let sel_metric = match w.metric {
        WarnMetric::Load => "Load".to_string(),
        _ => "Temperature".to_string(),
    };
    let unit_label = if matches!(w.metric, WarnMetric::Load) { " %" } else { " \u{00B0}C" };

    let body = column![
        // Threshold
        row![
            label("Threshold", p), Space::with_width(Length::Fill),
            text_input("", &format!("{}", w.threshold as i64)).size(11).width(70)
                .on_input(move |s| Message::SetWarnThresholdStr(k1.clone(), s)),
            text(unit_label.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Metric
        row![
            label("Metric", p), Space::with_width(Length::Fill),
            pick_list(metrics, Some(sel_metric), move |s: String| {
                let m = if s == "Load" { WarnMetric::Load } else { WarnMetric::Temperature };
                Message::SetWarnMetric(k2.clone(), m)
            }).text_size(11).width(140),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Flash
        row![
            toggler(w.flash_enabled).size(14).on_toggle(move |on| Message::SetWarnFlash(k3.clone(), on)).style(crate::style::toggler_style(p)),
            text("Flash".to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            Space::with_width(Length::Fill),
            text_input("#FFFF3333", &w.flash_color).size(10).width(100)
                .font(iced::Font::with_name("Consolas"))
                .on_input(move |s| Message::SetWarnFlashColor(k4.clone(), s)),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Gradient
        row![
            toggler(w.gradient_mode).size(14).on_toggle(move |on| Message::SetWarnGradient(k5.clone(), on)).style(crate::style::toggler_style(p)),
            text("Gradient mode \u{2014} unit color shifts blue \u{2192} red by temperature".to_string()).size(10)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center),
    ].spacing(8);

    // Dim the config when disabled (mimics the C# DataTrigger opacity).
    let config: Element<'a, Message> = if enabled {
        body.into()
    } else {
        container(body).style(|_| iced::widget::container::Style::default()).into()
    };

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
        Space::with_height(8),
        warn_card(settings, "RAM", p),
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
    pill(format!("{} {}", glyph, label_text), active, Message::SetGameModePosition(pos), p)
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
            Space::with_width(6),
            pos_cell(s, SnapPosition::TopCenter, "\u{2191}", "Top C", p),
            Space::with_width(6),
            pos_cell(s, SnapPosition::TopRight, "\u{2197}", "Top R", p),
        ].spacing(0),
        row![
            pos_cell(s, SnapPosition::LeftCenter, "\u{2190}", "Left", p),
            Space::with_width(6),
            empty,
            Space::with_width(6),
            pos_cell(s, SnapPosition::RightCenter, "\u{2192}", "Right", p),
        ].spacing(0),
        row![
            pos_cell(s, SnapPosition::BottomLeft, "\u{2199}", "Bot L", p),
            Space::with_width(6),
            pos_cell(s, SnapPosition::BottomCenter, "\u{2193}", "Bot C", p),
            Space::with_width(6),
            pos_cell(s, SnapPosition::BottomRight, "\u{2198}", "Bot R", p),
        ].spacing(0),
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
    ("Behavior", "Always-on-top, click-through, snap-to-edges and snap-to-windows. Optionally launch fluidMonitor at Windows sign-in."),
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
    let body = scrollable(container(col).padding(iced::Padding { top: 4.0, right: 6.0, bottom: 8.0, left: 0.0 })).height(Length::Fill);
    shell("HELP", win_id, p, body.into())
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
        text("Disclaimer: Third-party tools linked here are not bundled with, vetted by, or endorsed by fluidMonitor. fluidMonitor only opens their official website \u{2014} review anything you download or run yourself. Use at your own risk.".to_string())
            .size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    )
    .width(Length::Fill).padding(iced::Padding { top: 8.0, right: 10.0, bottom: 8.0, left: 10.0 })
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba8(0xFF, 0x66, 0x00, 0.19))),
        border: Border { radius: 6.0.into(), ..Border::default() },
        ..Default::default()
    });

    let body = scrollable(
        column![ct, blocklist_card, disclaimer].spacing(8)
            .padding(iced::Padding { top: 4.0, right: 6.0, bottom: 4.0, left: 0.0 })
    ).height(Length::Fill);
    shell("UTILITIES", win_id, p, body.into())
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
