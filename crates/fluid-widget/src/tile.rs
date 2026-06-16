//! Per-metric tile rendering (CPU/GPU/RAM/Disk/Network/Clock).

use fluid_core::sensor_data::*;
use fluid_core::settings::AppSettings;
use iced::widget::{button, column, container, row, stack, text, Space};
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

// Percentage readout, guarded against non-finite sensor values: a NaN/Inf
// usage would otherwise render as "NaN"/"inf" and blow out the tile layout.
fn pct(v: f32) -> String {
    format!("{:.0}", if v.is_finite() { v.clamp(0.0, 999.0) } else { 0.0 })
}

// ── Tile text roles (C# Theme.xaml styles) ──────────────────────────────────
// Header: IndicatorFontSize (16+indicatorOffset), Bold, muted, secondary font.
fn header<'a>(label: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    container(
        text(label).size(sz(16, s.indicator_font_offset, s))
            .font(named_font(&s.secondary_font, Weight::Bold))
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).width(Length::Fill).center_x(Length::Fill).into()
}

// Truncate a tile subheader to roughly the tile's inner width (at the subheader
// font), appending an ellipsis. Used for disk model / network adapter names so a
// long "Model · C:" or adapter name cuts off cleanly on the right instead of
// wrapping or clipping mid-glyph. Width-estimate only; errs on the short side.
fn fit_name(name: String, s: &AppSettings) -> String {
    let fs = sz(11, s.secondary_font_offset, s) as f32;
    let avail = (s.tile_width * s.ui_scale - 18.0).max(24.0);
    let max_chars = ((avail / (fs * 0.56)).floor() as usize).max(4);
    if name.chars().count() <= max_chars {
        return name;
    }
    let t: String = name.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}\u{2026}", t.trim_end())
}

// One subheader line's reserved height — keeps the subheader row the same height
// in every tile (even when a tile has no subheader text) so the primary value
// below it lands at the same vertical position across tiles.
fn sub_line_h(s: &AppSettings) -> f32 {
    sz(11, s.secondary_font_offset, s) as f32 * 1.45
}

// SubHeader: SecondaryFontSize (11+secondaryOffset), muted (0.8), secondary
// font. Always reserves one line of height (even when empty) so primary values
// stay aligned tile-to-tile.
fn sub_header<'a>(label: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    let content: Element<'a, Message> = if label.trim().is_empty() {
        Space::with_height(0).into()
    } else {
        text(label).size(sz(11, s.secondary_font_offset, s))
            .font(named_font(&s.secondary_font, Weight::Normal))
            // Clip long hardware / disk-model names to one line instead of
            // word-wrapping, which would grow into the fixed-height tile and
            // shove the centered value rows around.
            .wrapping(iced::widget::text::Wrapping::None)
            .style(move |_| iced::widget::text::Style { color: Some(Color { a: p.muted.a * 0.8, ..p.muted }) })
            .into()
    };
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(sub_line_h(s)))
        .center_x(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

// Wrap a tile's secondary content in a fixed two-line zone, top-aligned, so the
// primary value above it centers identically in every tile (a tile with one
// secondary line and one with two both reserve the same space) — this is what
// keeps CPU temp / GPU temp / RAM in line, especially horizontally.
fn secondary_zone<'a>(content: impl Into<Element<'a, Message>>, s: &AppSettings) -> Element<'a, Message> {
    let line = sz(13, s.secondary_font_offset, s) as f32 * 1.4;
    container(content.into())
        .width(Length::Fill)
        .height(Length::Fixed((line * 2.0).ceil()))
        .center_x(Length::Fill)
        .align_y(iced::alignment::Vertical::Top)
        .into()
}

// Build a tile whose body is a list of equal stat lines (Network ↓/↑, Disk R:/W:)
// using the SAME primary + secondary_zone layout as the value tiles: the first
// line takes the primary slot and the rest go in the secondary zone, so the
// first line lines up with CPU/GPU/RAM across the row (esp. horizontal).
fn stat_lines_body<'a>(
    title: String,
    sub: String,
    lines: Vec<Element<'a, Message>>,
    p: Palette,
    w: WarnView,
    s: &AppSettings,
) -> Element<'a, Message> {
    let mut it = lines.into_iter();
    let primary: Element<'a, Message> = it.next().unwrap_or_else(|| Space::with_height(0).into());
    let mut sec = column![].spacing(4);
    for l in it {
        sec = sec.push(l);
    }
    tile_container(
        column![
            header(title, p, s),
            sub_header(sub, p, s),
            Space::with_height(Length::Fill),
            container(primary).width(Length::Fill).center_x(Length::Fill),
            Space::with_height(Length::Fill),
            secondary_zone(sec, s),
        ],
        p,
        w,
        s,
    )
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

// One Network/Disk stat line: the NUMBER is pinned to the tile's centerline and
// grows symmetrically as its digit count changes. The two side cells are equal
// `Length::Fill`, so they always split the leftover width evenly — keeping the
// number dead-centre without measuring any text. The unit hugs the number on the
// right with a small fixed gap; the arrow/label sits on the left, nudged in from
// the edge by `left_inset` (the per-tile position slider) so the user can slide
// R:/W: (or the arrow) left or right without disturbing the centred number.
// Number at PrimaryFontSize (18+primaryOffset); unit at UnitFontSize accent.
#[allow(clippy::too_many_arguments)]
fn centered_stat_line<'a>(
    left: Element<'a, Message>, v: String, u: String,
    p: Palette, accent: Color, left_inset: f32, label_w: f32, s: &AppSettings,
) -> Element<'a, Message> {
    // Small symmetric gap on each side of the number keeps it centred and the
    // unit close, scaled with the UI so it tracks the font size.
    let gap = 4.0 * s.ui_scale;
    // Dynamic safety clamp on the inset. Each Fill side cell is
    //   fill = (tile_inner - 2*gap - widest_number) / 2
    // wide. If `inset + label_w` exceeds that, iced grows the Fill to fit its
    // content, which shoves the centred number off-centre and clips the unit
    // (verified empirically). So cap the inset so the label always fits inside
    // its half — keeping the number dead-centre for EVERY value, including the
    // widest 4-digit reading. Works for any tile size / UI scale / font.
    let prim = sz(18, s.primary_font_offset, s) as f32;
    let num_w = prim * 4.0 * 0.6; // worst case: 4 tabular digits, generous width
    let inner = (s.tile_width * s.ui_scale - 20.0).max(0.0); // tile inner width (h-padding ~20)
    let fill = ((inner - 2.0 * gap - num_w) * 0.5).max(0.0);
    let max_inset = (fill - label_w).max(0.0);
    let inset = left_inset.clamp(0.0, max_inset);
    let number = text(v).size(sz(18, s.primary_font_offset, s))
        .font(named_font(&s.primary_font, Weight::Bold))
        .wrapping(iced::widget::text::Wrapping::None)
        .style(move |_| iced::widget::text::Style { color: Some(p.text) });
    let unit = text(u).size(sz(12, s.primary_font_offset, s))
        .font(named_font(&s.indicator_font, Weight::Semibold))
        .wrapping(iced::widget::text::Wrapping::None)
        .style(move |_| iced::widget::text::Style { color: Some(accent) });
    row![
        // Left cell is Fill (so the number stays centred); the label/arrow is
        // left-aligned within it and pushed right by `inset` — the position slider.
        container(row![Space::with_width(inset), left].align_y(iced::Alignment::End))
            .width(Length::Fill).align_x(iced::alignment::Horizontal::Left),
        Space::with_width(gap),
        number,
        Space::with_width(gap),
        container(unit).width(Length::Fill).align_x(iced::alignment::Horizontal::Left),
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
    if s.cpu_show_temp {
        if let Some((tv, tu)) = fmt::fmt_temp(cpu.temperature_c, s) {
            primary = primary.push(big(tv, p, s)).push(unit_inline(tu, accent, s)).push(Space::with_width(8));
        }
    }
    primary = primary
        .push(big(pct(cpu.usage_percent), p, s))
        .push(unit_inline("%".into(), accent, s));

    // C# v1.25 "turn on temperature" affordance: when there's no real reading
    // and the optional sensor driver isn't installed, the tile offers to enable
    // it (with a small dismiss). Replaces the clock line while shown.
    let show_hint = s.cpu_show_temp && cpu.temperature_c.is_none() && !driver_installed && !s.cpu_temp_hint_dismissed;
    let secondary: Element<'a, Message> = if show_hint {
        cpu_temp_hint(p, s)
    } else if s.cpu_show_clock {
        match cpu.clock_mhz {
            Some(m) => row![
                small(format!("{:.0}", m), p, s),
                Space::with_width(3),
                small_unit("MHz".into(), accent, s),
            ].align_y(iced::Alignment::End).into(),
            None => Space::with_height(0).into(),
        }
    } else {
        Space::with_height(0).into()
    };

    tile_container(column![
        header("CPU".into(), p, s),
        sub_header(if s.cpu_show_name { name } else { String::new() }, p, s),
        Space::with_height(Length::Fill),
        container(primary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
        secondary_zone(secondary, s),
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
        row![
            Space::with_width(14),
            crate::style::with_tip(enable, "Set up accurate CPU temperature (installs the optional signed driver)", p),
            crate::style::with_tip(dismiss, "Dismiss this hint", p),
        ]
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
    if s.gpu_show_temp {
        if let Some((tv, tu)) = fmt::fmt_temp(gpu.temperature_c, s) {
            primary = primary.push(big(tv, p, s)).push(unit_inline(tu, accent, s)).push(Space::with_width(8));
        }
    }
    primary = primary
        .push(big(pct(gpu.usage_percent), p, s))
        .push(unit_inline("%".into(), accent, s));

    let mut sec = column![].spacing(1).align_x(iced::Alignment::Center);
    if s.gpu_show_clock {
        if let Some(m) = gpu.clock_mhz {
            sec = sec.push(
                row![
                    small(format!("{:.0}", m), p, s),
                    Space::with_width(3),
                    small_unit("MHz".into(), accent, s),
                ].align_y(iced::Alignment::End)
            );
        }
    }
    if s.gpu_show_vram && gpu.vram_used_mb > 0.0 && gpu.vram_total_mb > 0.0 {
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
        sub_header(if s.gpu_show_name { name } else { String::new() }, p, s),
        Space::with_height(Length::Fill),
        container(primary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
        secondary_zone(sec, s),
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

    let secondary: Element<'a, Message> = if s.ram_show_details {
        row![
            small(pct(ram.usage_percent), p, s),
            small_unit("%".into(), accent, s),
            small(format!(" of {:.1}", total_gb), p, s),
            Space::with_width(3),
            small_unit("GB".into(), accent, s),
        ].align_y(iced::Alignment::End).into()
    } else {
        Space::with_height(0).into()
    };

    // Subheader: RAM type + rated speed, e.g. "DDR5-6000" (C# behaviour).
    let ram_label = if s.ram_show_speed && ram.speed_mhz > 0 {
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
        secondary_zone(secondary, s),
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

    // Each R:/W: line centers its number on the tile centerline (see
    // centered_stat_line); the label hugs it on the left, the unit on the right,
    // and the value grows symmetrically about the centre as its digits change.
    let dline = |lbl: &str, v: String, u: String| -> Element<'a, Message> {
        let label: Element<'a, Message> = text(lbl.to_string()).size(label_size)
            .font(named_font(&s.indicator_font, Weight::Bold))
            .wrapping(iced::widget::text::Wrapping::None)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
            .into();
        // "R:" / "W:" are ~2 label-font glyphs wide.
        let label_w = label_size as f32 * 1.2;
        centered_stat_line(label, v, u, p, accent, spacing, label_w, s)
    };
    let mut lines: Vec<Element<'a, Message>> = Vec::new();
    if s.disk_show_read { lines.push(dline("R:", rv, ru)); }
    if s.disk_show_write { lines.push(dline("W:", wv, wu)); }

    stat_lines_body("Disk".into(), fit_name(mount, s), lines, p, w, s)
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
    let glow_w = (arrow_size as f32) * 1.4;

    let nline = |down_dir: bool, active: bool, col: Color, v: String, u: String| -> Element<'a, Message> {
        // Exact vertical mirrors about y=16, so up and down are pixel-identical.
        let d = if down_dir { "M16 8 V24 M9 16 L16 24 L23 16" } else { "M16 24 V8 M9 16 L16 8 L23 16" };
        let body_hex = format!("#{:02X}{:02X}{:02X}",
            (col.r * 255.0).round() as u8, (col.g * 255.0).round() as u8, (col.b * 255.0).round() as u8);
        let mut layers = String::new();
        if glow && active {
            // Behind the body: radial bloom + a blurred accent stroke.
            layers.push_str(&format!(
                "<circle cx=\"16\" cy=\"16\" r=\"16\" fill=\"url(#h)\"/>\
                 <path d=\"{d}\" stroke=\"{a}\" stroke-width=\"4.0\" opacity=\"0.9\" filter=\"url(#b)\"/>",
                d = d, a = accent_hex));
        }
        // Solid body stroke — present in every mode, identical thickness.
        layers.push_str(&format!(
            "<path d=\"{d}\" stroke=\"{c}\" stroke-width=\"2.8\" opacity=\"{op:.3}\"/>",
            d = d, c = body_hex, op = col.a));
        if glow && active {
            // White-hot core on top for the luminous-tube look.
            layers.push_str(&format!("<path d=\"{d}\" stroke=\"#EAF5FF\" stroke-width=\"1.4\" opacity=\"0.95\"/>", d = d));
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
        // Center the number on the tile centerline; the arrow sits on the left
        // (nudged by the position slider) and the unit on the right. The arrow's
        // glow box (glow_w) is its layout width — the value the inset must clear.
        centered_stat_line(arrow, v, u, p, accent, spacing, glow_w, s)
    };
    // Each line carries its own direction arrow, so swapping the order also flips
    // the arrows for free — download(↓)/upload(↑) stay correct in either layout.
    let down_line = |c: Color, v: String, u: String| nline(true, down > 0, c, v, u);
    let up_line = |c: Color, v: String, u: String| nline(false, up > 0, c, v, u);
    let mut lines: Vec<Element<'a, Message>> = Vec::new();
    if s.net_upload_first {
        if s.net_show_up { lines.push(up_line(up_color, uv, uu)); }
        if s.net_show_down { lines.push(down_line(down_color, dv, du)); }
    } else {
        if s.net_show_down { lines.push(down_line(down_color, dv, du)); }
        if s.net_show_up { lines.push(up_line(up_color, uv, uu)); }
    }

    stat_lines_body("Network".into(), fit_name(label, s), lines, p, w, s)
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
    let secondary: Element<'a, Message> = if s.clock_show_date {
        column![
            date_text(format!("{},", weekday)),
            row![
                date_text(format!("{} ", month)),
                text(day_n.to_string()).size(date_size)
                    .font(named_font(&s.secondary_font, Weight::Normal))
                    .style(move |_| iced::widget::text::Style { color: Some(accent) }),
                date_text(suffix.to_string()),
            ],
        ].align_x(iced::Alignment::Center).into()
    } else {
        Space::with_height(0).into()
    };

    tile_container(column![
        header("Clock".into(), p, s),
        // Empty (reserved) subheader so the time lines up with the other tiles'
        // primary values across the row.
        sub_header(String::new(), p, s),
        Space::with_height(Length::Fill),
        container(primary).width(Length::Fill).center_x(Length::Fill),
        Space::with_height(Length::Fill),
        secondary_zone(secondary, s),
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

    // Shadow: bold skins get an accent-tinted outer GLOW; the hard-edged flat
    // skins opt out; everyone else gets a subtle drop shadow for depth.
    let shadow = if skin.glow > 0.0 {
        iced::Shadow {
            color: Color { a: 0.60 * skin.glow, ..p.accent },
            offset: iced::Vector::new(0.0, 0.0),
            blur_radius: 6.0 + skin.glow * 16.0,
        }
    } else if matches!(s.active_skin.as_str(), "Brutalist" | "Terminal" | "Sharp" | "Ink" | "Minimal") {
        iced::Shadow::default()
    } else {
        iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 6.0,
        }
    };

    // Background: gradient skins fade from a lighter top to an accent-tinted
    // bottom; everyone else is a flat fill. (Flashing alerts stay solid red.)
    let bg_fill: iced::Background = if skin.gradient > 0.0 && !w.flash {
        let top = crate::style::lerp(tile_bg, Color::WHITE, skin.gradient * 0.10);
        let bottom = crate::style::lerp(tile_bg, p.accent, skin.gradient * 0.22);
        let g = iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
            .add_stop(0.0, top)
            .add_stop(1.0, bottom);
        iced::Background::Gradient(iced::Gradient::Linear(g))
    } else {
        iced::Background::Color(tile_bg)
    };

    let tile_el = container(body)
        .width(Length::Fixed(tw))
        .height(Length::Fixed(th))
        .style(move |_| iced::widget::container::Style {
            background: Some(bg_fill),
            border: Border {
                radius: skin.tile_radius.into(),
                width: skin.tile_border,
                color: border_color,
            },
            shadow,
            ..Default::default()
        });

    // Texture overlay (scanlines / grid) for skins that want it — a subtle,
    // non-interactive SVG pattern clipped to the tile's rounded rect.
    let texture: u8 = match s.active_skin.as_str() {
        "Terminal" | "Retro" => 1, // scanlines
        "Cyberpunk" => 2,          // grid
        _ => 0,
    };
    if texture == 0 || w.flash {
        return tile_el.into();
    }
    let r = skin.tile_radius;
    let hexof = |c: Color| format!("#{:02X}{:02X}{:02X}", (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8);
    let pat = if texture == 1 {
        // Horizontal scanlines every 3px.
        format!("<pattern id=\"t\" width=\"{tw}\" height=\"3\" patternUnits=\"userSpaceOnUse\">\
                 <rect x=\"0\" y=\"0\" width=\"{tw}\" height=\"1\" fill=\"{c}\" opacity=\"0.10\"/></pattern>",
            tw = tw, c = hexof(p.text))
    } else {
        // 8px grid.
        format!("<pattern id=\"t\" width=\"8\" height=\"8\" patternUnits=\"userSpaceOnUse\">\
                 <path d=\"M8 0 H0 V8\" fill=\"none\" stroke=\"{c}\" stroke-width=\"0.5\" opacity=\"0.18\"/></pattern>",
            c = hexof(p.accent))
    };
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{tw}\" height=\"{th}\" viewBox=\"0 0 {tw} {th}\">\
         <defs>{pat}</defs>\
         <rect x=\"0\" y=\"0\" width=\"{tw}\" height=\"{th}\" rx=\"{r}\" ry=\"{r}\" fill=\"url(#t)\"/></svg>",
        tw = tw, th = th, r = r, pat = pat);
    let overlay = iced::widget::svg(iced::widget::svg::Handle::from_memory(svg.into_bytes()))
        .width(Length::Fixed(tw)).height(Length::Fixed(th))
        .style(|_t, _s| iced::widget::svg::Style { color: None });
    stack![tile_el, overlay].width(Length::Fixed(tw)).height(Length::Fixed(th)).into()
}
