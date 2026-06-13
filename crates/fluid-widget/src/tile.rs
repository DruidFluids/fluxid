use fluid_core::sensor_data::*;
use fluid_core::settings::AppSettings;
use iced::widget::{column, container, row, text, Space};
use iced::{Border, Element, Length};
use crate::fmt;
use crate::style::Palette;
use crate::Message;

#[derive(Debug, Clone, Copy, Default)]
pub struct WarnView {
    pub flash: bool,
    pub accent_override: Option<iced::Color>,
}

fn sz(base: i32, offset: i32, s: &AppSettings) -> u16 {
    (((base + offset).max(7)) as f32 * s.ui_scale).round().max(7.0) as u16
}

use iced::font::Weight;
use crate::style::named_font;

fn header<'a>(label: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    container(
        text(label).size(sz(13, s.secondary_font_offset, s))
            .font(named_font(&s.secondary_font, Weight::Semibold))
            .style(move |_| iced::widget::text::Style { color: Some(p.text) })
    ).width(Length::Fill).center_x(Length::Fill).into()
}

fn sub_header<'a>(label: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    container(
        text(label).size(sz(11, s.secondary_font_offset, s))
            .font(named_font(&s.secondary_font, Weight::Normal))
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).width(Length::Fill).center_x(Length::Fill).into()
}

fn big<'a>(t: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(18, s.primary_font_offset, s))
        .font(named_font(&s.primary_font, Weight::Bold))
        .style(move |_| iced::widget::text::Style { color: Some(p.text) })
        .into()
}

fn unit<'a>(t: String, accent: iced::Color, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(12, s.indicator_font_offset, s))
        .font(named_font(&s.indicator_font, Weight::Bold))
        .style(move |_| iced::widget::text::Style { color: Some(accent) })
        .into()
}

fn small<'a>(t: String, p: Palette, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(11, s.secondary_font_offset, s))
        .font(named_font(&s.secondary_font, Weight::Normal))
        .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
        .into()
}

fn small_unit<'a>(t: String, accent: iced::Color, s: &AppSettings) -> Element<'a, Message> {
    text(t).size(sz(9, s.indicator_font_offset, s))
        .font(named_font(&s.indicator_font, Weight::Normal))
        .style(move |_| iced::widget::text::Style { color: Some(accent) })
        .into()
}

fn line_value<'a>(v: String, u: String, p: Palette, accent: iced::Color, s: &AppSettings) -> Element<'a, Message> {
    row![
        text(v).size(sz(14, s.primary_font_offset, s))
            .font(named_font(&s.primary_font, Weight::Bold))
            .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        Space::with_width(3),
        text(u).size(sz(9, s.indicator_font_offset, s))
            .font(named_font(&s.indicator_font, Weight::Bold))
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

    let mut primary = row![].spacing(4).align_y(iced::Alignment::End);
    if let Some((tv, tu)) = fmt::fmt_temp(cpu.temperature_c, s) {
        primary = primary.push(big(tv, p, s)).push(unit(tu, accent, s)).push(Space::with_width(6));
    }
    primary = primary
        .push(big(format!("{:.0}", cpu.usage_percent), p, s))
        .push(unit("%".into(), accent, s));

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

    let mut primary = row![].spacing(4).align_y(iced::Alignment::End);
    if let Some((tv, tu)) = fmt::fmt_temp(gpu.temperature_c, s) {
        primary = primary.push(big(tv, p, s)).push(unit(tu, accent, s)).push(Space::with_width(6));
    }
    primary = primary
        .push(big(format!("{:.0}", gpu.usage_percent), p, s))
        .push(unit("%".into(), accent, s));

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
    let mount = if s.disk_label_style == "None" {
        String::new()
    } else {
        selected.map(|d| d.mount.trim_end_matches('\\').to_string()).unwrap_or_default()
    };
    let (rv, ru) = fmt::fmt_disk(read);
    let (wv, wu) = fmt::fmt_disk(write);

    let label_size = sz(14, s.indicator_font_offset + s.disk_label_font_offset, s);
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
    .align_x(iced::Alignment::Start);

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
    // colour active arrows with the accent and pulse their opacity (pulse is the
    // 0..1 multiplier from the widget's sine-driven animation clock).
    let indicator_on = s.network_traffic_indicator != "Off";
    let arrow_color = |active: bool| -> iced::Color {
        if indicator_on && active {
            iced::Color { a: accent.a * pulse.clamp(0.0, 1.0), ..accent }
        } else {
            p.muted
        }
    };
    let down_color = arrow_color(down > 0);
    let up_color = arrow_color(up > 0);

    let arrow_size = sz(15, s.indicator_font_offset + s.arrow_font_offset, s);
    let spacing = s.network_arrow_spacing.max(0.0);

    let lines = column![
        row![
            text("\u{2193}".to_string()).size(arrow_size)
                .font(named_font(&s.indicator_font, Weight::Bold))
                .style(move |_| iced::widget::text::Style { color: Some(down_color) }),
            Space::with_width(spacing),
            line_value(dv, du, p, accent, s),
        ].align_y(iced::Alignment::Center),
        row![
            text("\u{2191}".to_string()).size(arrow_size)
                .font(named_font(&s.indicator_font, Weight::Bold))
                .style(move |_| iced::widget::text::Style { color: Some(up_color) }),
            Space::with_width(spacing),
            line_value(uv, uu, p, accent, s),
        ].align_y(iced::Alignment::Center),
    ]
    .spacing(4)
    .align_x(iced::Alignment::Start);

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
    let time = now.format("%-I:%M").to_string();
    let ampm = now.format("%p").to_string().to_lowercase();
    let day = now.format("%-d").to_string();
    let day_n: u32 = day.parse().unwrap_or(1);
    let suffix = match day_n % 100 {
        11..=13 => "th",
        _ => match day_n % 10 { 1 => "st", 2 => "nd", 3 => "rd", _ => "th" },
    };
    let weekday = now.format("%A").to_string();
    let month = now.format("%B").to_string();

    let primary = row![
        big(time, p, s),
        Space::with_width(4),
        unit(ampm, accent, s),
    ].align_y(iced::Alignment::End);

    let secondary = column![
        small(format!("{},", weekday), p, s),
        row![
            small(format!("{} ", month), p, s),
            text(day).size(sz(11, s.secondary_font_offset, s))
                .style(move |_| iced::widget::text::Style { color: Some(accent) }),
            small(suffix.to_string(), p, s),
        ],
    ].align_x(iced::Alignment::Center);

    tile_container(column![
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
        iced::Color::from_rgb(1.0, 0.2, 0.2)
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
                    background: Some(iced::Background::Color(iced::Color { a: 0.6, ..accent })),
                    ..Default::default()
                }),
            container(inner).width(Length::Fill).height(Length::Fill).padding([4, 10]),
        ].into()
    } else {
        container(inner).width(Length::Fill).height(Length::Fill).padding([8, 10]).into()
    };

    // Sheen overlay: subtle white-to-transparent gradient effect via lighter tile bg
    let tile_bg = if skin.sheen > 0.0 {
        iced::Color {
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
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.28),
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

