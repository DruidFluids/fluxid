//! Secondary windows: Tools, Alerts, Game Mode, Help, Utilities, the Window
//! Picker, and the widget right-click context menu.

use flux_core::settings::{AppSettings, RemoteDevice, SnapPosition, WarnMetric};
use iced::widget::{button, checkbox, column, container, mouse_area, pick_list, row, scrollable, slider, text, text_editor, text_input, toggler, Space};
use iced::{window, Border, Color, Element, Length};
use crate::style::Palette;
use crate::Message;

// ── Shared chrome ──────────────────────────────────────────────────────────

fn caption<'a>(_title: &str, win_id: window::Id, p: Palette) -> Element<'a, Message> {
    // Clean, minimal title bar: an accent band with just the centred brand mark
    // and a close button on the right, drawn in the theme bg colour for contrast.
    // The whole band drags the window.
    let on_bar = Color { a: 1.0, ..p.bg };
    let close = crate::style::with_tip(button(
        text("\u{2715}").size(13).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(on_bar) })
    ).padding([2, 8]).style(|_, _| button::Style { background: None, ..Default::default() })
        .on_press(Message::ClosePopup(win_id)), "Close", p);
    let brand = crate::style::brand_pulse(on_bar, 18.0);
    mouse_area(
        container(
            row![
                // Balances the close button so the brand mark stays truly centred.
                Space::with_width(Length::Fixed(34.0)),
                Space::with_width(Length::Fill),
                brand,
                Space::with_width(Length::Fill),
                close,
            ].align_y(iced::Alignment::Center)
        )
        .width(Length::Fill)
        .center_y(Length::Fixed(44.0)) // fixed-height band with the title vertically centred
        .padding(iced::Padding { top: 0.0, right: 6.0, bottom: 0.0, left: 8.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color { a: 1.0, ..p.accent })),
            border: Border { radius: iced::border::Radius { top_left: crate::style::win_radius(14.5), top_right: crate::style::win_radius(14.5), bottom_right: 0.0, bottom_left: 0.0 }, ..Border::default() },
            ..Default::default()
        })
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

fn toggle_row<'a>(label_text: &str, on: bool, msg: fn(bool) -> Message, p: Palette, tip: &str) -> Element<'a, Message> {
    crate::style::with_tip(
        row![
            toggler(on).size(14).on_toggle(msg).style(crate::style::toggler_style(p)),
            text(label_text.to_string()).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center),
        tip, p)
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

/// Shared "Save and Close" footer — a hairline divider above a right-aligned
/// primary button. Used by every Tools-tab popup so they're identical.
fn save_close_footer<'a>(win_id: window::Id, p: Palette) -> Element<'a, Message> {
    column![
        container(Space::new(Length::Fill, 1))
            .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(Color { a: 0.25, ..p.muted })), ..Default::default() }),
        container(row![Space::with_width(Length::Fill), primary_btn("Save and Close", Message::ClosePopup(win_id), p)].align_y(iced::Alignment::Center))
            .width(Length::Fill).padding(iced::Padding { top: 8.0, right: 0.0, bottom: 0.0, left: 0.0 }),
    ].into()
}

fn shell<'a>(title: &str, win_id: window::Id, p: Palette, body: Element<'a, Message>) -> Element<'a, Message> {
    // Match the Settings window's "Soft Premium" frame: a slightly darkened
    // window backdrop, a soft accent-tinted hairline border, and a large radius.
    let window_bg = Color { r: p.bg.r * 0.88, g: p.bg.g * 0.88, b: p.bg.b * 0.88, ..p.bg };
    let accent_border = crate::style::lerp(window_bg, p.accent, 0.45);
    // Caption is flush in the top-left corner; only the body is inset.
    // Thin accent hairline under the title bar, mirroring the Settings window.
    let hairline = container(Space::new(Length::Fill, Length::Fixed(1.0)))
        .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(Color { a: 0.30, ..p.accent })), ..Default::default() });
    container(column![
        caption(title, win_id, p),
        hairline,
        container(body).width(Length::Fill).height(Length::Fill)
            .padding(iced::Padding { top: 8.0, right: 16.0, bottom: 12.0, left: 16.0 }),
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
        crate::style::with_tip(item("Exit", Message::WidgetMenuExit), "Quit Flux", p),
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
// A clickable colour swatch matching the Appearance tab: the filled chip is a
// button that toggles an inline hex editor (shown to its left while `editing`).
fn color_swatch_field<'a, F>(hex: &str, editing: bool, toggle: Message, p: Palette, on_input: F) -> Element<'a, Message>
where
    F: Fn(String) -> Message + 'a,
{
    let c = crate::style::parse_hex(hex, p.muted);
    let swatch = button(Space::new(24, 16))
        .padding(0)
        .style(move |_, status: button::Status| {
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(c)),
                border: Border {
                    radius: 4.0.into(),
                    width: if editing || hover { 2.0 } else { 1.0 },
                    color: if editing || hover { p.text } else { Color { a: 0.45, ..p.muted } },
                },
                ..Default::default()
            }
        })
        .on_press(toggle);
    if editing {
        row![
            text_input("#AARRGGBB", hex).size(10).width(112)
                .font(iced::Font::with_name("Consolas"))
                .on_input(on_input)
                .style(crate::style::dark_input_style(p)),
            Space::with_width(6),
            swatch,
        ].align_y(iced::Alignment::Center).into()
    } else {
        swatch.into()
    }
}

// ── Tile Alerts (Warnings) ───────────────────────────────────────────────────

pub const ALERTS_SIZE: iced::Size = iced::Size::new(460.0, 560.0);

fn warn_card<'a>(settings: &AppSettings, kind: &str, p: Palette, editing: Option<&str>) -> Element<'a, Message> {
    let w = settings.warn(kind).cloned().unwrap_or_default();
    let enabled = w.enabled;
    let cool_key = format!("{kind}/cool");
    let hot_key = format!("{kind}/hot");
    let flash_key = format!("{kind}/flash");
    let edit_cool = editing == Some(cool_key.as_str());
    let edit_hot = editing == Some(hot_key.as_str());
    let edit_flash = editing == Some(flash_key.as_str());
    let k1 = kind.to_string();
    let k2 = kind.to_string();
    let k3 = kind.to_string();
    let k4 = kind.to_string();
    let k5 = kind.to_string();
    let k6 = kind.to_string();
    let k7 = kind.to_string();
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
    let grad_cool_row: Element<'a, Message> = if w.gradient_mode {
        row![
            text("Start color".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(bp.muted) }),
            Space::with_width(Length::Fill),
            crate::style::with_tip(color_swatch_field(&w.gradient_cool_color, edit_cool, Message::EditWarnColor(cool_key.clone()), bp, move |s| Message::SetWarnGradientCoolColor(k7.clone(), s)),
                "The starting color the unit shows when the value is comfortably below the threshold.", bp),
        ].spacing(6).align_y(iced::Alignment::Center).into()
    } else {
        Space::with_height(0).into()
    };
    let grad_color_row: Element<'a, Message> = if w.gradient_mode {
        row![
            text("Hot color".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(bp.muted) }),
            Space::with_width(Length::Fill),
            crate::style::with_tip(color_swatch_field(&w.gradient_color, edit_hot, Message::EditWarnColor(hot_key.clone()), bp, move |s| Message::SetWarnGradientColor(k6.clone(), s)),
                "The 'hot' color the unit text shifts toward as the metric approaches the threshold.", bp),
        ].spacing(6).align_y(iced::Alignment::Center).into()
    } else {
        Space::with_height(0).into()
    };
    let body = column![
        // Threshold
        row![
            label("Threshold", bp), Space::with_width(Length::Fill),
            crate::style::with_tip(text_input("", &format!("{}", w.threshold as i64)).size(11).width(70)
                .on_input(move |s| Message::SetWarnThresholdStr(k1.clone(), s))
                .style(crate::style::dark_input_style(bp)),
                "The value this alert triggers at (in the unit shown) — when the metric crosses it, the tile flashes or its gradient shifts.", bp),
            text(unit_label.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(bp.muted) }),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Metric
        row![
            label("Metric", bp), Space::with_width(Length::Fill),
            crate::style::with_tip(pick_list(metrics, Some(sel_metric), move |s: String| {
                let m = if s == "Load" { WarnMetric::Load } else { WarnMetric::Temperature };
                Message::SetWarnMetric(k2.clone(), m)
            }).text_size(11).width(140).style(crate::style::pick_list_style(bp)),
                "Whether this alert watches the tile's temperature or its load percentage.", bp),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Flash + flash colour swatch
        row![
            crate::style::with_tip(toggler(w.flash_enabled).size(14).on_toggle(move |on| Message::SetWarnFlash(k3.clone(), on)).style(crate::style::toggler_style(bp)), "Flash the tile background when the threshold is crossed.", bp),
            text("Flash".to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(bp.text) }),
            Space::with_width(Length::Fill),
            crate::style::with_tip(color_swatch_field(&w.flash_color, edit_flash, Message::EditWarnColor(flash_key.clone()), bp, move |s| Message::SetWarnFlashColor(k4.clone(), s)), "The colour the tile flashes when alerting.", bp),
        ].spacing(6).align_y(iced::Alignment::Center),
        // Gradient + (when on) the gradient hot-colour swatch
        row![
            crate::style::with_tip(toggler(w.gradient_mode).size(14).on_toggle(move |on| Message::SetWarnGradient(k5.clone(), on)).style(crate::style::toggler_style(bp)), "Instead of flashing, shift the unit colour from your start colour toward your hot colour as the value climbs.", bp),
            text("Gradient mode \u{2014} unit color shifts start \u{2192} hot".to_string()).size(10)
                .style(move |_| iced::widget::text::Style { color: Some(bp.text) }),
        ].spacing(6).align_y(iced::Alignment::Center),
        grad_cool_row,
        grad_color_row,
    ].spacing(8);

    let config: Element<'a, Message> = body.into();

    let ek = kind.to_string();
    container(column![
        row![
            crate::style::with_tip(toggler(enabled).size(16).on_toggle(move |on| Message::SetWarnEnabled(ek.clone(), on)).style(crate::style::toggler_style(p)), "Turn threshold alerts on or off for this tile.", p),
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

pub fn alerts_view<'a>(settings: &AppSettings, p: Palette, win_id: window::Id, editing: Option<&str>) -> Element<'a, Message> {
    let intro = text(
        "When the threshold is crossed, the tile background flashes. Gradient mode shifts the \
         unit color from your start color to your hot color as the value climbs. Click any \
         swatch to edit it. Temperature thresholds are in \u{00B0}C."
            .to_string()
    ).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) });

    let list = column![
        intro,
        Space::with_height(12),
        warn_card(settings, "CPU", p, editing),
        Space::with_height(8),
        warn_card(settings, "GPU", p, editing),
    ];

    let body = column![
        scrollable(list).height(Length::Fill).style(crate::style::scrollable_style(p)),
        save_close_footer(win_id, p),
    ];
    shell("Tile Alerts", win_id, p, body.into())
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
        let el: Element<'a, Message> = crate::style::with_tip(row![
            toggler(on).size(14).on_toggle(move |v| Message::ToggleGameModeTile(name.clone(), v)).style(crate::style::toggler_style(p)),
            text(display.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)),
            &format!("Show the {display} tile while Game Mode is active."), p);
        if i < 3 { row0.push(el); } else { row1.push(el); }
    }
    let tiles_grid = column![row(row0).spacing(8), row(row1).spacing(8)].spacing(6);

    let content = column![
        intro,
        Space::with_height(10),
        toggle_row("Enable Game Mode", s.game_mode_enabled, Message::SetGameModeEnabled, p, "Turn Game Mode on so the hotkey snaps the overlay to your chosen corner."),
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
        crate::style::with_tip(crate::settings_panel::marked_slider(0.1, 1.0, s.game_mode_opacity, 0.01, 0.7, p, Message::SetGameModeOpacity), "How see-through the widget is while Game Mode is active.", p),
        Space::with_height(6),
        label("Orientation", p),
        orient_pills,
        Space::with_height(8),
        toggle_row("Enable click-through while in Game Mode", s.game_mode_click_through, Message::SetGameModeClickThrough, p, "Let the mouse pass through the overlay while Game Mode is active."),
        Space::with_height(10),
        section_header("TILES WHEN ACTIVE", p),
        Space::with_height(4),
        tiles_grid,
    ].spacing(2);

    let body = column![
        scrollable(content).height(Length::Fill).style(crate::style::scrollable_style(p)),
        save_close_footer(win_id, p),
    ];
    shell("Game Mode", win_id, p, body.into())
}

// ── Help ─────────────────────────────────────────────────────────────────────

pub const HELP_SIZE: iced::Size = iced::Size::new(530.0, 640.0);

const HELP_SECTIONS: [(&str, &str); 19] = [
    ("The Widget", "The floating panel shows your live tiles. The gear (top-left) opens Settings; the close button (top-right) hides the widget to the system tray, where you can reopen or quit it. Drag the widget anywhere and it remembers its position."),
    ("The Tiles", "CPU and GPU show temperature, load percentage and clock speed; RAM shows usage and type/speed; Network shows live download and upload; Disk shows read and write speed; Clock shows the time and date. Choose which tiles appear, and exactly what each one shows, in Settings then Tiles."),
    ("Reorder and Layout", "Drag a tile row in Settings then Tiles to reorder them and the widget reflows live to match. Switch between Vertical and Horizontal layout in the Layout section."),
    ("Tiles Tab", "The grid at the top turns each tile on or off. Below it, every tile has its own row; expand it with the chevron to pick exactly which details that tile displays (model name, temperature, clock, VRAM, and so on)."),
    ("Behavior", "Always on top keeps Flux above other windows. Snap to edges docks it to screen edges as you drag; Snap to windows also docks to other windows' borders, with an adjustable snap distance. Run at Windows startup launches Flux when you sign in. Opacity sets transparency and Update interval sets how often the stats refresh."),
    ("Click-Through", "Click-through makes the widget ignore the mouse, so clicks pass through to whatever is behind it. Set the click-through hotkey to toggle it back, since you cannot click the widget while it is active."),
    ("Appearance: Skins", "A skin sets the widget's shape, borders, tile style and corner radius. Cycle through the 16 built-in skins with the arrows or open the skin browser."),
    ("Appearance: Colors", "A theme is a five-colour palette: Background, Tile, Accent, Text and Muted. Edit any swatch's hex value directly, cycle 100+ presets with the arrows, roll the dice for a random look, or undo the last change. Muted-text visibility tunes how bright the secondary text is."),
    ("Preset Themes and Slots", "Preset themes are one-click skin and colour combos. Save up to five favourites to the numbered slots and recall them instantly. Switch between dark and light from the colour presets."),
    ("Share and Theme Store", "Export your current look as a share code to send to others, or import a code to apply theirs. The Theme Store (folder icon) browses downloadable theme packs."),
    ("Fonts", "Choose the Primary (numbers), Secondary (names) and Indicator (units) fonts. Sync keeps all three the same and the dice can randomise them. The per-element size offsets nudge each group of text larger or smaller."),
    ("Size", "UI scale resizes the whole widget at once; Tile width and Tile height size the individual tiles; Round widget corners toggles the rounded frame."),
    ("Tile Alerts", "Set a per-tile threshold on temperature or load. Flash blinks the tile background in a colour you choose when the threshold is crossed; Gradient mode instead shifts the unit colour from cool blue toward your hot colour as the value climbs."),
    ("Game Mode", "Bind a hotkey to instantly snap a compact overlay into a corner of your primary monitor, even in fullscreen. Pick the corner, opacity, orientation, optional click-through, and which tiles show while it is active. Press the hotkey again to send it back."),
    ("Utilities", "A quick link to the Chris Titus Windows utility (it only opens the official site, nothing is bundled) plus a window-snap blocklist so chosen windows are never used as snap targets. Use Pick window to add one by clicking it."),
    ("Remote Monitoring", "Enable the TCP sensor feed to share this PC's stats over your LAN, protected by a handshake key that others connect with. Add other machines by their IP and key to watch them; each gets its own popout widget with independent layout and theming."),
    ("CPU Temperature", "Reading CPU die temperature needs a one-time, optional sensor driver (PawnIO), downloaded on demand from its official source and never bundled. Install or remove it from the CPU tile's info menu, and switch between Celsius and Fahrenheit there too."),
    ("Updates", "Off never checks; Manual checks only when you press Check now; Auto checks on launch and periodically and flags the gear when an update is waiting; Auto-install also downloads and installs them for you. Every download is verified against its published SHA-256 before it runs (see the Verification tab)."),
    ("License", "Flux is licensed, not sold, under a Personal Use License: you may view, build, run, and personally modify it, but you may not redistribute it or use it commercially. The full terms ship with the app as LICENSE.txt and are on GitHub (use the link below)."),
];

pub fn help_view<'a>(expanded: &std::collections::HashSet<usize>, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let mut col = column![].spacing(5);
    for (i, (title, desc)) in HELP_SECTIONS.iter().enumerate() {
        let open = expanded.contains(&i);
        // Collapsible header: chevron + title; click to toggle its description.
        let chevron = if open { "\u{25BE}" } else { "\u{25B8}" }; // ▾ open / ▸ closed
        let header = button(
            row![
                text(chevron.to_string()).size(10).font(crate::style::ICONS)
                    .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
                Space::with_width(8),
                text(title.to_string()).size(13)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                Space::with_width(Length::Fill),
            ].align_y(iced::Alignment::Center)
        )
        .width(Length::Fill)
        .padding(iced::Padding { top: 7.0, right: 10.0, bottom: 7.0, left: 8.0 })
        .style(move |_: &iced::Theme, status: button::Status| {
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(p.tile)),
                border: Border { radius: 7.0.into(), width: 1.0, color: if open || hover { Color { a: 0.55, ..p.accent } } else { Color { a: 0.0, ..p.muted } } },
                ..Default::default()
            }
        })
        .on_press(Message::ToggleHelpSection(i));
        col = col.push(header);
        if open {
            col = col.push(
                container(
                    text(desc.to_string()).size(11)
                        .style(move |_| iced::widget::text::Style { color: Some(Color { a: 0.92, ..p.text }) })
                ).padding(iced::Padding { top: 4.0, right: 10.0, bottom: 6.0, left: 26.0 })
            );
        }
    }
    // Footer: the project name + tagline, centered.
    col = col.push(
        container(
            column![
                text("Flux".to_string()).size(12)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
                text("your system vitals, always in flux".to_string()).size(10)
                    .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
                Space::with_height(6),
                button(text("View full license \u{2197}").size(10)
                    .style(move |_| iced::widget::text::Style { color: Some(p.accent) }))
                    .padding(0)
                    .style(|_: &iced::Theme, _: button::Status| button::Style { background: None, ..Default::default() })
                    .on_press(Message::OpenUrl("https://github.com/DruidFluids/Flux/blob/master/LICENSE".to_string())),
                text("Personal Use License \u{2014} \u{00A9} 2026 Matt Hakes").size(9)
                    .style(move |_| iced::widget::text::Style { color: Some(iced::Color { a: 0.7, ..p.muted }) }),
            ].spacing(1).align_x(iced::Alignment::Center)
        ).width(Length::Fill).center_x(Length::Fill).padding(iced::Padding { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 })
    );
    let body = scrollable(container(col).padding(iced::Padding { top: 4.0, right: 6.0, bottom: 8.0, left: 0.0 })).height(Length::Fill).style(crate::style::scrollable_style(p));
    shell("Help", win_id, p, body.into())
}

// ── "Updated to vX.Y.Z" notice (shown on first launch after an update) ───────

pub const UPDATED_SIZE: iced::Size = iced::Size::new(480.0, 540.0);

/// First-launch-after-update notice: a celebratory header plus the new release's
/// notes, rendered exactly like the Updates tab's changelog, and a Close button.
pub fn updated_view<'a>(version: &str, changelog: &str, reset_checked: bool, p: Palette, win_id: window::Id) -> Element<'a, Message> {
    let header = column![
        text(format!("Updated to v{version}")).size(17)
            .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
        text("Flux is now up to date. Here's what's new:").size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
    ].spacing(2);

    let notes: Element<'a, Message> = if changelog.trim().is_empty() {
        text("Release notes aren't available right now — check the Updates tab in Settings.")
            .size(11).style(move |_| iced::widget::text::Style { color: Some(Color { a: 0.9, ..p.text }) }).into()
    } else {
        crate::settings_panel::changelog_md(changelog, p)
    };
    // Notes box styled like the Updates card: a subtle tile-coloured panel.
    let notes_box = container(scrollable(container(notes).padding(iced::Padding { top: 2.0, right: 10.0, bottom: 2.0, left: 2.0 })).height(Length::Fill).style(crate::style::scrollable_style(p)))
        .width(Length::Fill).height(Length::Fill)
        .padding(10)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color { a: 0.5, ..p.tile })),
            border: Border { radius: 8.0.into(), width: 1.0, color: Color { a: 0.25, ..p.muted } },
            ..Default::default()
        });

    // Optional clean-slate reset, off by default. When ticked, closing the notice
    // wipes all saved settings (incl. window position) back to defaults. A single
    // built-in labelled checkbox so the box + text always render together.
    let reset_row = checkbox("Reset all saved settings (including window position) to defaults", reset_checked)
        .size(16).text_size(11).spacing(8)
        .on_toggle(Message::ToggleUpdateReset)
        .style(move |_t: &iced::Theme, status: checkbox::Status| {
            let on = matches!(status,
                checkbox::Status::Active { is_checked: true }
                | checkbox::Status::Hovered { is_checked: true }
                | checkbox::Status::Disabled { is_checked: true });
            checkbox::Style {
                background: iced::Background::Color(if on { p.accent } else { p.tile }),
                icon_color: Color::WHITE,
                border: Border { radius: 4.0.into(), width: 1.0, color: if on { p.accent } else { Color { a: 0.5, ..p.muted } } },
                text_color: Some(Color { a: 0.9, ..p.text }),
            }
        });

    let footer = column![
        container(Space::new(Length::Fill, 1))
            .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(Color { a: 0.25, ..p.muted })), ..Default::default() }),
        container(row![Space::with_width(Length::Fill), primary_btn("Close", Message::FinishUpdateNotice(win_id), p)].align_y(iced::Alignment::Center))
            .width(Length::Fill).padding(iced::Padding { top: 8.0, right: 0.0, bottom: 0.0, left: 0.0 }),
    ];

    let body = column![
        header,
        Space::with_height(10),
        notes_box,
        Space::with_height(10),
        reset_row,
        Space::with_height(8),
        footer,
    ].height(Length::Fill);
    shell("Updated", win_id, p, body.into())
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
    pawnio_installed: bool,
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

        // PawnIO is already installed, but the background service that feeds the
        // non-elevated widget isn't set up yet (e.g. after updating from a build
        // that ran elevated). One quick admin step enables it — no re-download.
        S::Primary if pawnio_installed => column![
            heading("Enable CPU temperature", p.accent),
            para("The PawnIO sensor driver is already installed. To read the temperature while Flux runs normally (no admin), Flux sets up a small background service that does the privileged read for it."),
            Space::with_height(4),
            muted("You'll see one Windows permission prompt to set up the service. After that it starts automatically on boot \u{2014} Flux itself never needs to run as administrator."),
            Space::with_height(Length::Fill),
            row![
                ibtn("Cancel", Message::ClosePopup(win_id)),
                Space::with_width(Length::Fill),
                primary_btn("Enable", Message::CpuDriverInstall, p),
            ].align_y(iced::Alignment::Center),
        ].spacing(6).height(Length::Fill).into(),

        S::Primary => column![
            heading("Turn on CPU temperature", p.accent),
            para("Reading the CPU's die temperature needs a small hardware-sensor driver. Flux uses PawnIO \u{2014} a free, open-source, Microsoft-signed driver built specifically for safe sensor access."),
            Space::with_height(4),
            muted("Flux never bundles the driver. It downloads the official signed installer, verifies its signature, then runs it. You'll see one Windows permission prompt (driver installs require it). Everything else on the widget works without this."),
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

    shell("CPU Temperature", win_id, p, body)
}

// ── Utilities (Tweaks) ───────────────────────────────────────────────────────

pub const UTILITIES_SIZE: iced::Size = iced::Size::new(460.0, 560.0);
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
        text("Disclaimer: Third-party tools linked here are not bundled with, vetted by, or endorsed by Flux. Flux only opens their official website \u{2014} review anything you download or run yourself. Use at your own risk.".to_string())
            .size(10).style(move |_| iced::widget::text::Style { color: Some(Color::from_rgb(1.0, 0.90, 0.84)) })
    )
    .width(Length::Fill).padding(iced::Padding { top: 8.0, right: 10.0, bottom: 8.0, left: 10.0 })
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba8(0xC0, 0x40, 0x18, 0.28))),
        border: Border { radius: 6.0.into(), width: 1.0, color: Color::from_rgba8(0xE0, 0x6A, 0x40, 0.55) },
        ..Default::default()
    });

    let list = scrollable(
        column![ct, blocklist_card, disclaimer].spacing(8)
            .padding(iced::Padding { top: 4.0, right: 6.0, bottom: 4.0, left: 0.0 })
    ).height(Length::Fill).style(crate::style::scrollable_style(p));
    let body = column![list, save_close_footer(win_id, p)].height(Length::Fill);
    shell("Utilities", win_id, p, body.into())
}

pub const REMOTE_SIZE: iced::Size = iced::Size::new(480.0, 640.0);

pub fn remote_view<'a>(
    mut remote: crate::settings_panel::RemoteView,
    settings: &AppSettings,
    p: Palette,
    win_id: window::Id,
) -> Element<'a, Message> {
    // Screenshot/QA mode (hidden --shot flag): never render the real handshake key
    // — it's a connection credential. Show a placeholder so captures are shareable.
    if std::env::args().any(|a| a == "--shot") {
        remote.handshake_key = "FM1:ExampleKeyOnly-DoNotUse-RegenerateInApp".to_string();
    }
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

    let feed_toggle = crate::style::with_tip(row![
        toggler(remote.feed_on).size(14).on_toggle(Message::SetTcpFeedEnabled).style(crate::style::toggler_style(p)),
        text("Enable TCP sensor feed (port 5199)").size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
    ].spacing(6).align_y(iced::Alignment::Center),
        "Share this PC's sensor data over your LAN so other machines can monitor it.", p);

    let key_row = row![
        crate::style::with_tip(text_input("", &remote.handshake_key).size(10).width(280).style(crate::style::dark_input_style(p)), "The handshake key other machines use to connect to this PC. Keep it private.", p),
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
        sh("Remote Devices", "Monitor other machines running Flux. Add them using their IP and handshake key."),
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
    col = col.push(crate::style::with_tip(row![
        toggler(settings.show_remote_status_dot).size(14)
            .on_toggle(Message::SetShowRemoteStatusDot).style(crate::style::toggler_style(p)),
        text("Show a green/red status dot on the widget's device tabs").size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
    ].spacing(6).align_y(iced::Alignment::Center),
        "Show a connection indicator (green = connected, red = offline) on each device tab.", p));

    let body = column![
        scrollable(col.padding(iced::Padding { top: 4.0, right: 6.0, bottom: 4.0, left: 0.0 })).height(Length::Fill).style(crate::style::scrollable_style(p)),
        save_close_footer(win_id, p),
    ].height(Length::Fill);
    shell("Remote Monitoring", win_id, p, body.into())
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
    let body = scrollable(container(col).padding(iced::Padding { top: 4.0, right: 6.0, bottom: 8.0, left: 0.0 })).height(Length::Fill).style(crate::style::scrollable_style(p));
    shell("Pick Window", win_id, p, body.into())
}

// ── Per-device Popout settings editor ────────────────────────────────────────

pub const POPOUT_CONFIG_SIZE: iced::Size = iced::Size::new(360.0, 540.0);

pub fn popout_config_view<'a>(dev: Option<&'a RemoteDevice>, p: Palette, win_id: window::Id, editing: Option<&'a str>) -> Element<'a, Message> {
    let dev = match dev {
        Some(d) => d,
        None => return shell("Popout", win_id, p,
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
        crate::style::with_tip(toggler(po.sync_colors).size(14).on_toggle(move |b| Message::PopoutSyncColors(sid.clone(), b)).style(crate::style::toggler_style(p)), "Match this popout to the main widget theme colours.", p),
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
            crate::style::with_tip(toggler(on).size(14).on_toggle(move |b| Message::PopoutTile(tid.clone(), name.to_string(), b)).style(crate::style::toggler_style(p)), "Show this tile on the remote popout.", p),
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
        let (ke, km, kt, kf, kfc, kg, kgc, kgcc) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
        let cool_key = format!("popout:{}:{}/cool", id, kind);
        let hot_key = format!("popout:{}:{}/hot", id, kind);
        let flash_key = format!("popout:{}:{}/flash", id, kind);
        let edit_cool = editing == Some(cool_key.as_str());
        let edit_hot = editing == Some(hot_key.as_str());
        let edit_flash = editing == Some(flash_key.as_str());
        let metrics = vec!["Temperature".to_string(), "Load".to_string()];
        let gradient_cool_row: Element<'a, Message> = if w.gradient_mode {
            row![
                text("Start color".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
                Space::with_width(Length::Fill),
                color_swatch_field(&w.gradient_cool_color, edit_cool, Message::EditWarnColor(cool_key.clone()), p, move |s| Message::PopoutWarnGradientCoolColor(kgcc.clone(), kind.to_string(), s)),
            ].spacing(6).align_y(iced::Alignment::Center).into()
        } else {
            Space::with_height(0).into()
        };
        let gradient_row: Element<'a, Message> = if w.gradient_mode {
            row![
                text("Hot color".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
                Space::with_width(Length::Fill),
                color_swatch_field(&w.gradient_color, edit_hot, Message::EditWarnColor(hot_key.clone()), p, move |s| Message::PopoutWarnGradientColor(kgc.clone(), kind.to_string(), s)),
            ].spacing(6).align_y(iced::Alignment::Center).into()
        } else {
            Space::with_height(0).into()
        };
        column![
            row![
                crate::style::with_tip(toggler(w.enabled).size(14).on_toggle(move |b| Message::PopoutWarnEnabled(ke.clone(), kind.to_string(), b)).style(crate::style::toggler_style(p)), "Enable threshold alerts for this tile on the popout.", p),
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
                crate::style::with_tip(toggler(w.flash_enabled).size(14).on_toggle(move |b| Message::PopoutWarnFlash(kf.clone(), kind.to_string(), b)).style(crate::style::toggler_style(p)), "Flash the tile when its threshold is crossed.", p),
                text("Flash".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
                Space::with_width(Length::Fill),
                color_swatch_field(&w.flash_color, edit_flash, Message::EditWarnColor(flash_key.clone()), p, move |s| Message::PopoutWarnFlashColor(kfc.clone(), kind.to_string(), s)),
            ].spacing(6).align_y(iced::Alignment::Center),
            row![
                crate::style::with_tip(toggler(w.gradient_mode).size(14).on_toggle(move |b| Message::PopoutWarnGradient(kg.clone(), kind.to_string(), b)).style(crate::style::toggler_style(p)), "Shift the unit colour from your start colour to your hot colour as the value climbs.", p),
                text("Gradient mode".to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            ].spacing(6).align_y(iced::Alignment::Center),
            gradient_cool_row,
            gradient_row,
        ].spacing(4).into()
    };
    col = col.push(warn_block("CPU"));
    col = col.push(warn_block("GPU"));

    let body = scrollable(
        container(col.width(Length::Fill)).width(Length::Fill)
            .padding(iced::Padding { top: 4.0, right: 8.0, bottom: 8.0, left: 0.0 })
    ).width(Length::Fill).height(Length::Fill).style(crate::style::scrollable_style(p));
    shell("Popout", win_id, p, body.into())
}

// ── Theme Store (bundled game theme packs) ───────────────────────────────────

pub const THEME_STORE_SIZE: iced::Size = iced::Size::new(580.0, 600.0);

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

    // Compact single-column list rows: name + color chips + count + Install all + Browse.
    let row_item = |pi: usize| -> Element<'a, Message> {
        let pack = &packs[pi];
        let installed_n = pack.themes.iter().filter(|t| is_installed(&t.name)).count();
        let all_in = installed_n == pack.themes.len();
        let mut sw = row![].spacing(3);
        for t in pack.themes.iter().take(6) { sw = sw.push(chip(&t.accent, p)); }
        let count_txt = if all_in {
            "Installed".to_string()
        } else if installed_n > 0 {
            format!("{}/{} Installed", installed_n, pack.themes.len())
        } else {
            format!("{} themes", pack.themes.len())
        };
        // Primary download CTA — bolder + a soft accent glow so it clearly reads as
        // the main action; "Browse" stays a quiet secondary button.
        let install_btn = button(
            text(if all_in { "Installed \u{2713}" } else { "Install all" }).size(11)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        )
            .width(Length::Fixed(98.0))
            .padding(iced::Padding { top: 5.0, right: 12.0, bottom: 5.0, left: 12.0 })
            .style(move |_: &iced::Theme, st: button::Status| {
                let hover = matches!(st, button::Status::Hovered);
                button::Style {
                    background: Some(iced::Background::Color(
                        if all_in { Color::TRANSPARENT } else if hover { Color { a: 0.9, ..p.accent } } else { p.accent }
                    )),
                    border: Border { radius: 7.0.into(), ..Border::default() },
                    text_color: if all_in { p.muted } else { Color::WHITE },
                    shadow: if all_in { iced::Shadow::default() } else {
                        iced::Shadow {
                            color: Color { a: if hover { 0.6 } else { 0.4 }, ..p.accent },
                            offset: iced::Vector::new(0.0, 1.0),
                            blur_radius: if hover { 9.0 } else { 5.0 },
                        }
                    },
                    ..Default::default()
                }
            })
            .on_press_maybe(if all_in { None } else { Some(Message::ThemeStoreTogglePack(pi, true)) });
        let browse_btn = button(text("Browse").size(10))
            .width(Length::Fixed(82.0))
            .padding(iced::Padding { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 })
            .style(move |_: &iced::Theme, st: button::Status| {
                let hover = matches!(st, button::Status::Hovered);
                button::Style {
                    background: Some(iced::Background::Color(if hover { Color { a: 0.12, ..p.accent } } else { Color::TRANSPARENT })),
                    border: Border { radius: 6.0.into(), width: 1.0, color: Color { a: 0.45, ..p.accent } },
                    text_color: p.accent,
                    ..Default::default()
                }
            })
            .on_press(Message::ThemeStoreOpenFranchise(pi));
        container(
            row![
                text(pack.franchise.clone()).size(12)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(p.text) })
                    .width(Length::Fixed(132.0)),
                sw,
                Space::with_width(Length::Fill),
                text(count_txt).size(9)
                    .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
                    .width(Length::Fixed(62.0)),
                install_btn,
                browse_btn,
            ].align_y(iced::Alignment::Center).spacing(6)
        )
        .width(Length::Fill)
        .padding(iced::Padding { top: 6.0, right: 10.0, bottom: 6.0, left: 10.0 })
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.tile)),
            border: Border { radius: 7.0.into(), ..Border::default() },
            ..Default::default()
        })
        .into()
    };

    let mut grid = column![].spacing(4);
    for pi in 0..packs.len() {
        grid = grid.push(row_item(pi));
    }

    let body = column![
        summary,
        hint,
        Space::with_height(6),
        scrollable(container(grid).padding(iced::Padding { top: 0.0, right: 8.0, bottom: 8.0, left: 0.0 })).height(Length::Fill).style(crate::style::scrollable_style(p)),
    ].spacing(2);
    shell("Theme Store", win_id, p, body.into())
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

    let back = button(text("\u{2039}  |  All Packs".to_string()).size(11)
        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(p.accent) }))
        .padding(iced::Padding { top: 4.0, right: 12.0, bottom: 4.0, left: 12.0 })
        .style(move |_: &iced::Theme, st: button::Status| {
            let hover = matches!(st, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(if hover { Color { a: 0.12, ..p.accent } } else { Color::TRANSPARENT })),
                border: Border { radius: 6.0.into(), width: 1.0, color: Color { a: 0.45, ..p.accent } },
                text_color: p.accent,
                ..Default::default()
            }
        })
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
                // Only badge the installed ones — "Available" is implied for the rest.
                if installed { status_pill("Installed".into(), true, p) } else { Space::with_width(0).into() },
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
        scrollable(container(rows).padding(iced::Padding { top: 0.0, right: 8.0, bottom: 8.0, left: 0.0 })).height(Length::Fill).style(crate::style::scrollable_style(p)),
    ];
    shell("Theme Store", win_id, p, body.into())
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
            let tip = format!("Apply the {name} skin (shape, borders, tile style, corner radius).");
            col = col.push(crate::style::with_tip(
                button(row![
                    skin_preview(&name, p, 40.0, 24.0),
                    Space::with_width(10),
                    text(name.clone()).size(12).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                    Space::with_width(Length::Fill),
                ].align_y(iced::Alignment::Center))
                .width(Length::Fill).padding(iced::Padding { top: 7.0, right: 10.0, bottom: 7.0, left: 8.0 }).style(card_style(sel))
                .on_press(Message::ApplySkin(nm)),
                &tip, p)
            );
        }
    } else {
        let cur = crate::style::match_preset(settings);
        for (i, t) in crate::style::THEME_PRESETS.iter().enumerate() {
            let sel = cur == Some(i);
            let tip = format!("Apply the '{}' color theme (its five-color palette).", t.0);
            col = col.push(crate::style::with_tip(
                button(row![
                    text(t.0.to_string()).size(12)
                        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                        .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                    Space::with_width(Length::Fill),
                    chip(t.1), chip(t.2), chip(t.3), chip(t.4), chip(t.5),
                ].spacing(4).align_y(iced::Alignment::Center))
                .width(Length::Fill).padding(iced::Padding { top: 7.0, right: 10.0, bottom: 7.0, left: 10.0 }).style(card_style(sel))
                .on_press(Message::ApplyThemePreset(i)),
                &tip, p)
            );
        }
        // Installed game-pack themes get their own collapsible folder, with a
        // sub-folder per game so the list stays tidy. Each row has an X to remove.
        let installed = &settings.installed_themes;
        if !installed.is_empty() {
            let preset_match = cur.is_some();
            let red = Color::from_rgb8(0xCD, 0x5C, 0x5C);

            // One installed-theme row: apply on the left, X (remove) on the right.
            let theme_row = |i: usize, t: &flux_core::settings::PresetSlot| -> Element<'a, Message> {
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
    let title = if skins { "Choose a Skin" } else { "Choose a Theme" };
    // Stable id per list so the scroll position is kept while you click around
    // (and across re-opens within a session).
    let sid = iced::widget::scrollable::Id::new(if skins { "Flux-skin-picker" } else { "Flux-theme-picker" });
    let list = scrollable(container(col).padding(iced::Padding { top: 4.0, right: 8.0, bottom: 8.0, left: 0.0 }))
        .id(sid).height(Length::Fill).style(crate::style::scrollable_style(p));
    // Bottom bar: a "Save and Close" action (selections apply live as you click).
    let body = column![list, save_close_footer(win_id, p)].height(Length::Fill);
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
    shell("Confirm", win_id, p, body.into())
}
