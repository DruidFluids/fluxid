use fluid_core::sensor_data::*;
use fluid_core::settings::AppSettings;
use iced::widget::{column, container, row, text, Space};
use iced::{Border, Color, Element, Length};
use iced::font::Weight;
use chrono::{Datelike, Timelike};
use crate::fmt;
use crate::style::{named_font, Palette};
use crate::Message;

#[derive(Debug, Clone, Copy, Default)]
pub struct WarnView {
    pub flash: bool,
    pub accent_override: Option<Color>,
}

// Font-size resolver matching C# ThemeApplier: base + offset, floored at 7,
// scaled by UI scale.
fn sz(base: i32, offset: i32, s: &AppSettings) -> u16 {
    (((base + offset).max(7)) as f32 * s.ui_scale).round().max(7.0) as u16
}

// Secondary text colour: C# SecondaryValueText uses the text brush at 0.85.
fn text085(p: Palette) -> Color { Color { a: p.text.a * 0.85, ..p.text } }

// ── Tile text roles (C# Theme.xaml styles) ──────────────────────────────────
// Header: IndicatorFontSize (16+indicatorOffset), Bold, muted, secondary font.
fn header<'a>(label: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    container(
        text(label).size(sz(16, s.indicator_font_offset, s))
            .font(named_font(&s.secondary_font, Weight::Bold))
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).width(Length::Fill).center_x(Length::Fill).into()
}

// SubHeader: SecondaryFontSize (11+secondaryOffset), muted (0.8), secondary
// font. Collapses when empty (matches the C# Text="" trigger).
fn sub_header<'a>(label: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    if label.trim().is_empty() {
        return Space::with_height(0).into();
    }
    container(
        text(label).size(sz(11, s.secondary_font_offset, s))
            .font(named_font(&s.secondary_font, Weight::Normal))
            .style(move |_| iced::widget::text::Style { color: Some(Color { a: p.muted.a * 0.8, ..p.muted }) })
    ).width(Length::Fill).center_x(Length::Fill).into()
}

// Primary value: PrimaryFontSize (18+primaryOffset), Bold, text, primary font.
fn big<'a>(t: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(18, s.primary_font_offset, s))
        .font(named_font(&s.primary_font, Weight::Bold))
        .style(move |_| iced::widget::text::Style { color: Some(p.text) })
        .into()
}

// Inline accent unit that lives INSIDE the primary value (CPU/GPU °C, %): same
// size as the primary number (PrimaryFontSize), accent colour, indicator font.
fn unit_inline<'a>(t: String, accent: Color, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(18, s.primary_font_offset, s))
        .font(named_font(&s.indicator_font, Weight::Bold))
        .style(move |_| iced::widget::text::Style { color: Some(accent) })
        .into()
}

// Unit slot next to the primary (RAM GB, clock am/pm): UnitFontSize
// (12+primaryOffset), SemiBold, accent, indicator font.
fn unit<'a>(t: String, accent: Color, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(12, s.primary_font_offset, s))
        .font(named_font(&s.indicator_font, Weight::Semibold))
        .style(move |_| iced::widget::text::Style { color: Some(accent) })
        .into()
}

// Secondary-line number: SecondaryFontSize (11+secondaryOffset), text@0.85.
fn small<'a>(t: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    let c = text085(p);
    text(t).size(sz(13, s.secondary_font_offset, s))
        .font(named_font(&s.secondary_font, Weight::Normal))
        .style(move |_| iced::widget::text::Style { color: Some(c) })
        .into()
}

// Secondary-line unit ([a] segment): same size as the secondary number, accent.
fn small_unit<'a>(t: String, accent: Color, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(13, s.secondary_font_offset, s))
        .font(named_font(&s.secondary_font, Weight::Normal))
        .style(move |_| iced::widget::text::Style { color: Some(accent) })
        .into()
}

// Network/Disk stacked value: number at PrimaryFontSize (18+primaryOffset),
// unit at UnitFontSize (12+primaryOffset) accent (C# AccentScale=0.75 path).
fn line_value<'a>(v: String, u: String, p: Palette, accent: Color, s: &AppSettings) -> Element<'a, Message> {
    row![
        text(v).size(sz(18, s.primary_font_offset, s))
            .font(named_font(&s.primary_font, Weight::Bold))
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        Space::with_width(3),
        text(u).size(sz(12, s.primary_font_offset, s))
            .font(named_font(&s.indicator_font, Weight::Semibold))
            .style(move |_| iced::widget::text::Style { color: Some(accent) }),
    ]
    .align_y(iced::Alignment::End)
    .into()
}

pub fn cpu_tile<'a>(cpu: &CpuData, s: &AppSettings, p: Palette, w: WarnView) -> Element<'a, Message> {
    let accent = w.accent_override.unwrap_or(p.accent);
    let name = if !s.cpu_custom_name.is_empty() {
        s.cpu_custom_name.clone()
    } else {
        let n = fmt::shorten(&cpu.name);
        if n.is_empty() { "CPU".to_string() } else { n }
    };

    // C# CPU primary: "{temp}°C  {load}%" on one line (temp present only when
    // a real reading exists), units inline-accent at primary size.
    let mut primary = row![].align_y(iced::Alignment::End);
    if let Some((tv, tu)) = fmt::fmt_temp(cpu.temperature_c, s) {
        primary = primary.push(big(tv, p, s)).push(unit_inline(tu, accent, s)).push(Space::with_width(8));
    }
    primary = primary
        .push(big(format!("{:.0}", cpu.usage_percent), p, s))
        .push(unit_inline("%".into(), accent, s));

    let secondary: Element<'a, Message> = match cpu.clock_mhz {
        Some(m) => row![
            small(format!("{:.0}", m), p, s),
            Space::with_width(3),
            small_unit("MHz".into(), accent, s),
        ].align_y(iced::Alignment::End).into(),
        None => Space::with_height(0).into(),
    };

    tile_container(column![
        header("CPU".into(), p, s),
        sub_header(name, p, s),
        Space::with_height(Length::Fill),
        container(primary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
        container(secondary).width(Length::Fill).center_x(Length::Fill),
    ], p, w, s)
}

pub fn gpu_tile<'a>(gpu: &GpuData, s: &AppSettings, p: Palette, w: WarnView) -> Element<'a, Message> {
    let accent = w.accent_override.unwrap_or(p.accent);
    let name = if !s.gpu_custom_name.is_empty() {
        s.gpu_custom_name.clone()
    } else {
        let n = fmt::shorten(&gpu.name);
        if n.is_empty() { "GPU".to_string() } else { n }
    };

    let mut primary = row![].align_y(iced::Alignment::End);
    if let Some((tv, tu)) = fmt::fmt_temp(gpu.temperature_c, s) {
        primary = primary.push(big(tv, p, s)).push(unit_inline(tu, accent, s)).push(Space::with_width(8));
    }
    primary = primary
        .push(big(format!("{:.0}", gpu.usage_percent), p, s))
        .push(unit_inline("%".into(), accent, s));

    let mut sec = column![].spacing(1).align_x(iced::Alignment::Center);
    if let Some(m) = gpu.clock_mhz {
        sec = sec.push(
            row![
                small(format!("{:.0}", m), p, s),
                Space::with_width(3),
                small_unit("MHz".into(), accent, s),
            ].align_y(iced::Alignment::End)
        );
    }
    if gpu.vram_used_mb > 0.0 && gpu.vram_total_mb > 0.0 {
        sec = sec.push(
            row![
                small(format!("{:.1}/{:.1}", gpu.vram_used_mb / 1024.0, gpu.vram_total_mb / 1024.0), p, s),
                Space::with_width(3),
                small_unit("GB".into(), accent, s),
            ].align_y(iced::Alignment::End)
        );
    }

    tile_container(column![
        header("GPU".into(), p, s),
        sub_header(name, p, s),
        Space::with_height(Length::Fill),
        container(primary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
        container(sec).width(Length::Fill).center_x(Length::Fill),
    ], p, w, s)
}

pub fn ram_tile<'a>(ram: &RamData, s: &AppSettings, p: Palette, w: WarnView) -> Element<'a, Message> {
    let accent = w.accent_override.unwrap_or(p.accent);
    let used_gb = ram.used_mb / 1024.0;
    let total_gb = ram.total_mb / 1024.0;

    // C# RAM: PrimaryValue "17.4", PrimaryUnit "GB" (12px slot),
    // Secondary "27% of 64.0 GB".
    let primary = row![
        big(format!("{:.1}", used_gb), p, s),
        Space::with_width(4),
        unit("GB".into(), accent, s),
    ].align_y(iced::Alignment::End);

    let secondary = row![
        small(format!("{:.0}", ram.usage_percent), p, s),
        small_unit("%".into(), accent, s),
        small(format!(" of {:.1}", total_gb), p, s),
        Space::with_width(3),
        small_unit("GB".into(), accent, s),
    ].align_y(iced::Alignment::End);

    tile_container(column![
        header("RAM".into(), p, s),
        Space::with_height(Length::Fill),
        container(primary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
        container(secondary).width(Length::Fill).center_x(Length::Fill),
    ], p, w, s)
}

pub fn disk_tile<'a>(disk: &DiskData, s: &AppSettings, p: Palette, w: WarnView) -> Element<'a, Message> {
    let accent = w.accent_override.unwrap_or(p.accent);
    let selected = disk.drives.iter()
        .find(|d| d.mount.trim_end_matches('\\').eq_ignore_ascii_case(s.selected_disk_mount.trim_end_matches('\\')))
        .or_else(|| disk.drives.first());
    let (read, write) = selected
        .map(|d| (d.read_bytes_sec as f64, d.write_bytes_sec as f64))
        .unwrap_or((0.0, 0.0));

    // SubHeader honours DiskLabelStyle (Letter / Model / Both), like C#.
    let letters = selected.map(|d| d.mount.trim_end_matches('\\').to_string()).unwrap_or_default();
    let model = selected.map(|d| d.name.clone()).unwrap_or_default();
    let mount = match s.disk_label_style.as_str() {
        "Model" => model,
        "Both" => {
            if !letters.is_empty() && !model.is_empty() { format!("{} \u{00B7} {}", letters, model) }
            else if !letters.is_empty() { letters } else { model }
        }
        "None" => String::new(),
        _ => letters, // "Letter"
    };
    let (rv, ru) = fmt::fmt_disk(read);
    let (wv, wu) = fmt::fmt_disk(write);

    // R: / W: labels: DiskLabelFontSize = max(8, 13 + indicatorOffset + diskLabelOffset).
    let label_size = sz(13, s.indicator_font_offset + s.disk_label_font_offset, s);
    let spacing = s.disk_label_spacing.max(0.0);

    let lines = column![
        row![
            text("R:".to_string()).size(label_size)
                .font(named_font(&s.indicator_font, Weight::Bold))
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            Space::with_width(spacing),
            line_value(rv, ru, p, accent, s),
        ].align_y(iced::Alignment::Center),
        row![
            text("W:".to_string()).size(label_size)
                .font(named_font(&s.indicator_font, Weight::Bold))
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            Space::with_width(spacing),
            line_value(wv, wu, p, accent, s),
        ].align_y(iced::Alignment::Center),
    ]
    .spacing(4)
    .align_x(iced::Alignment::Center);

    tile_container(column![
        header("Disk".into(), p, s),
        sub_header(mount, p, s),
        Space::with_height(Length::Fill),
        container(lines).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
    ], p, w, s)
}

pub fn network_tile<'a>(net: &NetworkData, s: &AppSettings, p: Palette, w: WarnView, pulse: f32) -> Element<'a, Message> {
    let accent = w.accent_override.unwrap_or(p.accent);
    let sel = &s.network_adapter_name;
    let (down, up): (u64, u64) = if sel.is_empty() {
        (
            net.interfaces.iter().map(|i| i.download_bytes_sec).sum(),
            net.interfaces.iter().map(|i| i.upload_bytes_sec).sum(),
        )
    } else {
        net.interfaces.iter()
            .find(|i| &i.name == sel)
            .map(|i| (i.download_bytes_sec, i.upload_bytes_sec))
            .unwrap_or((0, 0))
    };
    let label = if sel.is_empty() { "All adapters".to_string() } else { sel.clone() };
    let (dv, du) = fmt::fmt_net(down as f64);
    let (uv, uu) = fmt::fmt_net(up as f64);

    // P6: animated traffic indicator. "Off" = static muted arrows. Other modes
    // colour active arrows with the accent and pulse their opacity.
    let indicator_on = s.network_traffic_indicator != "Off";
    let arrow_color = |active: bool| -> Color {
        if indicator_on && active {
            Color { a: accent.a * pulse.clamp(0.0, 1.0), ..accent }
        } else {
            p.muted
        }
    };
    let down_color = arrow_color(down > 0);
    let up_color = arrow_color(up > 0);

    // ArrowFontSize = 16 + indicatorOffset + arrowOffset.
    let arrow_size = sz(16, s.indicator_font_offset + s.arrow_font_offset, s);
    let spacing = s.network_arrow_spacing.max(0.0);

    // Glow mode: static accent arrow with a soft halo (container shadow).
    let glow = s.network_traffic_indicator == "Glow";
    let arrow_el = |glyph: &str, active: bool, col: Color| -> Element<'a, Message> {
        let t = text(glyph.to_string()).size(arrow_size)
            .font(named_font(&s.indicator_font, Weight::Bold))
            .style(move |_| iced::widget::text::Style { color: Some(col) });
        if glow && active {
            // Soft diffuse halo (no hard box): low opacity + wide blur.
            container(t)
                .style(move |_| iced::widget::container::Style {
                    shadow: iced::Shadow {
                        color: Color { a: 0.45, ..accent },
                        offset: iced::Vector::new(0.0, 0.0),
                        blur_radius: 24.0,
                    },
                    ..Default::default()
                })
                .into()
        } else {
            t.into()
        }
    };

    let lines = column![
        row![
            arrow_el("\u{2193}", down > 0, down_color),
            Space::with_width(spacing),
            line_value(dv, du, p, accent, s),
        ].align_y(iced::Alignment::Center),
        row![
            arrow_el("\u{2191}", up > 0, up_color),
            Space::with_width(spacing),
            line_value(uv, uu, p, accent, s),
        ].align_y(iced::Alignment::Center),
    ]
    .spacing(4)
    .align_x(iced::Alignment::Center);

    tile_container(column![
        header("Network".into(), p, s),
        sub_header(label, p, s),
        Space::with_height(Length::Fill),
        container(lines).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
    ], p, w, s)
}

pub fn clock_tile<'a>(s: &AppSettings, p: Palette, w: WarnView) -> Element<'a, Message> {
    let accent = w.accent_override.unwrap_or(p.accent);
    let now = chrono::Local::now();
    // 12-hour, non-padded (chrono has no %-I flag, compute it).
    let h24 = now.hour();
    let h12 = { let x = h24 % 12; if x == 0 { 12 } else { x } };
    let time = format!("{}:{:02}", h12, now.minute());
    let ampm = if h24 < 12 { "am" } else { "pm" };
    let day_n = now.day();
    let suffix = match day_n % 100 {
        11..=13 => "th",
        _ => match day_n % 10 { 1 => "st", 2 => "nd", 3 => "rd", _ => "th" },
    };
    let weekday = now.format("%A").to_string();
    let month = now.format("%B").to_string();

    let primary = row![
        big(time, p, s),
        Space::with_width(4),
        unit(ampm.to_string(), accent, s),
    ].align_y(iced::Alignment::End);

    // ClockDateFontSize = 13 + secondaryOffset; day number is accent.
    let date_size = sz(13, s.secondary_font_offset, s);
    let dc = text085(p);
    let date_text = |t: String| text(t).size(date_size)
        .font(named_font(&s.secondary_font, Weight::Normal))
        .style(move |_| iced::widget::text::Style { color: Some(dc) });
    let secondary = column![
        date_text(format!("{},", weekday)),
        row![
            date_text(format!("{} ", month)),
            text(day_n.to_string()).size(date_size)
                .font(named_font(&s.secondary_font, Weight::Normal))
                .style(move |_| iced::widget::text::Style { color: Some(accent) }),
            date_text(suffix.to_string()),
        ],
    ].align_x(iced::Alignment::Center);

    tile_container(column![
        header("Clock".into(), p, s),
        Space::with_height(Length::Fill),
        container(primary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(4),
        container(secondary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
    ], p, w, s)
}

fn tile_container<'a>(content: impl Into<Element<'a, Message>>, p: Palette, w: WarnView, s: &AppSettings) -> Element<'a, Message> {
    let skin = crate::style::skin_style(&s.active_skin);
    let bg = if w.flash {
        Color::from_rgb(1.0, 0.2, 0.2)
    } else {
        p.tile
    };
    let border_color = skin.border_color(&p);
    let tw = s.tile_width * s.ui_scale;
    let th = s.tile_height * s.ui_scale;

    let inner = content.into();

    // Accent bar (Sharp/Neon/Cyberpunk/Holographic): colored bar on the left
    let body: Element<'a, Message> = if skin.accent_bar > 0.0 {
        let bar_w = skin.accent_bar;
        let accent = p.accent;
        row![
            container(Space::new(bar_w, Length::Fill))
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(accent)),
                    ..Default::default()
                }),
            container(inner).width(Length::Fill).height(Length::Fill).padding([6, 8]),
        ].into()
    } else if skin.header_bar > 0.0 {
        // Header bar (Retro): colored bar at the top
        let bar_h = skin.header_bar;
        let accent = p.accent;
        column![
            container(Space::new(Length::Fill, bar_h))
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(Color { a: 0.6, ..accent })),
                    ..Default::default()
                }),
            container(inner).width(Length::Fill).height(Length::Fill).padding([4, 10]),
        ].into()
    } else {
        container(inner).width(Length::Fill).height(Length::Fill).padding([8, 10]).into()
    };

    // Sheen overlay: subtle white-to-transparent gradient effect via lighter tile bg
    let tile_bg = if skin.sheen > 0.0 {
        Color {
            r: bg.r + (1.0 - bg.r) * skin.sheen * 0.5,
            g: bg.g + (1.0 - bg.g) * skin.sheen * 0.5,
            b: bg.b + (1.0 - bg.b) * skin.sheen * 0.5,
            a: bg.a,
        }
    } else {
        bg
    };

    // Subtle drop shadow for depth (P10). Skins that draw their own hard edges
    // (Brutalist / Terminal / Sharp / Ink) opt out for a flatter look.
    let shadow = if matches!(s.active_skin.as_str(), "Brutalist" | "Terminal" | "Sharp" | "Ink" | "Minimal") {
        iced::Shadow::default()
    } else {
        iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 6.0,
        }
    };

    container(body)
        .width(Length::Fixed(tw))
        .height(Length::Fixed(th))
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(tile_bg)),
            border: Border {
                radius: skin.tile_radius.into(),
                width: skin.tile_border,
                color: border_color,
            },
            shadow,
            ..Default::default()
        })
        .into()
}
