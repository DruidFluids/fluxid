//! The Settings window view: left column (tiles, behavior, network/disk) and
//! right column (appearance, fonts, remote, updates).

use flux_core::settings::{AppSettings, Orientation, TempUnit};
use iced::widget::{button, column, container, mouse_area, pick_list, row, scrollable, slider, stack, text, text_input, toggler, tooltip, Space};
use iced::widget::tooltip::Position as TipPos;
use iced::{Border, Element, Length};
use crate::style::Palette;
use crate::Message;

// C# TileTogglesGrid order: row0 CPU/GPU/RAM, row1 Network/Storage/Clock.
const TILES: [&str; 6] = ["CPU","GPU","RAM","Network","Storage","Clock"];
const TILE_INTERNAL: [&str; 6] = ["CPU","GPU","RAM","Network","Disk","Clock"];

const FONT_DEFAULT: &str = "(Default)";

/// Remote-monitoring UI state passed in from the App.
pub struct RemoteView {
    pub feed_on: bool,
    pub handshake_key: String,
    pub devices: Vec<flux_core::settings::RemoteDevice>,
    pub conn: std::collections::HashMap<String, bool>,
    pub add_open: bool,
    pub new_name: String,
    pub new_ip: String,
    pub new_key: String,
    pub test_status: String,
    pub test_ok: bool,
}

/// Update-section UI state passed in from the App.
pub struct UpdateView {
    pub current_version: String,
    pub mode: flux_core::settings::UpdateMode,
    pub last_checked: String,
    pub status: String,
    pub status_kind: u8, // 0 neutral, 1 good, 2 bad
    pub available: Option<(String, String)>, // version, changelog
    pub latest_changelog: Option<(String, String)>, // latest release notes (version, body)
    pub show_info: bool, // Updates card sub-tab: false = changelog, true = verification info
    pub progress: Option<f32>, // Some(fraction) while an update is downloading/verifying.
}

// Explainer shown in the Updates card's "Verification" sub-tab.
const VERIFICATION_MD: &str = "## How updates work\n\nFlux checks GitHub for the latest release. It never installs anything silently \u{2014} you choose Auto, Manual, or Off above. \"Check now\" looks for a newer version on demand.\n\n## Verified downloads\n\nEvery release publishes a SHA-256 checksum. When Flux downloads an installer it computes the file's hash and refuses to run it unless that hash exactly matches the published checksum \u{2014} so a tampered or corrupted download can't execute.\n\n## VirusTotal\n\nEach release is also scanned on VirusTotal; the detection result and a link are included in its release notes. You can re-check any download yourself in PowerShell:\n\nGet-FileHash .\\flux-setup.exe -Algorithm SHA256\n\nThe build is unsigned, so Windows SmartScreen shows a one-time prompt \u{2014} verifying the hash is how you confirm the file is the real one.";

// Render a GitHub-flavoured-markdown release body as styled elements so the raw
// `###` / `-` / `**` markers don't show. Line-based — handles headings, bullets
// (with nesting), blank-line spacing, and strips inline bold/code markers.
// Replace Markdown links `[text](url)` with just `text`, leaving other text intact.
fn strip_md_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if rest.as_bytes()[0] == b'[' {
            if let Some(mid) = rest.find("](") {
                if let Some(end) = rest[mid + 2..].find(')') {
                    out.push_str(&rest[1..mid]); // the link text
                    i += mid + 2 + end + 1;       // skip past the closing ')'
                    continue;
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub(crate) fn changelog_md<'a>(body: &str, p: Palette) -> Element<'a, Message> {
    // Strip inline bold/code markers and flatten Markdown links to their text
    // (this renderer can't make them clickable, so `[view the scan](url)` → `view the scan`).
    let clean = |s: &str| strip_md_links(&s.replace("**", "").replace('`', ""));
    let body_col = iced::Color { a: 0.9, ..p.text };
    let line_txt = move |s: String, size: u16, w: iced::font::Weight, c: iced::Color| -> Element<'a, Message> {
        text(s).size(size).width(Length::Fill)
            .font(iced::Font { weight: w, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(c) }).into()
    };
    let mut col = column![].spacing(3).width(Length::Fill);
    for raw in body.lines() {
        let lead = raw.len() - raw.trim_start().len();
        let t = raw.trim();
        if t.is_empty() {
            col = col.push(Space::with_height(4));
        } else if let Some(h) = t.strip_prefix("### ") {
            col = col.push(Space::with_height(2));
            col = col.push(line_txt(clean(h), 12, iced::font::Weight::Semibold, p.text));
        } else if let Some(h) = t.strip_prefix("## ") {
            col = col.push(Space::with_height(3));
            col = col.push(line_txt(clean(h), 13, iced::font::Weight::Bold, p.accent));
        } else if let Some(h) = t.strip_prefix("# ") {
            col = col.push(line_txt(clean(h), 14, iced::font::Weight::Bold, p.text));
        } else if t.starts_with("- ") || t.starts_with("* ") {
            col = col.push(row![
                Space::with_width(8.0 + (lead as f32) * 2.0),
                text("\u{2022}").size(10).style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
                Space::with_width(6),
                line_txt(clean(&t[2..]), 10, iced::font::Weight::Normal, body_col),
            ].align_y(iced::Alignment::Start));
        } else {
            col = col.push(line_txt(clean(t), 10, iced::font::Weight::Normal, body_col));
        }
    }
    col.into()
}

/// A click-to-capture hotkey field: shows the bound combo, "(click to set)"
/// when empty, or "(press keys…)" while armed. Pressing it emits `arm_msg`.
pub(crate) fn hotkey_field<'a>(combo: &str, capturing: bool, width: f32, arm_msg: Message, p: Palette) -> Element<'a, Message> {
    let label = if capturing {
        "(press keys\u{2026})".to_string()
    } else if combo.is_empty() {
        "(click to set)".to_string()
    } else {
        combo.to_string()
    };
    let dim = combo.is_empty() || capturing;
    button(text(label).size(11).style(move |_| iced::widget::text::Style { color: Some(if dim { p.muted } else { p.text }) }))
        .width(Length::Fixed(width))
        .padding(iced::Padding { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 })
        .style(move |_: &iced::Theme, _: button::Status| button::Style {
            background: Some(iced::Background::Color(crate::style::field_bg(p))),
            border: Border { radius: 4.0.into(), width: 1.0, color: if capturing { p.accent } else { iced::Color { a: 0.4, ..p.muted } } },
            ..Default::default()
        })
        .on_press(arm_msg).into()
}

// The settings window aggregates a lot of independent inputs; the cohesive
// groups are already bundled (RemoteView, UpdateView) and the rest are distinct
// scalars that a wrapper struct would only obscure.
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    settings: &AppSettings, p: Palette, win_id: iced::window::Id,
    theme_name: String, disks: Vec<String>, adapters: Vec<String>,
    fonts: Vec<String>,
    cpu_name: String, gpu_name: String,
    editing_color: Option<u8>,
    tab: usize,
    capturing_click_through: bool,
    appearance_status: String,
    update: UpdateView,
    cpu_driver_installed: bool,
    cpu_pawnio_installed: bool,
    tiles_open: Option<String>,
    preset_arming: Option<u8>,
    undo_accent: Option<iced::Color>,
    share_dialog: Option<(bool, String)>,
    copied_opacity: f32,
    tile_order: Vec<String>,
    // Active drag-reorder: (tile name being dragged, target drop-slot index).
    drag: Option<(String, usize)>,
) -> Element<'a, Message> {
    // ── Style helpers ──
    let sh = |label: &str, tip: &'static str| -> Element<'a, Message> {
        row![
            // Soft Premium: section headers are UPPERCASE in the accent color.
            text(label.to_uppercase()).size(12)
                .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
            Space::with_width(5),
            qmark(p, tip),
        ].align_y(iced::Alignment::Center).into()
    };
    let fl = |t: &str| -> Element<'a, Message> {
        text(t.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
            .into()
    };
    let vl = |t: String| -> Element<'a, Message> {
        text(t).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
            .into()
    };
    // C# PillBtn: 24px tall, radius 12, transparent + muted outline when off,
    // accent fill + white text when on; hover lights the border accent.
    let pill = |label_text: String, active: bool, msg: Message| -> Element<'a, Message> {
        button(text(label_text).size(11).font(iced::Font::with_name("Segoe UI Symbol")))
            .padding(iced::Padding { top: 3.0, right: 12.0, bottom: 3.0, left: 12.0 })
            .style(move |_: &iced::Theme, status: button::Status| {
                let hover = matches!(status, button::Status::Hovered);
                button::Style {
                    background: Some(iced::Background::Color(if active { p.accent } else { iced::Color::TRANSPARENT })),
                    text_color: if active { iced::Color::WHITE } else { p.muted },
                    border: Border { radius: 12.0.into(), width: 1.0, color: if active || hover { p.accent } else { p.muted } },
                    ..Default::default()
                }
            })
            .on_press(msg).into()
    };
    // C# layout / °C-°F segmented toggle: radius 4, tile fill (off) / accent (on).
    let seg = |label_text: String, active: bool, msg: Message| -> Element<'a, Message> {
        button(text(label_text).size(11).font(iced::Font::with_name("Segoe UI Symbol")))
            .padding(iced::Padding { top: 4.0, right: 14.0, bottom: 4.0, left: 14.0 })
            .style(move |_: &iced::Theme, _: button::Status| button::Style {
                background: Some(iced::Background::Color(if active { p.accent } else { p.tile })),
                text_color: if active { iced::Color::WHITE } else { p.text },
                border: Border { radius: 4.0.into(), ..Border::default() },
                ..Default::default()
            })
            .on_press(msg).into()
    };
    // C# InlineBtn: tile fill, 1px muted border, radius 6; hover accents text+border.
    let cycle_btn = |label_text: String, msg: Message| -> Element<'a, Message> {
        button(
            container(text(label_text).size(11)).center_x(Length::Fill)
        )
        .width(Length::Fill)
        .padding(iced::Padding { top: 4.0, right: 10.0, bottom: 4.0, left: 10.0 })
        .style(move |_: &iced::Theme, status: button::Status| {
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(p.tile)),
                text_color: if hover { p.accent } else { p.text },
                border: Border { radius: 6.0.into(), width: 1.0, color: if hover { p.accent } else { p.muted } },
                ..Default::default()
            }
        })
        .on_press(msg).into()
    };
    // Paired slider with C# default-value marker + thin accent/muted track.
    let pslider = |label_text: &str, value_text: String, min: f32, max: f32, val: f32, default: f32, step: f32, msg: fn(f32)->Message, tip: &'static str| -> Element<'a, Message> {
        crate::style::with_tip(
            column![
                row![fl(label_text), Space::with_width(Length::Fill), vl(value_text)],
                marked_slider(min, max, val, step, default, p, msg),
            ].spacing(2).width(Length::FillPortion(1)),
            tip, p)
    };

    // ════════════════════════════════════════════════════════════
    //  LEFT COLUMN  (matches C# SettingsWindow.xaml left panel)
    // ════════════════════════════════════════════════════════════

    // ── Tiles: 3x2 toggle grid ──
    let mut t_r0 = Vec::<Element<'a, Message>>::new();
    let mut t_r1 = Vec::<Element<'a, Message>>::new();
    for (i, (display, internal)) in TILES.iter().zip(TILE_INTERNAL.iter()).enumerate() {
        let visible = settings.visible_tiles.iter().any(|v| v == internal);
        let name = internal.to_string();
        let t: Element<'a, Message> = crate::style::with_tip(
            row![
                toggler(visible).size(14).on_toggle(move |on| Message::ToggleTile(name.clone(), on)).style(crate::style::toggler_style(p)),
                text(display.to_string()).size(11)
                    .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)),
            &format!("Show or hide the {display} tile on the widget."), p);
        if i < 3 { t_r0.push(t); } else { t_r1.push(t); }
    }
    let tiles_grid = column![row(t_r0).spacing(4), row(t_r1).spacing(4)].spacing(6);

    let fahrenheit = settings.temperature_unit == TempUnit::Fahrenheit;
    // Optional CPU-temp sensor driver (PawnIO): an "i" badge opens the manage
    // dialog, and an Active/Inactive chip reflects whether the driver is present.
    let info_badge: Element<'a, Message> = button(
        container(text("i").size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
            .center_x(Length::Fixed(15.0)).center_y(Length::Fixed(15.0))
    )
    .padding(0)
    .style(move |_: &iced::Theme, status: button::Status| {
        let hover = matches!(status, button::Status::Hovered);
        button::Style {
            background: None,
            text_color: if hover { p.accent } else { p.muted },
            border: Border { radius: 8.0.into(), width: 1.0, color: if hover { p.accent } else { iced::Color { a: 0.6, ..p.muted } } },
            ..Default::default()
        }
    })
    .on_press(Message::OpenCpuDriver).into();
    // Driver status: green when active, red when inactive (used on the Tiles
    // row and the Sensors-tab section).
    let driver_green = iced::Color::from_rgb(0.30, 0.78, 0.45);
    let driver_red = iced::Color::from_rgb(0.86, 0.30, 0.25);
    let driver_amber = iced::Color::from_rgb(0.95, 0.66, 0.23);
    // Three states: fully working (green), driver present but service not set up
    // (amber — one click away), or not installed (red).
    let (status_color, status_label) = if cpu_driver_installed {
        (driver_green, "Active")
    } else if cpu_pawnio_installed {
        (driver_amber, "Setup needed")
    } else {
        (driver_red, "Inactive")
    };
    let driver_status: Element<'a, Message> = button(
        text(status_label).size(11).style(move |_| iced::widget::text::Style { color: Some(status_color) })
    )
    .padding(iced::Padding { top: 1.0, right: 6.0, bottom: 1.0, left: 6.0 })
    .style(move |_: &iced::Theme, status: button::Status| {
        let hover = matches!(status, button::Status::Hovered);
        button::Style {
            background: if hover { Some(iced::Background::Color(p.tile)) } else { None },
            border: Border { radius: 4.0.into(), width: if hover { 1.0 } else { 0.0 }, color: iced::Color { a: 0.6, ..p.muted } },
            ..Default::default()
        }
    })
    .on_press(Message::OpenCpuDriver).into();
    let temp_row: Element<'a, Message> = row![
        text("CPU temperature").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_width(6),
        tooltip(info_badge,
            tip_box("Optional: install a free, signed sensor driver (PawnIO) so CPU die temperature can be read. Click for details.", p),
            TipPos::FollowCursor),
        Space::with_width(2),
        driver_status,
        // °C/°F moved out to the master Temperature-unit bar atop the Tiles tab —
        // the unit is global, so it no longer lives in the CPU tile's options.
    ].align_y(iced::Alignment::Center).spacing(0).into();

    // ── Tile Labels: CPU/GPU with Auto/Custom pills ──
    let cpu_auto = settings.cpu_custom_name.is_empty();
    let gpu_auto = settings.gpu_custom_name.is_empty();
    // C# LineInput: underline-only text field (transparent, bottom border),
    // dimmed while "Auto" is selected.
    let name_input = |value: &str, placeholder: &str, auto: bool, on_input: fn(String) -> Message| -> Element<'a, Message> {
        let line = if auto { iced::Color { a: 0.35, ..p.muted } } else { p.muted };
        column![
            text_input(placeholder, value).size(11)
                .padding(iced::Padding { top: 2.0, right: 2.0, bottom: 3.0, left: 2.0 })
                .on_input(on_input)
                .style(move |_t, _s| iced::widget::text_input::Style {
                    background: iced::Background::Color(iced::Color::TRANSPARENT),
                    border: Border { radius: 0.0.into(), width: 0.0, color: iced::Color::TRANSPARENT },
                    icon: p.muted,
                    placeholder: iced::Color { a: 0.5, ..p.muted },
                    value: if auto { iced::Color { a: 0.5, ..p.text } } else { p.text },
                    selection: iced::Color { a: 0.3, ..p.accent },
                }),
            container(Space::new(Length::Fill, 1))
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(line)),
                    ..Default::default()
                }),
        ].spacing(0).width(Length::Fill).into()
    };
    let label_cell = |t: &str| -> Element<'a, Message> {
        text(t.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
            .width(28).into()
    };
    let tile_labels = column![
        row![
            label_cell("CPU"),
            crate::style::with_tip(name_input(&settings.cpu_custom_name, &cpu_name, cpu_auto, Message::SetCpuName), "Type your own label for the CPU tile instead of the detected name.", p),
            Space::with_width(8),
            crate::style::with_tip(pill("Auto".into(), cpu_auto, Message::SetCpuName(String::new())), "Show the auto-detected CPU name.", p),
            Space::with_width(4),
            crate::style::with_tip(pill("Custom".into(), !cpu_auto, Message::Noop), "Use the custom CPU name you typed.", p),
        ].spacing(0).align_y(iced::Alignment::Center),
        row![
            label_cell("GPU"),
            crate::style::with_tip(name_input(&settings.gpu_custom_name, &gpu_name, gpu_auto, Message::SetGpuName), "Type your own label for the GPU tile instead of the detected name.", p),
            Space::with_width(8),
            crate::style::with_tip(pill("Auto".into(), gpu_auto, Message::SetGpuName(String::new())), "Show the auto-detected GPU name.", p),
            Space::with_width(4),
            crate::style::with_tip(pill("Custom".into(), !gpu_auto, Message::Noop), "Use the custom GPU name you typed.", p),
        ].spacing(0).align_y(iced::Alignment::Center),
    ].spacing(8);

    // ── Layout ──
    let layout_pills = row![
        crate::style::with_tip(seg("Horizontal".into(), settings.orientation == Orientation::Horizontal, Message::SetOrientation(Orientation::Horizontal)), "Lay the tiles out side by side (wide).", p),
        crate::style::with_tip(seg("Vertical".into(), settings.orientation == Orientation::Vertical, Message::SetOrientation(Orientation::Vertical)), "Stack the tiles top to bottom (tall).", p),
    ].spacing(4);

    // ── Behavior: togglers in pairs + hotkey + paired sliders ──
    let sw = |label_text: &str, on: bool, msg: fn(bool)->Message| -> Element<'a, Message> {
        row![
            toggler(on).size(14).on_toggle(msg).style(crate::style::toggler_style(p)),
            text(label_text.to_string()).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)).into()
    };
    let sw_tt = |label_text: &str, on: bool, msg: fn(bool)->Message, tip: &'static str| -> Element<'a, Message> {
        tooltip(
            row![
                toggler(on).size(14).on_toggle(msg).style(crate::style::toggler_style(p)),
                text(label_text.to_string()).size(11)
                    .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)),
            tip_box(tip, p), TipPos::FollowCursor,
        ).into()
    };

    // "Snap to windows" is a sub-option of "Snap to edges" — only shown while
    // edge-snap is on (enabling edge-snap turns it on by default). When edge-snap
    // is off the startup toggle takes that slot (renamed "Run at startup").
    let startup_tip = "Launch the widget when you sign in to Windows. Uses your user account only \u{2014} no admin rights needed.";
    let click_tip = "Make the widget click-through \u{2014} the mouse passes through it to whatever's behind. Toggle it back with the click-through hotkey below.";
    let always_tip = "Keep the widget pinned above all other windows so it's never hidden behind them.";
    let snap_edges_tip = "Dock the widget flush against screen edges as you drag it close.";
    let _ = &sw; // (toggles below all carry tooltips)
    let snap_block: Element<'a, Message> = if settings.snap_to_edges {
        column![
            row![
                sw_tt("Snap to edges", settings.snap_to_edges, Message::SetSnap, snap_edges_tip),
                sw_tt("Snap to windows", settings.snap_to_windows, Message::SetSnapWindows,
                    "When snapping is on, the widget also docks to the outer edges of other windows."),
            ].spacing(8),
            column![
                row![fl("Snap distance"), Space::with_width(Length::Fill), vl(format!("{:.0}px", settings.snap_distance))],
                crate::style::with_tip(marked_slider(0.0, 50.0, settings.snap_distance, 1.0, 20.0, p, Message::SetSnapDistance), "How close (in pixels) the widget must be to an edge or window before it snaps.", p),
            ].spacing(2),
            sw_tt("Click-through", settings.click_through, Message::SetClickThrough, click_tip),
        ].spacing(4).into()
    } else {
        row![
            sw_tt("Snap to edges", settings.snap_to_edges, Message::SetSnap, snap_edges_tip),
            sw_tt("Click-through", settings.click_through, Message::SetClickThrough, click_tip),
        ].spacing(8).into()
    };

    let behavior = column![
        row![sw_tt("Always on top", settings.always_on_top, Message::SetAlwaysOnTop, always_tip), sw_tt("Run at Windows startup", settings.run_at_startup, Message::SetRunAtStartup, startup_tip)].spacing(8),
        snap_block,
        Space::with_height(4),
        fl("Click-through hotkey"),
        row![
            hotkey_field(&settings.click_through_hotkey, capturing_click_through, 150.0,
                Message::ArmHotkey(crate::hotkeys::HotkeyTarget::ClickThrough), p),
            crate::style::with_tip(button(text("\u{2715}").size(10).font(crate::style::ICONS).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding([2, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() })
                .on_press(Message::ClearHotkey(crate::hotkeys::HotkeyTarget::ClickThrough)), "Clear the hotkey", p),
        ].spacing(6).align_y(iced::Alignment::Center),
        Space::with_height(4),
        // Paired sliders: Opacity + Update interval
        row![
            pslider("Opacity", format!("{:.0}%", settings.widget_opacity * 100.0), 0.3, 1.0, settings.widget_opacity, 0.9, 0.01, Message::SetOpacity, "How see-through the widget is (lower = more transparent)."),
            Space::with_width(8),
            pslider("Update interval", format!("{} ms", settings.update_interval_ms), 250.0, 5000.0, settings.update_interval_ms as f32, 1500.0, 250.0, Message::SetInterval, "How often the stats refresh, in milliseconds."),
        ],
    ].spacing(4);

    // ── Size: sliders that change tile/widget dimensions (live in Appearance) ──
    let sizing = column![
        row![
            pslider("UI scale", format!("{:.2}x", settings.ui_scale), 0.75, 1.5, settings.ui_scale, 1.0, 0.01, Message::SetUiScale, "Scale the whole widget and its text up or down."),
            Space::with_width(8),
            pslider("Tile width", format!("{:.0}px", settings.tile_width), 110.0, 200.0, settings.tile_width, 130.0, 5.0, Message::SetTileWidth, "Width of each tile, in pixels."),
        ],
        column![
            row![fl("Tile height"), Space::with_width(Length::Fill), vl(format!("{:.0}px", settings.tile_height))],
            crate::style::with_tip(marked_slider(80.0, 150.0, settings.tile_height, 2.0, 110.0, p, Message::SetTileHeight), "Height of each tile, in pixels.", p),
        ].spacing(2),
        sw_tt("Round widget corners", settings.round_corners, Message::SetRoundCorners,
            "Round the outer corners of the widget window (Windows 11)."),
    ].spacing(4);

    // ── Network: paired grid ──
    let traffic_label = format!("\u{2193} {} \u{2191}", settings.network_traffic_indicator);
    let adapter_value = if settings.network_adapter_name.is_empty() { "All adapters".to_string() } else { settings.network_adapter_name.clone() };
    let selected_adapter = if adapters.contains(&adapter_value) { Some(adapter_value) } else { Some("All adapters".to_string()) };
    let network = column![
        row![
            column![fl("Traffic indicator"), tooltip(cycle_btn(traffic_label, Message::TrafficCycle), tip_box("Click to cycle: Off > Blink > Fade > Glow", p), TipPos::FollowCursor)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("Arrow position", format!("{:.0}px", settings.network_arrow_spacing.min(8.0)), 0.0, 8.0, settings.network_arrow_spacing.min(8.0), 5.0, 1.0, Message::SetArrowSpacing, "Shift the Network up/down arrows left or right."),
        ],
        Space::with_height(4),
        row![
            column![fl("Monitor adapter"), crate::style::with_tip(pick_list(adapters, selected_adapter, Message::SetAdapter).text_size(11).width(Length::Fill).style(crate::style::pick_list_style(p)), "Choose which network adapter the Network tile measures.", p)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("Arrow size", signed(settings.arrow_font_offset), -5.0, 10.0, settings.arrow_font_offset as f32, 0.0, 1.0, Message::SetArrowFontOffset, "Make the Network direction arrows larger or smaller."),
        ],
    ].spacing(2);

    // ── Disk: paired grid ──
    let disk_label_text = format!("Show: {}", settings.disk_label_style);
    let selected_disk = if disks.contains(&settings.selected_disk_mount) { Some(settings.selected_disk_mount.clone()) } else { disks.first().cloned() };
    let disk = column![
        row![
            column![fl("Tile label"), tooltip(cycle_btn(disk_label_text, Message::DiskLabelCycle), tip_box("Click to cycle: Drive letter, Model, Both", p), TipPos::FollowCursor)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("R: / W: position", format!("{:.0}px", settings.disk_label_spacing.min(14.0)), 0.0, 14.0, settings.disk_label_spacing.min(14.0), 8.0, 1.0, Message::SetDiskLabelSpacing, "Shift the Disk R: / W: labels left or right."),
        ],
        Space::with_height(4),
        row![
            column![fl("Monitor disk"), crate::style::with_tip(pick_list(disks, selected_disk, Message::SetDisk).text_size(11).width(Length::Fill).style(crate::style::pick_list_style(p)), "Choose which disk the Disk tile measures.", p)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("R: / W: size", signed(settings.disk_label_font_offset), -5.0, 10.0, settings.disk_label_font_offset as f32, 0.0, 1.0, Message::SetDiskLabelFontOffset, "Make the Disk R: / W: labels larger or smaller."),
        ],
    ].spacing(2);

    // ── Tiles tab: one expandable section per tile (accordion). Each holds the
    // tile's visibility, label, per-field toggles, and its own options, so all
    // of a tile's settings live in one place. ──
    let _ = (&tiles_grid, &tile_labels); // superseded by the per-tile sections
    let field_tog = |label: &str, on: bool, key: &'static str, tip: &'static str| -> Element<'a, Message> {
        crate::style::with_tip(
            row![
                toggler(on).size(13).on_toggle(move |v| Message::SetTileField(key.to_string(), v)).style(crate::style::toggler_style(p)),
                text(label.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            ].spacing(6).align_y(iced::Alignment::Center),
            tip, p)
    };
    let open_is = |n: &str| tiles_open.as_deref() == Some(n);

    // Optional CPU sensor driver (PawnIO) — lives inside the CPU tile section.
    let driver_status_chip = container(
        text(status_label).size(11)
            .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(status_color) })
    )
    .padding(iced::Padding { top: 2.0, right: 8.0, bottom: 2.0, left: 8.0 })
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color { a: 0.14, ..status_color })),
        border: Border { radius: 5.0.into(), width: 1.0, color: iced::Color { a: 0.5, ..status_color } },
        ..Default::default()
    });
    let driver_btn_label = if cpu_driver_installed {
        "Manage / Remove"
    } else if cpu_pawnio_installed {
        "Enable CPU temp"
    } else {
        "Install driver"
    };
    let driver_desc = if cpu_pawnio_installed && !cpu_driver_installed {
        "Driver installed \u{2014} enable the background service (one quick admin step) so Flux can read the temperature while running normally."
    } else {
        "Reads the CPU's die temperature directly. Flux downloads the official signed PawnIO driver, verifies its signature, and installs it on request \u{2014} the rest of the widget works without it."
    };
    // When the driver's installed but the service isn't set up, the status IS the
    // action: one amber "Setup needed" button that starts the enable flow — no
    // separate chip + button.
    let needs_service = cpu_pawnio_installed && !cpu_driver_installed;
    let driver_action: Element<'a, Message> = if needs_service {
        crate::style::with_tip(
            button(
                text("Setup needed \u{2014} Enable CPU temperature").size(11)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(driver_amber) }),
            )
            .padding(iced::Padding { top: 5.0, right: 12.0, bottom: 5.0, left: 12.0 })
            .style(move |_: &iced::Theme, status: button::Status| {
                let hover = matches!(status, button::Status::Hovered);
                button::Style {
                    background: Some(iced::Background::Color(iced::Color { a: if hover { 0.24 } else { 0.14 }, ..driver_amber })),
                    border: Border { radius: 6.0.into(), width: 1.0, color: iced::Color { a: 0.6, ..driver_amber } },
                    ..Default::default()
                }
            })
            .on_press(Message::OpenCpuDriver),
            "Set up the CPU-temperature service (one quick admin step).", p)
    } else {
        row![
            driver_status_chip,
            Space::with_width(Length::Fill),
            crate::style::inline_btn_tip(driver_btn_label, Message::OpenCpuDriver, "Open the CPU sensor driver dialog (install, verify, or remove)", p),
        ].align_y(iced::Alignment::Center).into()
    };
    let cpu_driver = column![
        text("Temperature driver (optional)").size(11)
            .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        text(driver_desc.to_string())
            .size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_height(4),
        driver_action,
    ].spacing(2);

    // Bodies (built unconditionally so temp_row/network/disk are consumed once).
    let cpu_body: Element<'a, Message> = column![
        row![
            name_input(&settings.cpu_custom_name, &cpu_name, cpu_auto, Message::SetCpuName),
            Space::with_width(8),
            pill("Auto".into(), cpu_auto, Message::SetCpuName(String::new())),
            Space::with_width(4),
            pill("Custom".into(), !cpu_auto, Message::Noop),
        ].spacing(0).align_y(iced::Alignment::Center),
        temp_row,
        row![field_tog("Model", settings.cpu_show_name, "cpu_name", "Show the CPU's model name on the tile."), field_tog("Temperature", settings.cpu_show_temp, "cpu_temp", "Show CPU temperature (needs the optional sensor driver below)."), field_tog("Clock", settings.cpu_show_clock, "cpu_clock", "Show the CPU's live clock speed in MHz.")].spacing(10),
        Space::with_height(2),
        cpu_driver,
    ].spacing(6).into();
    let gpu_body: Element<'a, Message> = column![
        row![
            name_input(&settings.gpu_custom_name, &gpu_name, gpu_auto, Message::SetGpuName),
            Space::with_width(8),
            pill("Auto".into(), gpu_auto, Message::SetGpuName(String::new())),
            Space::with_width(4),
            pill("Custom".into(), !gpu_auto, Message::Noop),
        ].spacing(0).align_y(iced::Alignment::Center),
        row![field_tog("Model", settings.gpu_show_name, "gpu_name", "Show the GPU's model name on the tile."), field_tog("Temperature", settings.gpu_show_temp, "gpu_temp", "Show the GPU's temperature."), field_tog("Clock", settings.gpu_show_clock, "gpu_clock", "Show the GPU's live core clock in MHz."), field_tog("VRAM", settings.gpu_show_vram, "gpu_vram", "Show GPU video-memory used / total.")].spacing(10),
    ].spacing(6).into();
    let ram_body: Element<'a, Message> =
        row![field_tog("Speed / type", settings.ram_show_speed, "ram_speed", "Show the RAM's type and speed (e.g. DDR5-6000)."), field_tog("Usage detail", settings.ram_show_details, "ram_details", "Show used / total amount under the percentage.")].spacing(10).into();
    let net_body: Element<'a, Message> = column![
        row![field_tog("Download", settings.net_show_down, "net_down", "Show the download (incoming) traffic line."), field_tog("Upload", settings.net_show_up, "net_up", "Show the upload (outgoing) traffic line.")].spacing(10),
        row![field_tog("Upload on top", settings.net_upload_first, "net_swap", "Put the upload line above the download line on the tile.")].spacing(10),
        network,
    ].spacing(6).into();
    let disk_body: Element<'a, Message> = column![
        row![field_tog("Read", settings.disk_show_read, "disk_read", "Show the disk read-speed line."), field_tog("Write", settings.disk_show_write, "disk_write", "Show the disk write-speed line.")].spacing(10),
        disk,
    ].spacing(6).into();
    let clock_body: Element<'a, Message> = row![field_tog("Date", settings.clock_show_date, "clock_date", "Show the date beneath the time on the Clock tile.")].into();
    let mut bodies = [Some(clock_body), Some(cpu_body), Some(gpu_body), Some(ram_body), Some(net_body), Some(disk_body)];

    // A small "Shown / Hidden" chip that toggles the tile's visibility (right
    // side of each list row) — replaces the old left-edge switch.
    let vis_chip = |shown: bool, internal: String| -> Element<'a, Message> {
        // A self-explanatory LED toggle: on = tile shown, off = hidden.
        container(
            toggler(shown).size(15)
                .on_toggle(move |v| Message::ToggleTile(internal.clone(), v))
                .style(crate::style::toggler_style(p))
        )
        // Nudge down so it sits level with the chevron on the right.
        .padding(iced::Padding { top: 4.0, right: 0.0, bottom: 0.0, left: 0.0 })
        .into()
    };
    // Thin divider between list rows.
    let row_divider = move || -> Element<'a, Message> {
        container(Space::with_height(Length::Fixed(1.0)))
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color { a: 0.10, ..p.muted })),
                ..Default::default()
            })
            .into()
    };

    // Display the rows in the user's saved order (drag-reorderable); any tile
    // missing from tile_order falls back to canonical order.
    const CANON: [&str; 6] = ["Clock", "CPU", "GPU", "RAM", "Network", "Disk"];
    let mut display_order: Vec<&str> = tile_order.iter()
        .map(|s| s.as_str())
        .filter(|t| CANON.contains(t))
        .collect();
    for c in CANON {
        if !display_order.contains(&c) { display_order.push(c); }
    }

    // Drag state. As you drag, the list snaps to the LIVE preview order (the
    // dragged row at its target slot) and the dragged row stays visible, highlighted
    // in place. The order only changes when the cursor crosses a slot (driven by
    // SetDropTarget), so the list repaints a handful of times per drag — that's the
    // stutter fix.
    let drag_name: Option<&str> = drag.as_ref().map(|d| d.0.as_str());
    let drop_target: usize = drag.as_ref().map(|d| d.1).unwrap_or(0);
    // Row header height (pinned so expanding one row doesn't squeeze the others).
    const FLOAT_H: f32 = 44.0;
    // Reorder the rows into the live preview order while dragging.
    if let Some(name) = drag_name {
        if let Some(cur) = display_order.iter().position(|t| *t == name) {
            let t = drop_target.min(display_order.len().saturating_sub(1));
            if cur != t { let item = display_order.remove(cur); display_order.insert(t, item); }
        }
    }

    let mut tcol = column![].spacing(0);
    let last = display_order.len() - 1;
    for (i, &name) in display_order.iter().enumerate() {
        let canon_idx = CANON.iter().position(|c| *c == name).unwrap();
        let open = open_is(name);
        let vis = settings.visible_tiles.iter().any(|v| v == name);
        let is_dragging = drag_name == Some(name);
        // Soft, custom-drawn expand chevron (rounded strokes, not a font glyph).
        let chev_col = if open { p.accent } else { p.muted };
        let lblcol = if open { p.accent } else { p.text };
        // Drag band: the grip dots + label + the Shown/Hidden chip, spanning the
        // whole left region up to the "|" separator. The band is a drag layer;
        // the chip is overlaid on top of it via a stack, so clicking the chip
        // still toggles visibility while dragging works everywhere else.
        // Expand/collapse lives on the chevron.
        let band_content = container(
            row![
                crate::style::drag_grip(if is_dragging { p.accent } else { p.muted }, 16.0),
                Space::with_width(10),
                text(name.to_string()).size(13).font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(lblcol) }),
                Space::with_width(Length::Fill),
                crate::style::with_tip(vis_chip(vis, name.to_string()), if vis { "Hide this tile" } else { "Show this tile" }, p),
                Space::with_width(10),
            ].align_y(iced::Alignment::Center)
        )
        .width(Length::Fill)
        .padding(iced::Padding { top: 10.0, right: 4.0, bottom: 10.0, left: 6.0 });
        let drag_handle = stack![
            // Base drag layer — captures presses anywhere the content above
            // doesn't (i.e. everywhere except the chip button).
            mouse_area(container(Space::new(Length::Fill, Length::Fill)))
                .interaction(iced::mouse::Interaction::Grab)
                .on_press(Message::StartTileDrag(name.to_string())),
            band_content,
        ];
        let chev_btn = crate::style::with_tip(
            button(crate::style::expand_chevron(open, chev_col, 18.0))
                .padding(iced::Padding { top: 7.0, right: 9.0, bottom: 7.0, left: 9.0 })
                .style(|_: &iced::Theme, _: button::Status| button::Style { background: None, ..Default::default() })
                .on_press(Message::ToggleTileSection(name.to_string())),
            if open { "Collapse options" } else { "Expand for more options" }, p);
        // Thin separator between the Shown/Hidden chip and the expand arrow.
        let sep = container(Space::with_width(Length::Fixed(1.0)))
            .height(Length::Fixed(16.0))
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color { a: 0.22, ..p.muted })),
                ..Default::default()
            });
        let header = container(row![
            drag_handle,
            sep,
            Space::with_width(4),
            chev_btn,
        ].align_y(iced::Alignment::Center))
        // Pin the header to a fixed height. Without it the rows are Fill-height
        // (the drag layer's Space is Fill/Fill), so expanding one tile's body
        // squeezes the column and the other rows shrink — clipping their names.
        .height(Length::Fixed(FLOAT_H))
        .style(move |_| iced::widget::container::Style {
            // Highlight the row being dragged so it reads as "grabbed".
            background: if is_dragging { Some(iced::Background::Color(iced::Color { a: 0.22, ..p.accent })) } else { None },
            border: if is_dragging {
                Border { radius: 8.0.into(), width: 1.0, color: p.accent }
            } else {
                Border { radius: 8.0.into(), ..Border::default() }
            },
            ..Default::default()
        });
        tcol = tcol.push(header);
        let body = bodies[canon_idx].take().unwrap();
        if open {
            tcol = tcol.push(
                container(body).width(Length::Fill)
                    .padding(iced::Padding { top: 2.0, right: 4.0, bottom: 12.0, left: 16.0 })
            );
        }
        if i != last { tcol = tcol.push(row_divider()); }
    }
    tcol = tcol.push(Space::with_height(14));
    tcol = tcol.push(sh("Layout", "Stack tiles vertically (tall) or horizontally (wide)."));
    tcol = tcol.push(layout_pills);
    tcol = tcol.push(Space::with_height(14));
    tcol = tcol.push(sh("Behavior", "How the widget behaves on your desktop."));
    tcol = tcol.push(behavior);
    // Understated, centered hint at the very top of the Tiles tab.
    let drag_hint = container(
        text("drag to reorder tiles").size(10)
            .style(move |_| iced::widget::text::Style { color: Some(iced::Color { a: 0.65, ..p.muted }) })
    ).width(Length::Fill).center_x(Length::Fill)
        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 8.0, left: 0.0 });
    // Master temperature-unit switch — a full-width bar at the very top of the
    // Tiles tab. The unit is global (every tile's temperature uses it), so it lives
    // here as one master control rather than buried in the CPU tile's options.
    let temp_unit_bar = container(
        row![
            text("Temperature unit").size(12)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            Space::with_width(Length::Fill),
            crate::style::with_tip(seg("\u{00B0}C".into(), !fahrenheit, Message::SetFahrenheit(false)), "Show all temperatures in Celsius.", p),
            crate::style::with_tip(seg("\u{00B0}F".into(), fahrenheit, Message::SetFahrenheit(true)), "Show all temperatures in Fahrenheit.", p),
        ].align_y(iced::Alignment::Center)
    )
    .width(Length::Fill)
    .padding(iced::Padding { top: 8.0, right: 12.0, bottom: 8.0, left: 12.0 })
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color { a: 0.5, ..p.tile })),
        border: Border { radius: 8.0.into(), width: 1.0, color: iced::Color { a: 0.25, ..p.muted } },
        ..Default::default()
    });
    let tiles_tab: Element<'a, Message> = column![temp_unit_bar, Space::with_height(10), drag_hint, tcol].into();

    // ════════════════════════════════════════════════════════════
    //  RIGHT COLUMN  (Appearance / Font / Remote / Updates)
    // ════════════════════════════════════════════════════════════

    // ── Saved Presets row (label sits centered above it) ──
    let mut saved_row = row![].spacing(0).align_y(iced::Alignment::Center);
    for i in 0..5u8 {
        let idx = i as usize;
        let preset = settings.presets.get(idx).filter(|p| !p.accent.is_empty());
        let armed = preset_arming == Some(i);
        // Saved slots take the saved theme's accent as their fill (themed square);
        // armed slots show a save icon; empty slots show a plain number.
        let (label, fill, fg): (String, iced::Color, iced::Color) = if armed {
            ("\u{1F4BE}".to_string(), p.accent, iced::Color::WHITE)
        } else if let Some(pr) = preset {
            let acc = crate::style::parse_hex(&pr.accent, p.accent);
            let lum = acc.r * 0.299 + acc.g * 0.587 + acc.b * 0.114;
            ((i + 1).to_string(), acc, if lum < 0.5 { iced::Color::WHITE } else { iced::Color::BLACK })
        } else {
            ((i + 1).to_string(), p.tile, p.text)
        };
        let tip = if armed { "Click again to save the current theme here".to_string() }
            else if preset.is_some() { format!("Apply saved theme {} (right-click to delete)", i + 1) }
            else { format!("Save the current theme to slot {}", i + 1) };
        let slot = mouse_area(
            container(text(label).size(11).font(crate::style::ICONS)
                .style(move |_| iced::widget::text::Style { color: Some(fg) }))
                .width(Length::Fixed(24.0)).height(Length::Fixed(22.0))
                .align_x(iced::alignment::Horizontal::Center).align_y(iced::alignment::Vertical::Center)
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(fill)),
                    border: Border { radius: 3.0.into(), width: 1.0, color: if armed { p.accent } else { iced::Color { a: 0.5, ..p.muted } } },
                    ..Default::default()
                })
        )
        .on_press(Message::PresetSlotClick(i))
        .on_right_press(Message::ConfirmDeletePreset(i));
        saved_row = saved_row.push(crate::style::with_tip(slot, &tip, p));
        saved_row = saved_row.push(Space::with_width(3));
    }
    saved_row = saved_row.push(Space::with_width(6));
    saved_row = saved_row.push(
        tooltip(
            button(text("\u{2199}").size(12).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() })
                .on_press(Message::ImportAppearance),
            tip_box("Import appearance from a share code on the clipboard.", p), TipPos::FollowCursor,
        )
    );
    saved_row = saved_row.push(Space::with_width(3));
    saved_row = saved_row.push(
        tooltip(
            button(text("\u{2197}").size(12).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() })
                .on_press(Message::ExportAppearance),
            tip_box("Export the current appearance as a share code to the clipboard.", p), TipPos::FollowCursor,
        )
    );
    // The Theme Store is the discoverability hook for downloadable content, so it
    // gets a prominent labeled accent button (with a soft glow) instead of blending
    // in with the small import/export icons.
    saved_row = saved_row.push(Space::with_width(10));
    saved_row = saved_row.push(
        tooltip(
            button(
                row![
                    text("\u{1F4E5}").size(12).font(crate::style::ICONS)
                        .style(move |_| iced::widget::text::Style { color: Some(iced::Color::WHITE) }),
                    text("Theme Store").size(11)
                        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                        .style(move |_| iced::widget::text::Style { color: Some(iced::Color::WHITE) }),
                ].spacing(6).align_y(iced::Alignment::Center)
            )
                .padding([5, 12])
                .style(move |_: &iced::Theme, st: button::Status| {
                    let hover = matches!(st, button::Status::Hovered);
                    button::Style {
                        background: Some(iced::Background::Color(if hover { iced::Color { a: 0.9, ..p.accent } } else { p.accent })),
                        border: Border { radius: 7.0.into(), ..Border::default() },
                        text_color: iced::Color::WHITE,
                        shadow: iced::Shadow {
                            color: iced::Color { a: if hover { 0.55 } else { 0.4 }, ..p.accent },
                            offset: iced::Vector::new(0.0, 1.0),
                            blur_radius: if hover { 9.0 } else { 6.0 },
                        },
                    }
                })
                .on_press(Message::OpenThemeStore),
            tip_box("Browse & download themes and skins from the Theme Store.", p), TipPos::FollowCursor,
        )
    );
    if !appearance_status.is_empty() {
        saved_row = saved_row.push(Space::with_width(8));
        saved_row = saved_row.push(text(appearance_status.to_string()).size(10).style(move |_| iced::widget::text::Style { color: Some(p.accent) }));
    }

    let is_dark = (p.bg.r * 0.299 + p.bg.g * 0.587 + p.bg.b * 0.114) < 0.5;

    // Uniform fixed-size cluster button so both rows line up and sizes match.
    let cbtn = |glyph: &str, active: bool, msg: Message, tip: &str| -> Element<'a, Message> {
        crate::style::with_tip(
            button(
                container(text(glyph.to_string()).size(14).font(crate::style::ICONS)
                    .style(move |_| iced::widget::text::Style { color: Some(if active { iced::Color::WHITE } else { p.muted }) }))
                    .center_x(Length::Fill).center_y(Length::Fill)
            )
            .width(Length::Fixed(34.0)).height(Length::Fixed(28.0)).padding(0)
            .style(move |_: &iced::Theme, status: button::Status| {
                let hover = matches!(status, button::Status::Hovered);
                button::Style {
                    background: Some(iced::Background::Color(if active { p.accent } else { p.tile })),
                    border: Border { radius: 4.0.into(), width: 1.0, color: if hover { p.accent } else { iced::Color { a: 0.4, ..p.muted } } },
                    ..Default::default()
                }
            })
            .on_press(msg),
            tip, p)
    };
    // Randomize (dice): same size as cbtn; left = skin + colors, right = skin only.
    let dice: Element<'a, Message> = crate::style::with_tip(
        mouse_area(
            container(crate::style::dice_icon(p.muted, 18.0))
                .width(Length::Fixed(34.0)).height(Length::Fixed(28.0))
                .align_x(iced::alignment::Horizontal::Center).align_y(iced::alignment::Vertical::Center)
                .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), width: 1.0, color: iced::Color { a: 0.4, ..p.muted } }, ..Default::default() })
        )
        .on_press(Message::RandomizeAppearance).on_right_press(Message::RandomizeSkinOnly),
        "Randomize \u{2014} left-click: skin + colors; right-click: skin only", p);

    // The name cycler field (fill) shared by both rows. Clicking the name opens
    // the full picker; the ‹ › arrows still step one at a time. `lead` is the
    // small preview shown left of the name (accent dot for themes, skin box).
    let name_field = |lead: Element<'a, Message>, label: String, msg: Message, tip: &str| -> Element<'a, Message> {
        crate::style::with_tip(button(
            container(row![
                lead,
                Space::with_width(6),
                text(label).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            ].align_y(iced::Alignment::Center)).center_x(Length::Fill)
        ).width(Length::Fill).padding([4, 6])
        .style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() })
        .on_press(msg), tip, p)
    };
    // Accent dot for the theme field.
    let theme_dot: Element<'a, Message> = container(Space::new(Length::Fixed(7.0), Length::Fixed(7.0)))
        .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(p.accent)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() }).into();
    // Mini preview of the active skin's rough look (radius + border).
    let skin_prev: Element<'a, Message> = {
        let s = crate::style::skin_style(&settings.active_skin);
        container(Space::new(Length::Fixed(22.0), Length::Fixed(13.0)))
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(p.tile)),
                border: Border { radius: (s.tile_radius * 0.5).into(), width: s.tile_border.max(s.widget_border).clamp(1.0, 2.5), color: iced::Color { a: 0.6, ..p.muted } },
                ..Default::default()
            }).into()
    };

    // Top row (Skins): Download · Undo · Randomize | ‹ skin ›
    // Undo button: tinted to the accent of the appearance it would revert TO, so
    // you can see the theme you're undoing back to. Disabled when nothing to undo.
    let undo_col = undo_accent.unwrap_or(iced::Color { a: 0.4, ..p.muted });
    let undo_on = undo_accent.is_some();
    let undo_btn: Element<'a, Message> = crate::style::with_tip(
        button(
            container(text("\u{21BA}").size(15).font(crate::style::ICONS)
                .style(move |_| iced::widget::text::Style { color: Some(undo_col) }))
                .center_x(Length::Fill).center_y(Length::Fill)
        )
        .width(Length::Fixed(34.0)).height(Length::Fixed(28.0)).padding(0)
        .style(move |_: &iced::Theme, status: button::Status| {
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(p.tile)),
                border: Border { radius: 4.0.into(), width: 1.0, color: if hover && undo_on { undo_col } else { iced::Color { a: 0.4, ..p.muted } } },
                ..Default::default()
            }
        })
        .on_press_maybe(undo_on.then_some(Message::UndoAppearance)),
        "Undo to the previous appearance (color shows what you'll revert to)", p);
    let skins_row = row![
        undo_btn,
        dice,
        // Fill the slot the old Download button left behind (it moved out to the
        // prominent Theme Store button) so the ‹ › cycler lines up with Colors below.
        Space::with_width(34),
        Space::with_width(4),
        crate::style::with_tip(pill("\u{2039}".into(), false, Message::SkinPrev), "Previous skin", p),
        name_field(skin_prev, settings.active_skin.clone(), Message::OpenSkinPicker, "Browse all skins"),
        crate::style::with_tip(pill("\u{203A}".into(), false, Message::SkinNext), "Next skin", p),
    ].align_y(iced::Alignment::Center).spacing(3);

    // Bottom row (Colors): Dark · Light · (space) | ‹ theme ›
    let colors_row = row![
        cbtn("\u{1F319}", is_dark, Message::SetColorMode(true), "Dark color mode"),
        cbtn("\u{2600}", !is_dark, Message::SetColorMode(false), "Light color mode"),
        Space::with_width(34),
        Space::with_width(4),
        crate::style::with_tip(pill("\u{2039}".into(), false, Message::ThemePrev), "Previous theme", p),
        name_field(theme_dot, theme_name, Message::OpenThemePicker, "Browse all themes"),
        crate::style::with_tip(pill("\u{203A}".into(), false, Message::ThemeNext), "Next theme", p),
    ].align_y(iced::Alignment::Center).spacing(3);

    let skins_box = container(column![
        text("Skins").size(9).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        skins_row,
        Space::with_height(6),
        text("Colors").size(9).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        colors_row,
    ].spacing(3))
    .padding(8)
    .style(move |_| iced::widget::container::Style {
        border: Border { radius: 6.0.into(), width: 1.0, color: iced::Color { a: 0.3, ..p.muted } },
        ..Default::default()
    });

    // ── 5 big swatches with labels + hex ──
    let swatch_data: [(u8, &str, &str); 5] = [
        (0, "Background", &settings.theme_bg),
        (1, "Tile", &settings.theme_tile),
        (2, "Accent", &settings.theme_accent),
        (3, "Text", &settings.theme_text),
        (4, "Muted", &settings.theme_muted),
    ];
    let mut swatch_cols: Vec<Element<'a, Message>> = Vec::new();
    for (slot, name, hex) in swatch_data {
        let c = crate::style::swatch_color(hex);
        let hex_s = hex.to_string();
        let is_accent = slot == 2;
        let short_hex = if hex_s.len() > 4 { format!("#{}", &hex_s[3..]) } else { hex_s.clone() };
        let col: Element<'a, Message> = column![
            crate::style::with_tip(button(Space::new(Length::Fill, 36))
                .padding(0)
                .style(move |_, _| button::Style {
                    background: Some(iced::Background::Color(c)),
                    border: Border {
                        radius: 6.0.into(),
                        width: if is_accent { 2.0 } else { 1.0 },
                        color: if is_accent { p.text } else { iced::Color { a: 0.35, ..p.muted } },
                    },
                    ..Default::default()
                })
                .on_press(Message::EditColor(slot)), &format!("Edit the {name} color"), p),
            text(name.to_string()).size(9)
                .font(if is_accent { iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT } } else { iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(if is_accent { p.text } else { p.muted }) }),
            text(short_hex).size(9)
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        ].spacing(2).align_x(iced::Alignment::Center).width(Length::FillPortion(1)).into();
        swatch_cols.push(col);
    }
    let swatch_strip = row(swatch_cols).spacing(6);

    // Visual colour picker for the swatch the user clicked (EditColor toggles it):
    // a header (colour's name + Done) above the shared picker panel.
    let color_editor: Element<'a, Message> = if let Some(slot) = editing_color {
        let (lbl, hex) = match slot {
            0 => ("Background", settings.theme_bg.clone()),
            1 => ("Tile", settings.theme_tile.clone()),
            2 => ("Accent", settings.theme_accent.clone()),
            3 => ("Text", settings.theme_text.clone()),
            _ => ("Muted", settings.theme_muted.clone()),
        };
        let header = row![
            text(format!("{lbl} color")).size(11)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            Space::with_width(Length::Fill),
            crate::style::with_tip(button(text("Done").size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding([3, 12]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 5.0.into(), width: 1.0, color: iced::Color { a: 0.4, ..p.muted } }, ..Default::default() })
                .on_press(Message::EditColor(slot)), "Close the color picker", p),
        ].align_y(iced::Alignment::Center);
        column![
            header,
            Space::with_height(6),
            crate::color_picker::view(&hex, move |s| Message::SetHexColor(slot, s), p),
        ].spacing(0).into()
    } else {
        Space::with_height(0).into()
    };

    let appearance = column![
        container(text("Saved Presets").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
            .width(Length::Fill).center_x(Length::Fill),
        Space::with_height(3),
        container(saved_row).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(4),
        skins_box,
        Space::with_height(6),
        swatch_strip,
        Space::with_height(4),
        color_editor,
        row![fl("Muted text visibility"), Space::with_width(Length::Fill), vl(format!("{:.2}", settings.muted_contrast))],
        crate::style::with_tip(marked_slider(0.5, 1.6, settings.muted_contrast, 0.01, 1.0, p, Message::SetMutedContrast), "Brightness of secondary (muted) text like tile names and units.", p),
    ].spacing(3);

    // ── Font: sync toggle + font pickers + 3-col size sliders ──
    let fonts = column![
        row![
            tooltip(
                row![
                    toggler(settings.sync_fonts).size(14).on_toggle(Message::SetSyncFonts).style(crate::style::toggler_style(p)),
                    text("Sync fonts").size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                ].spacing(6).align_y(iced::Alignment::Center),
                tip_box("When on, changing Primary font also sets Secondary and Indicator to the same font.", p), TipPos::FollowCursor,
            ),
            Space::with_width(16),
            tooltip(
                row![
                    toggler(settings.randomize_fonts_on_dice).size(14).on_toggle(Message::SetRandomizeFonts).style(crate::style::toggler_style(p)),
                    text("Allow random fonts with die button").size(11)
                        .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                ].spacing(6).align_y(iced::Alignment::Center),
                tip_box("When on, the die button also picks random fonts in addition to theme + skin.", p), TipPos::FollowCursor,
            ),
        ].spacing(6).align_y(iced::Alignment::Center),
        {
            let mut opts = vec![FONT_DEFAULT.to_string()];
            opts.extend(fonts.iter().cloned());
            let font_picker = |slot: u8, current: &Option<String>| -> Element<'a, Message> {
                let mut o = opts.clone();
                let sel = match current {
                    Some(f) if !f.is_empty() => {
                        if !o.contains(f) { o.push(f.clone()); }
                        f.clone()
                    }
                    _ => FONT_DEFAULT.to_string(),
                };
                let tip = match slot {
                    0 => "Font for the main value numbers.",
                    1 => "Font for the secondary text (tile names).",
                    _ => "Font for the unit indicators (\u{00B0}C, %, MHz).",
                };
                crate::style::with_tip(pick_list(o, Some(sel), move |s: String| {
                    let name = if s == FONT_DEFAULT { String::new() } else { s };
                    Message::SetFont(slot, name)
                }).text_size(11).width(Length::Fill).style(crate::style::pick_list_style(p)), tip, p)
            };
            row![
                column![fl("Primary font"), font_picker(0, &settings.primary_font)].width(Length::FillPortion(1)).spacing(2),
                column![fl("Secondary font"), font_picker(1, &settings.secondary_font)].width(Length::FillPortion(1)).spacing(2),
                column![fl("Indicator font"), font_picker(2, &settings.indicator_font)].width(Length::FillPortion(1)).spacing(2),
            ].spacing(6)
        },
        fl("Font sizes"),
        row![
            column![
                fl("Primary"),
                crate::style::with_tip(marked_slider(-5.0, 5.0, settings.primary_font_offset as f32, 1.0, 0.0, p, Message::SetPrimaryFontOffset), "Nudge the main value text (numbers) larger or smaller.", p),
                vl(signed(settings.primary_font_offset)),
            ].width(Length::FillPortion(1)).spacing(2).align_x(iced::Alignment::Center),
            column![
                fl("Secondary"),
                crate::style::with_tip(marked_slider(-5.0, 5.0, settings.secondary_font_offset as f32, 1.0, 0.0, p, Message::SetSecondaryFontOffset), "Nudge the secondary text (names) larger or smaller.", p),
                vl(signed(settings.secondary_font_offset)),
            ].width(Length::FillPortion(1)).spacing(2).align_x(iced::Alignment::Center),
            column![
                fl("Indicators"),
                crate::style::with_tip(marked_slider(-5.0, 5.0, settings.indicator_font_offset as f32, 1.0, 0.0, p, Message::SetIndicatorFontOffset), "Nudge the unit indicators (°C, %, MHz) larger or smaller.", p),
                vl(signed(settings.indicator_font_offset)),
            ].width(Length::FillPortion(1)).spacing(2).align_x(iced::Alignment::Center),
        ].spacing(8),
    ].spacing(4);

    // ── Updates box ──
    let inline_btn = |lbl: &str, msg: Message| crate::style::inline_btn(lbl, msg, p);
    let status_color = match update.status_kind {
        1 => iced::Color::from_rgb8(0x58, 0xC8, 0x58),
        2 => iced::Color::from_rgb8(0xC0, 0x60, 0x60),
        _ => p.muted,
    };
    let mut version_row = row![
        text("Current version").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_width(Length::Fill),
        text(format!("v{}", update.current_version)).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
    ].align_y(iced::Alignment::Center);
    // Link to the latest GitHub release, to the right of the version.
    if update.latest_changelog.is_some() {
        version_row = version_row.push(Space::with_width(10));
        version_row = version_row.push(crate::style::with_tip(
            button(
                text("View release \u{2197}").size(10)
                    .style(move |_| iced::widget::text::Style { color: Some(p.accent) })
            )
            .padding(0)
            .style(|_: &iced::Theme, _: button::Status| button::Style { background: None, ..Default::default() })
            .on_press(Message::OpenUrl(crate::updates::RELEASES_URL.to_string())),
            "Open the latest release on GitHub", p));
    }
    let mut updates_col = column![
        version_row,
        Space::with_height(4),
    ].spacing(3);

    // Mode pills (Off → Manual → Auto, increasing automation). Flux never installs
    // anything on its own: Auto only pops up a notice; you choose when to install.
    use flux_core::settings::UpdateMode;
    let mode_row = row![
        crate::style::with_tip(pill("Off".into(), update.mode == UpdateMode::Off, Message::SetUpdateMode("Off".into())), "Never check for updates.", p),
        crate::style::with_tip(pill("Manual".into(), update.mode == UpdateMode::Manual, Message::SetUpdateMode("Manual".into())), "Check in the background and flag the gear with a dot \u{2014} never pops up.", p),
        crate::style::with_tip(pill("Auto".into(), update.mode == UpdateMode::Auto, Message::SetUpdateMode("Auto".into())), "Check in the background and pop up a notice when an update is ready (never installs on its own).", p),
        Space::with_width(Length::Fill),
    ].spacing(4).align_y(iced::Alignment::Center);
    updates_col = updates_col.push(mode_row);
    // One-line description of what the selected mode does.
    let mode_hint = match update.mode {
        UpdateMode::Off => "Never checks for updates.",
        UpdateMode::Manual => "Checks in the background and flags the gear with a dot \u{2014} never pops up.",
        UpdateMode::Auto => "Checks in the background and pops up a notice when an update is ready \u{2014} you choose when to install.",
    };
    updates_col = updates_col.push(
        text(mode_hint).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    );
    // Action button: Check now, or Download/Later when an update is available.
    // Hidden in Off mode and while a download's progress bar is showing.
    if update.progress.is_none() && update.mode != UpdateMode::Off {
        let mut action_row = row![Space::with_width(Length::Fill)].align_y(iced::Alignment::Center);
        if update.available.is_some() {
            action_row = action_row.push(crate::style::with_tip(inline_btn("Download", Message::DownloadUpdate), "Download, verify, and install the available update now.", p));
            action_row = action_row.push(crate::style::with_tip(inline_btn("Later", Message::UpdateLater), "Dismiss this update for now.", p));
        } else {
            action_row = action_row.push(crate::style::with_tip(inline_btn("Check now", Message::CheckForUpdates), "Check GitHub for a newer version right now.", p));
        }
        updates_col = updates_col.push(action_row);
    }

    if !update.status.is_empty() {
        updates_col = updates_col.push(
            text(update.status.clone()).size(11).style(move |_| iced::widget::text::Style { color: Some(status_color) })
        );
    }
    // Live download/verify progress bar, in the accent colour.
    if let Some(frac) = update.progress {
        updates_col = updates_col.push(Space::with_height(2));
        updates_col = updates_col.push(
            iced::widget::progress_bar(0.0..=1.0, frac)
                .height(Length::Fixed(6.0))
                .style(move |_: &iced::Theme| iced::widget::progress_bar::Style {
                    background: iced::Background::Color(iced::Color { a: 0.18, ..p.muted }),
                    bar: iced::Background::Color(p.accent),
                    border: Border { radius: 3.0.into(), ..Border::default() },
                })
        );
    }
    // Sub-tabs: "Changelog" (what's new) vs "Verification" (how updates are
    // checked, the SHA-256 gate, and VirusTotal). Fills the rest of the card.
    updates_col = updates_col.push(Space::with_height(6));
    updates_col = updates_col.push(
        row![
            crate::style::with_tip(pill("Changelog".into(), !update.show_info, Message::SetUpdatesInfo(false)), "Show the latest release's notes.", p),
            crate::style::with_tip(pill("Verification".into(), update.show_info, Message::SetUpdatesInfo(true)), "Explain how updates are verified (SHA-256 + VirusTotal).", p),
        ].spacing(4)
    );
    let body_md: String = if update.show_info {
        VERIFICATION_MD.to_string()
    } else {
        match (&update.available, &update.latest_changelog) {
            // `log` is already trimmed; `body` is the raw latest release notes.
            (Some((_, log)), _) => log.clone(),
            (None, Some((_, body))) => crate::updates::whats_new(body),
            _ => "No release notes available \u{2014} check your internet connection, or open the \"Verification\" tab to read how updates work.".to_string(),
        }
    };
    // The version these notes describe — the header only shows the INSTALLED
    // version, so without this you couldn't tell which version an available update
    // actually is. Centered at the top of the box; accented when it's pending.
    let changelog_version: Option<String> = if update.show_info {
        None
    } else {
        match (&update.available, &update.latest_changelog) {
            (Some((v, _)), _) => Some(v.clone()),
            (None, Some((v, _))) => Some(v.clone()),
            _ => None,
        }
    };
    let updating = update.available.is_some();
    let mut box_inner = column![].spacing(6);
    if let Some(ver) = changelog_version {
        let v = ver.trim_start_matches('v'); // release tags are "v1.2.3" — avoid "vv"
        let label = if updating { format!("v{v} available") } else { format!("v{v}") };
        box_inner = box_inner.push(
            container(text(label).size(12)
                .font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(if updating { p.accent } else { p.text }) }))
                .width(Length::Fill).center_x(Length::Fill)
        );
    }
    box_inner = box_inner.push(
        scrollable(container(changelog_md(&body_md, p)).padding(iced::Padding { top: 0.0, right: 14.0, bottom: 0.0, left: 0.0 })).width(Length::Fill).height(Length::Fill).style(crate::style::scrollable_style(p))
    );
    updates_col = updates_col.push(
        container(box_inner)
            .padding(8).width(Length::Fill).height(Length::Fill)
            .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(crate::style::field_bg(p))), border: Border { radius: 6.0.into(), ..Border::default() }, ..Default::default() })
    );
    updates_col = updates_col.push(
        row![
            Space::with_width(Length::Fill),
            text(update.last_checked.clone()).size(9).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        ]
    );

    let updates = container(updates_col.height(Length::Fill))
    .padding([10, 12])
    .width(Length::Fill)
    .height(Length::Fill) // stretch to fill the Tools tab's remaining height
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(p.tile)),
        border: Border { radius: 8.0.into(), ..Border::default() },
        ..Default::default()
    });

    let appearance_tab: Element<'a, Message> = column![
        sh("Appearance", "Customize colors. Click any swatch in the strip to open the color picker."), appearance,
        Space::with_height(6),
        sh("Size", "Scale the whole widget and set tile width/height."), sizing,
        Space::with_height(6),
        sh("Font", "Pick fonts for Primary numbers, Secondary labels, and Indicators (units). Toggle 'Sync' to lock all three together. Sizes nudge the chosen font up or down."), fonts,
    ].spacing(4).into();

    // ── Tools tab: a 2×2 grid of launcher cards (icon-tinted, with live status) ──
    let n_alerts = settings.warnings.iter().filter(|w| w.enabled).count();
    let n_block = settings.snap_blocklist.len();
    let alerts_status = if n_alerts > 0 { (format!("{n_alerts} set"), true) } else { ("Off".to_string(), false) };
    let gm_status = if settings.game_mode_hotkey.trim().is_empty() { ("Unset".to_string(), false) } else { (settings.game_mode_hotkey.clone(), true) };
    let util_status = if n_block > 0 { (format!("{n_block} blocked"), true) } else { ("None".to_string(), false) };
    let remote_status = if settings.remote_enabled { (format!("On \u{00B7} :{}", settings.remote_port), true) } else { ("Off".to_string(), false) };

    let tool_card = |icon: &str, ic_col: iced::Color, title: &str, subtitle: &str, status: (String, bool), msg: Message| -> Element<'a, Message> {
        // Live-status pill: tinted in the card's colour when active, muted when off.
        let (stxt, active) = status;
        let scol = if active { ic_col } else { p.muted };
        let chip = container(
            text(stxt).size(9).font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .wrapping(iced::widget::text::Wrapping::None)
                .style(move |_| iced::widget::text::Style { color: Some(scol) })
        ).padding(iced::Padding { top: 2.0, right: 7.0, bottom: 2.0, left: 7.0 })
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color { a: 0.16, ..scol })),
                border: Border { radius: 7.0.into(), ..Border::default() },
                ..Default::default()
            });
        // Brighten the glyph well above the tint so its detail (e.g. the
        // controller) reads clearly against the chip.
        let lift = |c: f32| c + (1.0 - c) * 0.55;
        let icon_glyph = iced::Color { r: lift(ic_col.r), g: lift(ic_col.g), b: lift(ic_col.b), a: 1.0 };
        let ic = container(
            text(icon.to_string()).size(18).font(iced::Font::with_name("Segoe UI Symbol"))
                .style(move |_| iced::widget::text::Style { color: Some(icon_glyph) })
        ).width(36).height(36).center_x(36).center_y(36)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color { a: 0.22, ..ic_col })),
                border: Border { radius: 9.0.into(), ..Border::default() },
                ..Default::default()
            });
        button(
            column![
                row![ic, Space::with_width(Length::Fill), chip].align_y(iced::Alignment::Center).width(Length::Fill),
                Space::with_height(Length::Fill),
                text(title.to_string()).size(13)
                    .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                    .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                text(subtitle.to_string()).size(10)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            ].spacing(2)
        )
        .width(Length::FillPortion(1))
        .height(Length::Fixed(104.0))
        .padding(12)
        .style(move |_: &iced::Theme, st: button::Status| {
            let hover = matches!(st, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(iced::Color { a: if hover { 0.15 } else { 0.07 }, ..ic_col })),
                border: Border { radius: 12.0.into(), width: 1.0, color: iced::Color { a: if hover { 0.6 } else { 0.3 }, ..ic_col } },
                ..Default::default()
            }
        })
        .on_press(msg).into()
    };
    // Thematic card colours: four distinct hues rotated around the theme accent,
    // so the Tools tab tracks the active theme instead of fixed RGB colours.
    let c_alerts = p.accent;
    let c_game = crate::style::shift_hue(p.accent, 38.0);
    let c_util = crate::style::shift_hue(p.accent, -42.0);
    let c_remote = crate::style::shift_hue(p.accent, 90.0);
    let tools_tab: Element<'a, Message> = column![
        sh("Tools", "Configure Alerts, Game Mode, and Utilities."),
        row![
            tool_card("\u{26A0}", c_alerts, "Alerts", "Temp / load thresholds", alerts_status, Message::OpenAlerts),
            tool_card("\u{1F3AE}", c_game, "Game Mode", "Hotkey-snap overlay", gm_status, Message::OpenGameMode),
        ].spacing(8),
        row![
            tool_card("\u{1F527}", c_util, "Utilities", "Tools & snap blocklist", util_status, Message::OpenUtilities),
            tool_card("\u{1F4E1}", c_remote, "Remote", "Share & monitor", remote_status, Message::OpenRemote),
        ].spacing(8),
        Space::with_height(10),
        sh("Updates", "Check for and install new versions of Flux."),
        updates,
    ].spacing(8).height(Length::Fill).into();

    // ════════════════════════════════════════════
    //  ASSEMBLY  (tabbed)
    // ════════════════════════════════════════════

    let tab_labels = ["Tiles", "Appearance", "Tools"];
    let mut tab_panes = vec![tiles_tab, appearance_tab, tools_tab];
    let active = tab.min(tab_panes.len() - 1);

    // ── Soft Premium chrome colours (derived from the live palette) ──
    let darken = |c: iced::Color, f: f32| iced::Color { r: c.r * f, g: c.g * f, b: c.b * f, a: 1.0 };
    let window_bg = darken(p.bg, 0.88);
    let sunken = darken(p.bg, 0.70);
    let hairline = iced::Color { a: 0.22, ..p.muted };
    let accent_border = crate::style::lerp(window_bg, p.accent, 0.45);
    let bg_opaque = iced::Color { a: 1.0, ..p.bg };

    // Pill tab bar: a sunken container with an accent-filled pill on the active
    // tab (dark text), idle tabs muted. Equal-width tabs.
    let mut strip = row![].spacing(4);
    for (i, lbl) in tab_labels.iter().enumerate() {
        let on = i == active;
        strip = strip.push(
            button(container(text(lbl.to_string()).size(13)
                .wrapping(iced::widget::text::Wrapping::None)
                // Inactive tabs use Normal, not Medium: Segoe UI ships no Medium
                // (500) face, so iced fell back to a symbol font and the labels
                // rendered as tofu. Normal (400) is always present; active stays
                // Semibold for the weight contrast.
                .font(iced::Font { weight: if on { iced::font::Weight::Semibold } else { iced::font::Weight::Normal }, ..iced::Font::DEFAULT }))
                .center_x(Length::Fill))
                .width(Length::Fill)
                .padding(iced::Padding { top: 7.0, right: 4.0, bottom: 7.0, left: 4.0 })
                .style(move |_: &iced::Theme, status: button::Status| {
                    let hover = matches!(status, button::Status::Hovered);
                    button::Style {
                        background: Some(iced::Background::Color(if on {
                            p.accent
                        } else if hover {
                            iced::Color { a: 0.10, ..p.text }
                        } else {
                            iced::Color::TRANSPARENT
                        })),
                        text_color: if on { bg_opaque } else { p.muted },
                        border: Border { radius: 9.0.into(), ..Default::default() },
                        ..Default::default()
                    }
                })
                .on_press(Message::SetSettingsTab(i)),
        );
    }
    let strip_bar = container(strip)
        .padding(4)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(sunken)),
            border: Border { radius: 14.0.into(), width: 1.0, color: hairline },
            ..Default::default()
        });

    // The active section sits on a Soft-Premium card: tile surface, hairline
    // border, 16px corners, generous padding.
    // Card surface is the theme bg (between the darker window and the lighter
    // tile-colored controls), so the controls/inputs inside keep their contrast
    // and outlines instead of going tile-on-tile and disappearing.
    // Tools (index 2) fills the full height (grid pinned to the top, Updates card
    // stretches to fill the rest); the other tabs keep their content vertically
    // centred in the fixed-height window.
    let is_tools = active == 2;
    // Non-tools tabs scroll when their content overflows the window — otherwise
    // expanding a tile's options (Tiles tab) pushes everything below it off the
    // bottom with no way to reach it. The drag-reorder collapses all sections
    // first, so the list always fits (scroll offset 0) while dragging — keeping
    // the floating drag row aligned. Tools keeps its own fill layout.
    let pane_inner: Element<'a, Message> = if is_tools {
        tab_panes.remove(active)
    } else {
        // Inset the scrolled content on the right so the scrollbar sits in its
        // own gutter instead of overlapping the row toggles/chevrons.
        scrollable(
            container(tab_panes.remove(active))
                .padding(iced::Padding { top: 0.0, right: 14.0, bottom: 0.0, left: 0.0 }),
        )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::style::scrollable_style(p))
            .into()
    };
    let active_pane = container(pane_inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(16)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color { a: 1.0, ..p.bg })),
            // Echo the window's 20px rounding (a touch tighter as it's inset).
            border: Border { radius: 18.0.into(), width: 1.0, color: hairline },
            ..Default::default()
        });
    let pane_slot = container(active_pane).width(Length::Fill).height(Length::Fill);
    let columns = column![
        strip_bar,
        Space::with_height(12),
        pane_slot,
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // 32px caption: "Settings" left, ✕ right, whole bar draggable
    // On the muted-coloured title bar, draw the ✕ / title in the theme bg colour
    // (muted is designed to read against bg, so bg reads against muted).
    let on_bar = iced::Color { a: 1.0, ..p.bg };
    let close_btn = crate::style::with_tip(button(
        text("\u{2715}").size(13).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(on_bar) })
    ).padding([2, 8]).style(|_,_| button::Style { background: None, ..Default::default() }).on_press(Message::SaveClose),
        "Save and close", p);

    // Tall, fully-draggable title bar: an accent brand mark + centered "Settings"
    // on a subtly accent-tinted band (rounded to match the window top), ✕ on the
    // right. The whole band drags the window; only the close button doesn't.
    let brand = crate::style::brand_pulse(on_bar, 18.0);
    let caption = mouse_area(
        container(
            stack![
                container(row![
                    brand,
                    Space::with_width(8),
                    text("Settings").size(13).font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                        .style(move |_| iced::widget::text::Style { color: Some(on_bar) }),
                ].align_y(iced::Alignment::Center))
                    .width(Length::Fill).height(Length::Fill)
                    .center_x(Length::Fill).center_y(Length::Fill),
                container(close_btn)
                    .width(Length::Fill).height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Right).align_y(iced::alignment::Vertical::Center),
            ]
        )
        .width(Length::Fill)
        .height(Length::Fixed(48.0))
        .padding(iced::Padding { top: 0.0, right: 6.0, bottom: 0.0, left: 8.0 })
        .style(move |_| iced::widget::container::Style {
            // Title bar uses the theme's muted swatch directly.
            background: Some(iced::Background::Color(iced::Color { a: 1.0, ..p.accent })),
            // Match the window's INNER corner radius (outer 20 − 1.5px border)
            // so the caption fills the rounded corner exactly — no window-bg
            // wedge (too small) and no poking past the border (too large).
            border: Border { radius: iced::border::Radius { top_left: crate::style::win_radius(18.5), top_right: crate::style::win_radius(18.5), bottom_right: 0.0, bottom_left: 0.0 }, ..Border::default() },
            ..Default::default()
        })
    ).on_press(Message::DragWindow(win_id));
    let caption_hairline = container(Space::new(Length::Fill, Length::Fixed(1.0)))
        .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(iced::Color { a: 0.30, ..p.accent })), ..Default::default() });

    // Bottom bar: [?] Help + Reset + Save. (Tools moved to its own top tab.)
    let help_btn = tooltip(button(text("?").size(14).font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).padding([4, 12]).style(move |_,_| button::Style {
        background: Some(iced::Background::Color(p.tile)),
        border: Border { radius: 7.0.into(), ..Border::default() },
        ..Default::default()
    }).on_press(Message::OpenHelp), tip_box("Help \u{2014} feature guide", p), TipPos::FollowCursor);

    // C# BottomBarDanger: tile fill, IndianRed border + text, radius 6.
    let indian_red = iced::Color::from_rgb(0.804, 0.361, 0.361);
    let reset_btn = crate::style::with_tip(button(text("Reset to Defaults").size(12)
        .style(move |_| iced::widget::text::Style { color: Some(indian_red) })
    ).padding([7, 14]).style(move |_,_| button::Style {
        background: Some(iced::Background::Color(p.tile)),
        text_color: indian_red,
        border: Border { radius: 6.0.into(), width: 1.0, color: indian_red },
        ..Default::default()
    }).on_press(Message::ResetDefaults), "Reset all settings to their defaults", p);

    // C# BottomBarPrimary: accent fill, background-coloured text, semibold.
    let bg_opaque = iced::Color { a: 1.0, ..p.bg };
    let save_btn = crate::style::with_tip(button(text("Save and Close").size(12)
        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(bg_opaque) })
    ).padding([7, 14]).style(move |_,_| button::Style {
        background: Some(iced::Background::Color(p.accent)),
        text_color: bg_opaque,
        border: Border { radius: 6.0.into(), width: 1.0, color: p.accent },
        ..Default::default()
    }).on_press(Message::SaveClose), "Save changes and close", p);

    let divider = container(Space::new(Length::Fill, 1)).style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(iced::Color { a: 0.3, ..p.muted })), ..Default::default() });

    let bottom_bar = container(
        row![help_btn, Space::with_width(8), reset_btn, Space::with_width(Length::Fill), save_btn]
            .align_y(iced::Alignment::Center)
    ).width(Length::Fill).padding(iced::Padding { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 });

    // Caption sits flush in the top-left corner; the body below is inset.
    let body = container(column![
        columns,
        // Breathing room so the content card's rounded bottom border doesn't
        // collide with the divider line above the action bar.
        Space::with_height(12),
        divider,
        bottom_bar,
    ]).width(Length::Fill).height(Length::Fill)
        .padding(iced::Padding { top: 4.0, right: 20.0, bottom: 10.0, left: 20.0 });

    // Soft Premium window chrome: darker window bg, 20px corners, 1.5px
    // accent-tinted outline so dark-on-dark dialogs don't blend in.
    let window = container(column![caption, caption_hairline, body])
        .width(Length::Fill).height(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(window_bg)),
            border: Border { radius: crate::style::win_radius(20.0).into(), width: 1.5, color: accent_border },
            ..Default::default()
        });

    // Modal share-code dialog (Import/Export) layered over the settings window.
    // (The drag reorder no longer floats a row — it shows an in-list drop-line.)
    let mut layers: Vec<Element<'a, Message>> = vec![window.into()];
    if let Some((is_export, code)) = share_dialog {
        layers.push(share_dialog_view(is_export, code, copied_opacity, sunken, hairline, p));
    }
    if layers.len() == 1 {
        layers.pop().unwrap()
    } else {
        iced::widget::Stack::with_children(layers).into()
    }
}

// Centered modal for importing/exporting the appearance share code, on a dimmed
// backdrop. Export pre-fills the code (Copy button); Import starts empty (Apply).
fn share_dialog_view<'a>(is_export: bool, code: String, copied_opacity: f32, card_bg: iced::Color, hairline: iced::Color, p: Palette) -> Element<'a, Message> {
    let bg_opaque = iced::Color { a: 1.0, ..p.bg };
    let title = if is_export { "Export appearance" } else { "Import appearance" };
    let hint = if is_export {
        "Copy this code and share it. Paste it into another Flux to apply your look."
    } else {
        "Paste an appearance share code, then Apply."
    };
    let field = text_input("paste code\u{2026}", &code)
        .id(text_input::Id::new("share_code"))
        .on_input(Message::ShareCodeInput)
        .size(12)
        .padding(8)
        .style(move |_t, _s| iced::widget::text_input::Style {
            background: iced::Background::Color(crate::style::field_bg(p)),
            border: Border { radius: 6.0.into(), width: 1.0, color: iced::Color { a: 0.4, ..p.muted } },
            icon: p.muted, placeholder: p.muted, value: p.text, selection: iced::Color { a: 0.3, ..p.accent },
        });
    let action: Element<'a, Message> = if is_export {
        button(text("Copy").size(12).font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(bg_opaque) }))
            .padding([7, 16])
            .style(move |_, _| button::Style { background: Some(iced::Background::Color(p.accent)), text_color: bg_opaque, border: Border { radius: 6.0.into(), ..Border::default() }, ..Default::default() })
            .on_press(Message::CopyShareCode).into()
    } else {
        button(text("Apply").size(12).font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(bg_opaque) }))
            .padding([7, 16])
            .style(move |_, _| button::Style { background: Some(iced::Background::Color(p.accent)), text_color: bg_opaque, border: Border { radius: 6.0.into(), ..Border::default() }, ..Default::default() })
            .on_press(Message::ApplyShareCode).into()
    };
    let close = button(text("Close").size(12).style(move |_| iced::widget::text::Style { color: Some(p.text) }))
        .padding([7, 14])
        .style(move |_, _| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 6.0.into(), width: 1.0, color: iced::Color { a: 0.3, ..p.muted } }, ..Default::default() })
        .on_press(Message::CloseShareDialog);
    let card = container(column![
        text(title).size(15).font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        text(hint).size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_height(4),
        field,
        Space::with_height(4),
        row![
            // Fading "Copied!" toast (alpha driven by copied_opacity).
            text("Copied to clipboard!").size(11)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(iced::Color { a: copied_opacity, ..p.accent }) }),
            Space::with_width(Length::Fill),
            close,
            Space::with_width(8),
            action,
        ].align_y(iced::Alignment::Center),
    ].spacing(8))
        .width(Length::Fixed(440.0))
        .padding(18)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color { a: 1.0, ..card_bg })),
            border: Border { radius: 14.0.into(), width: 1.0, color: hairline },
            ..Default::default()
        });
    let backdrop = mouse_area(
        container(Space::new(Length::Fill, Length::Fill)).width(Length::Fill).height(Length::Fill)
            .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(iced::Color { a: 0.55, ..iced::Color::BLACK })), ..Default::default() })
    ).on_press(Message::CloseShareDialog);
    stack![
        backdrop,
        container(card).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill),
    ].into()
}

// C# value-label format "+0;-0;0": +N for positive, -N for negative, 0 for zero.
fn signed(v: i32) -> String {
    if v > 0 { format!("+{}pt", v) } else { format!("{}pt", v) }
}

// Recreates the C# "Slim" slider + the default-value marker.
//   * Track: 2px, accent on the filled (left) side, muted@0.25 on the right.
//   * Thumb: 12px accent circle with a 2px background-coloured ring.
//   * Marker: a thin vertical line at the default value (1.5px muted@0.5),
//     glowing accent (2px, full opacity) when the value is within 5% of default.
pub(crate) fn marked_slider<'a>(min: f32, max: f32, val: f32, step: f32, default: f32, p: Palette, on: fn(f32) -> Message) -> Element<'a, Message> {
    use iced::widget::slider::{Handle, HandleShape, Rail, Style};
    let muted = p.muted;
    let accent = p.accent;
    // Premium-glow: thick accent rail + bright bead thumb with a translucent
    // accent halo. Handle radius stays 6 so the default-marker tick stays aligned.
    let sl = slider(min..=max, val, on).step(step).style(move |_t, s| {
        let hot = matches!(s, iced::widget::slider::Status::Hovered | iced::widget::slider::Status::Dragged);
        Style {
            rail: Rail {
                backgrounds: (
                    iced::Background::Color(if hot { crate::style::lerp(accent, iced::Color::WHITE, 0.18) } else { accent }),
                    iced::Background::Color(iced::Color { a: 0.22, ..muted }),
                ),
                width: 5.0,
                border: Border { radius: 2.5.into(), width: 0.0, color: iced::Color::TRANSPARENT },
            },
            handle: Handle {
                shape: HandleShape::Circle { radius: 6.0 },
                background: iced::Background::Color(crate::style::lerp(accent, iced::Color::WHITE, 0.6)),
                border_width: if hot { 4.0 } else { 3.0 },
                border_color: iced::Color { a: if hot { 0.65 } else { 0.4 }, ..accent },
            },
        }
    });

    let range = max - min;
    let frac = if range > 0.0 { ((default - min) / range).clamp(0.0, 1.0) } else { 0.0 };
    // C# UpdateGlow: marker brightens to accent as the value nears the default,
    // fades to muted as it moves away.
    let dist = if range > 0.0 { (val - default).abs() / range } else { 1.0 };
    let (mc, mw, mo): (iced::Color, f32, f32) = if dist < 0.05 {
        (p.accent, 2.5, 1.0)
    } else if dist < 0.15 {
        let t = (dist - 0.05) / 0.10;
        (p.accent, 2.5 - t * 0.5, 1.0 - t * 0.3)
    } else {
        (p.muted, 2.0, 0.7)
    };
    let marker_color = iced::Color { a: mc.a * mo, ..mc };
    // Draw the marker with a canvas so it lands at EXACTLY iced's thumb centre
    // formula (x = 6 + frac*(width-12)). A flex-spacer layout was always off by
    // ~half the line width; the canvas knows the real width at draw time.
    let marker = iced::widget::canvas(DefaultMarker { frac, color: marker_color, width: mw })
        .width(Length::Fill)
        .height(Length::Fill);

    // Transparent top overlay that reports the Pointer cursor. iced's slider
    // reports "Grabbing", which winit maps to the 4-arrow SizeAll cursor on
    // Windows. Stack returns the topmost non-None interaction, so this overlay
    // wins; it has no handlers, so press/drag events fall through to the slider.
    let cursor_fix = mouse_area(Space::new(Length::Fill, Length::Fill))
        .interaction(iced::mouse::Interaction::Pointer);
    // Vertically center the slider so its rail sits at the canvas centre — then
    // the marker tick grows equally above and below it (iced's Stack top-anchors
    // children, which otherwise leaves the rail off-centre).
    let sl_centered = container(sl).height(Length::Fill).center_y(Length::Fill);
    stack![marker, sl_centered, cursor_fix].height(Length::Fixed(20.0)).into()
}

// Canvas program that draws the default-value tick at the exact thumb position.
struct DefaultMarker {
    frac: f32,
    color: iced::Color,
    width: f32,
}

impl iced::widget::canvas::Program<Message> for DefaultMarker {
    type State = ();
    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path};
        let mut frame = Frame::new(renderer, bounds.size());
        // Matches iced slider: handle centre = 6 + frac*(width - 12).
        let cx = 6.0 + self.frac * (bounds.width - 12.0);
        // Fill the box minus a 1px breathing margin, centred — so the tick's
        // height above the rail and depth below it are always equal.
        let h = (bounds.height - 2.0).max(0.0);
        let y = (bounds.height - h) / 2.0;
        let rect = Path::rectangle(
            iced::Point::new(cx - self.width / 2.0, y),
            iced::Size::new(self.width, h),
        );
        frame.fill(&rect, self.color);
        vec![frame.into_geometry()]
    }
}

// Styled tooltip body matching the C# hover cards (dark box, wrapped text).
fn tip_box<'a>(t: &str, p: Palette) -> Element<'a, Message> {
    container(
        text(t.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.text) })
    )
    .max_width(240)
    .padding(8)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(p.tile)),
        border: Border { radius: 6.0.into(), width: 1.0, color: iced::Color { a: 0.4, ..p.muted } },
        ..Default::default()
    })
    .into()
}

fn qmark<'a>(p: Palette, tip: &str) -> Element<'a, Message> {
    let bubble = container(
        text("?").size(9).font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(iced::Color::WHITE) })
    )
    .width(14).height(14)
    .center_x(14).center_y(14)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color { a: 0.4, ..p.muted })),
        border: Border { radius: 7.0.into(), ..Border::default() },
        ..Default::default()
    });
    if tip.is_empty() {
        bubble.into()
    } else {
        tooltip(bubble, tip_box(tip, p), TipPos::FollowCursor).into()
    }
}



