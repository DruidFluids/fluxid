//! Per-metric tile rendering (CPU/GPU/RAM/Disk/Network/Clock).

use fluid_core::sensor_data::*;
use fluid_core::settings::AppSettings;
use iced::widget::{button, column, container, row, text, Space};
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
            .wrapping(iced::widget::text::Wrapping::None)
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        Space::with_width(3),
        text(u).size(sz(12, s.primary_font_offset, s))
            .font(named_font(&s.indicator_font, Weight::Semibold))
            .wrapping(iced::widget::text::Wrapping::None)
            .style(move |_| iced::widget::text::Style { color: Some(accent) }),
    ]
    .align_y(iced::Alignment::End)
    .into()
}

pub fn cpu_tile<'a>(cpu: &CpuData, s: &AppSettings, p: Palette, w: WarnView, driver_installed: bool) -> Element<'a, Message> {
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

    // C# v1.25 "turn on temperature" affordance: when there's no real reading
    // and the optional sensor driver isn't installed, the tile offers to enable
    // it (with a small dismiss). Replaces the clock line while shown.
    let show_hint = cpu.temperature_c.is_none() && !driver_installed && !s.cpu_temp_hint_dismissed;
    let secondary: Element<'a, Message> = if show_hint {
        cpu_temp_hint(p, s)
    } else {
        match cpu.clock_mhz {
            Some(m) => row![
                small(format!("{:.0}", m), p, s),
                Space::with_width(3),
                small_unit("MHz".into(), accent, s),
            ].align_y(iced::Alignment::End).into(),
            None => Space::with_height(0).into(),
        }
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

// The CPU-tile "turn on temperature" nudge: a clickable label that opens the
// sensor-driver dialog, plus a small "×" to dismiss it. Sized like the
// secondary line so it fits the tile's reserved bottom slot.
fn cpu_temp_hint<'a>(p: Palette, s: &AppSettings) -> Element<'a, Message> {
    let fs = sz(11, s.secondary_font_offset, s);
    // The label fills the bar so the whole bar is the click target; the dismiss
    // "x" sits flush at the right edge.
    let enable = button(
        container(
            text("Turn on temp").size(fs)
                .font(named_font(&s.secondary_font, Weight::Normal))
                .wrapping(iced::widget::text::Wrapping::None)
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) })
        ).width(Length::Fill).center_x(Length::Fill)
    )
    .width(Length::Fill)
    .padding(0)
    .style(|_: &iced::Theme, _: button::Status| button::Style { background: None, ..Default::default() })
    .on_press(Message::OpenCpuDriver);

    let dismiss = button(
        text("\u{2715}").size(fs).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    )
    .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 4.0 })
    .style(move |_: &iced::Theme, status: button::Status| {
        let hover = matches!(status, button::Status::Hovered);
        button::Style {
            background: None,
            text_color: if hover { p.accent } else { p.muted },
            ..Default::default()
        }
    })
    .on_press(Message::DismissCpuTempHint);

    // Clean bar across the tile (subtle accent fill, rounded), matching the C#
    // affordance. The leading spacer balances the trailing "x" so the label
    // stays visually centred.
    container(
        row![Space::with_width(14), enable, dismiss]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
    )
    .width(Length::Fill)
    .padding(iced::Padding { top: 2.0, right: 4.0, bottom: 2.0, left: 4.0 })
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color { a: 0.14, ..p.accent })),
        border: Border { radius: 5.0.into(), ..Border::default() },
        ..Default::default()
    })
    .into()
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

    // Subheader: RAM type + rated speed, e.g. "DDR5-6000" (C# behaviour).
    let ram_label = if ram.speed_mhz > 0 {
        if !ram.mem_type.is_empty() { format!("{}-{}", ram.mem_type, ram.speed_mhz) }
        else { format!("{} MHz", ram.speed_mhz) }
    } else {
        String::new()
    };

    tile_container(column![
        header("RAM".into(), p, s),
        sub_header(ram_label, p, s),
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

    // Static layout: fixed label column + fixed-width left-aligned value column.
    // Nothing shifts when R/W change digit count; the gap is the spacing slider.
    // Fixed label column (28) + fixed wide value column (left-aligned), shared
    // with the Network tile so values line up across tiles, never wrap, and
    // never jump. The gap is the spacing slider.
    let col_w = Length::Fixed(30.0);
    let value_w = Length::Fill;
    let dline = |lbl: &str, v: String, u: String| -> Element<'a, Message> {
        row![
            container(text(lbl.to_string()).size(label_size)
                .font(named_font(&s.indicator_font, Weight::Bold))
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .width(col_w).align_x(iced::alignment::Horizontal::Right),
            Space::with_width(spacing),
            container(line_value(v, u, p, accent, s)).width(value_w).align_x(iced::alignment::Horizontal::Left),
        ].align_y(iced::Alignment::Center).into()
    };
    let lines = column![
        dline("R:", rv, ru),
        dline("W:", wv, wu),
    ]
    .spacing(4);

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

    // Traffic indicator. Off = static muted arrows. Blink/Fade pulse the accent
    // opacity; Glow is a static brighter (lit) accent — no halo box.
    let indicator_on = s.network_traffic_indicator != "Off";
    let glow = s.network_traffic_indicator == "Glow";
    let arrow_color = |active: bool| -> Color {
        if !indicator_on || !active {
            p.muted
        } else if glow {
            accent
        } else {
            Color { a: accent.a * pulse.clamp(0.0, 1.0), ..accent }
        }
    };
    let down_color = arrow_color(down > 0);
    let up_color = arrow_color(up > 0);

    // ArrowFontSize = 16 + indicatorOffset + arrowOffset.
    let arrow_size = sz(16, s.indicator_font_offset + s.arrow_font_offset, s);
    let spacing = s.network_arrow_spacing.max(0.0);
    let accent_hex = format!("#{:02X}{:02X}{:02X}",
        (accent.r * 255.0).round() as u8, (accent.g * 255.0).round() as u8, (accent.b * 255.0).round() as u8);
    // The glow SVG arrow spans ~17/32 of its box, so this makes the rendered
    // arrow match the text-glyph arrow size — identical size in every mode.
    // Every mode renders the SAME SVG arrow at the SAME size (so Off / Blink /
    // Fade / Glow are all identical geometry) — only the colour and, in Glow
    // mode, the bloom/halo layers change. This keeps up/down arrows and all
    // indicator modes pixel-identical.
    let glow_w = (arrow_size as f32) * 1.85;
    let col_w = Length::Fixed(glow_w);

    let nline = |down_dir: bool, active: bool, col: Color, v: String, u: String| -> Element<'a, Message> {
        let d = if down_dir { "M16 7 V24 M9 16 L16 24 L23 16" } else { "M16 25 V8 M9 16 L16 8 L23 16" };
        let body_hex = format!("#{:02X}{:02X}{:02X}",
            (col.r * 255.0).round() as u8, (col.g * 255.0).round() as u8, (col.b * 255.0).round() as u8);
        let mut layers = String::new();
        if glow && active {
            // Behind the body: radial bloom + a blurred accent stroke.
            layers.push_str(&format!(
                "<circle cx=\"16\" cy=\"16\" r=\"16\" fill=\"url(#h)\"/>\
                 <path d=\"{d}\" stroke=\"{a}\" stroke-width=\"3.4\" opacity=\"0.9\" filter=\"url(#b)\"/>",
                d = d, a = accent_hex));
        }
        // Solid body stroke — present in every mode, identical thickness.
        layers.push_str(&format!(
            "<path d=\"{d}\" stroke=\"{c}\" stroke-width=\"2.2\" opacity=\"{op:.3}\"/>",
            d = d, c = body_hex, op = col.a));
        if glow && active {
            // White-hot core on top for the luminous-tube look.
            layers.push_str(&format!("<path d=\"{d}\" stroke=\"#EAF5FF\" stroke-width=\"1.1\" opacity=\"0.95\"/>", d = d));
        }
        let svg_str = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"32\" height=\"32\" viewBox=\"0 0 32 32\">\
             <defs>\
             <radialGradient id=\"h\" cx=\"50%\" cy=\"50%\" r=\"50%\">\
             <stop offset=\"0%\" stop-color=\"{a}\" stop-opacity=\"0.65\"/>\
             <stop offset=\"55%\" stop-color=\"{a}\" stop-opacity=\"0.18\"/>\
             <stop offset=\"100%\" stop-color=\"{a}\" stop-opacity=\"0\"/>\
             </radialGradient>\
             <filter id=\"b\" x=\"-60%\" y=\"-60%\" width=\"220%\" height=\"220%\"><feGaussianBlur stdDeviation=\"1.6\"/></filter>\
             </defs>\
             <g fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{layers}</g></svg>",
            a = accent_hex, layers = layers);
        let arrow: Element<'a, Message> = iced::widget::svg(iced::widget::svg::Handle::from_memory(svg_str.into_bytes()))
            .width(Length::Fixed(glow_w)).height(Length::Fixed(glow_w))
            .style(|_t, _s| iced::widget::svg::Style { color: None })
            .into();
        // Value sizes to its content (no fill column squeezing it), so the unit
        // never wraps; the fixed arrow column keeps up/down arrows aligned.
        row![
            container(arrow).width(col_w).align_x(iced::alignment::Horizontal::Center),
            Space::with_width(spacing),
            line_value(v, u, p, accent, s),
        ].align_y(iced::Alignment::Center).into()
    };
    let lines = column![
        nline(true, down > 0, down_color, dv, du),
        nline(false, up > 0, up_color, uv, uu),
    ]
    .spacing(4);

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
