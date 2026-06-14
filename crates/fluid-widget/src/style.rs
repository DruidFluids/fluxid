//! Palette resolution, skins, theme presets, and shared iced widget styles.

use crate::Message;
use fluid_core::settings::AppSettings;
use iced::widget::{button, text};
use iced::{Border, Color, Element};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Field colour (dropdowns / inputs) derived from the theme background, so it
/// stays readable on both dark and light themes: dark themes get a clearly
/// darker field, light themes a subtly darker one (a muddy mid-tone from a flat
/// ×0.5 would be unreadable on light backgrounds).
pub fn field_bg(p: Palette) -> Color {
    let lum = 0.299 * p.bg.r + 0.587 * p.bg.g + 0.114 * p.bg.b;
    let f = if lum < 0.5 { 0.5 } else { 0.88 };
    Color { r: p.bg.r * f, g: p.bg.g * f, b: p.bg.b * f, a: 1.0 }
}

/// Themed slider style (accent rail + accent handle), matching the settings
/// sliders. For standalone `slider()` calls outside `marked_slider`.
pub fn slider_style(p: Palette) -> impl Fn(&iced::Theme, iced::widget::slider::Status) -> iced::widget::slider::Style + Copy {
    use iced::widget::slider::{Handle, HandleShape, Rail, Style};
    move |_t, _s| Style {
        rail: Rail {
            backgrounds: (
                iced::Background::Color(p.accent),
                iced::Background::Color(Color { a: p.muted.a * 0.25, ..p.muted }),
            ),
            width: 2.0,
            border: iced::Border { radius: 1.0.into(), width: 0.0, color: Color::TRANSPARENT },
        },
        handle: Handle {
            shape: HandleShape::Circle { radius: 6.0 },
            background: iced::Background::Color(p.accent),
            border_width: 2.0,
            border_color: p.bg,
        },
    }
}

/// C# `InlineBtn`: tile fill, 1px border, radius 6; hover accents text + border.
/// Auto-width (shrinks to its label). The single source of truth for the
/// inline-action buttons used across Settings and the popups.
pub fn inline_btn<'a>(label: impl Into<String>, msg: Message, p: Palette) -> Element<'a, Message> {
    let label = label.into();
    button(text(label).size(11))
        .padding(iced::Padding { top: 5.0, right: 12.0, bottom: 5.0, left: 12.0 })
        .style(move |_: &iced::Theme, status: button::Status| {
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: Some(iced::Background::Color(p.tile)),
                text_color: if hover { p.accent } else { p.text },
                border: Border { radius: 6.0.into(), width: 1.0, color: if hover { p.accent } else { p.muted } },
                ..Default::default()
            }
        })
        .on_press(msg)
        .into()
}

/// Wrap any control in the standard tooltip bubble. **Convention:** every
/// button in the app should be wrapped with this (or pass a tip to a helper that
/// calls it) so it always has a hint — including new buttons going forward.
pub fn with_tip<'a>(el: impl Into<Element<'a, Message>>, tip: &str, p: Palette) -> Element<'a, Message> {
    use iced::widget::{container, text as itext, tooltip};
    let bubble = container(
        itext(tip.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.text) })
    )
    .max_width(240)
    .padding(8)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(p.tile)),
        border: Border { radius: 6.0.into(), width: 1.0, color: Color { a: 0.4, ..p.muted } },
        ..Default::default()
    });
    tooltip(el, bubble, tooltip::Position::Top).into()
}

/// `inline_btn` with a tooltip — the preferred constructor for action buttons.
pub fn inline_btn_tip<'a>(label: impl Into<String>, msg: Message, tip: &str, p: Palette) -> Element<'a, Message> {
    with_tip(inline_btn(label, msg, p), tip, p)
}

/// Dark dropdown (pick_list) style.
pub fn pick_list_style(p: Palette) -> impl Fn(&iced::Theme, iced::widget::pick_list::Status) -> iced::widget::pick_list::Style + Copy {
    let bg = field_bg(p);
    move |_t, status| {
        let hover = matches!(status, iced::widget::pick_list::Status::Hovered | iced::widget::pick_list::Status::Opened);
        iced::widget::pick_list::Style {
            text_color: p.text,
            placeholder_color: p.muted,
            handle_color: p.muted,
            background: iced::Background::Color(bg),
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: if hover { p.accent } else { Color { a: 0.4, ..p.muted } },
            },
        }
    }
}

/// Dark text-input style (hotkey field, etc.).
pub fn dark_input_style(p: Palette) -> impl Fn(&iced::Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style + Copy {
    let bg = field_bg(p);
    move |_t, status| {
        let focused = matches!(status, iced::widget::text_input::Status::Focused);
        iced::widget::text_input::Style {
            background: iced::Background::Color(bg),
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: if focused { p.accent } else { Color { a: 0.4, ..p.muted } },
            },
            icon: p.muted,
            placeholder: Color { a: 0.6, ..p.muted },
            value: p.text,
            selection: Color { a: 0.3, ..p.accent },
        }
    }
}

/// Monochrome icon glyphs (die, folder, moon, sun, undo, arrows) — Segoe UI
/// Symbol, loaded at startup. Same font the C# app uses for these icons.
pub const ICONS: iced::Font = iced::Font::with_name("Segoe UI Symbol");

fn font_cache() -> &'static Mutex<HashMap<String, &'static str>> {
    static C: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Build an iced::Font from an optional family name + weight. iced wants a
/// `&'static str` family name, so runtime-chosen names are interned (leaked
/// once) into a process-wide cache. `None`/empty falls back to the default UI
/// font with the requested weight.
pub fn named_font(name: &Option<String>, weight: iced::font::Weight) -> iced::Font {
    match name.as_ref().filter(|s| !s.is_empty()) {
        Some(s) => {
            let mut cache = font_cache().lock().unwrap();
            let leaked: &'static str = cache
                .entry(s.clone())
                .or_insert_with(|| Box::leak(s.clone().into_boxed_str()));
            iced::Font { family: iced::font::Family::Name(leaked), weight, ..iced::Font::DEFAULT }
        }
        None => iced::Font { weight, ..iced::Font::DEFAULT },
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub tile: Color,
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
}

pub fn parse_hex(s: &str, fallback: Color) -> Color {
    let h = s.trim_start_matches('#');
    let (a, rgb) = match h.len() {
        8 => (u8::from_str_radix(&h[0..2], 16).unwrap_or(255), &h[2..]),
        6 => (255, h),
        _ => return fallback,
    };
    let r = u8::from_str_radix(&rgb[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&rgb[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&rgb[4..6], 16).unwrap_or(0);
    Color::from_rgba8(r, g, b, a as f32 / 255.0)
}

pub fn swatch_color(hex: &str) -> Color {
    parse_hex(hex, Color::from_rgb(0.3, 0.3, 0.3))
}

pub fn lerp(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

impl Palette {
    pub fn from_settings(s: &AppSettings, opacity: f32) -> Self {
        let bg = parse_hex(&s.theme_bg, Color::from_rgb(0.118, 0.118, 0.133));
        let tile = parse_hex(&s.theme_tile, Color::from_rgb(0.165, 0.165, 0.188));
        let accent = parse_hex(&s.theme_accent, Color::from_rgb(0.0, 0.659, 1.0));
        let text = parse_hex(&s.theme_text, Color::from_rgb(0.910, 0.910, 0.925));
        let mut muted = parse_hex(&s.theme_muted, Color::from_rgb(0.604, 0.604, 0.659));
        // C# MutedContrast: >1 blends toward text (more visible), <1 toward bg
        let mc = s.muted_contrast;
        if mc > 1.0 {
            muted = lerp(muted, text, (mc - 1.0).clamp(0.0, 1.0));
        } else if mc < 1.0 {
            muted = lerp(muted, bg, (1.0 - mc).clamp(0.0, 1.0));
        }
        let op = opacity.clamp(0.2, 1.0);
        Self {
            bg: Color { a: bg.a * op, ..bg },
            tile: Color { a: tile.a * op, ..tile },
            accent,
            text,
            muted,
        }
    }
}

/// Theme-aware toggle-switch style: accent track when on, muted track when off,
/// white knob. Replaces iced's default fixed-blue toggler so it matches the
/// active theme accent (C# ToggleSwitch behaviour).
pub fn toggler_style(p: Palette) -> impl Fn(&iced::Theme, iced::widget::toggler::Status) -> iced::widget::toggler::Style + Copy {
    move |_t, status| {
        use iced::widget::toggler::{Status, Style};
        let on = matches!(
            status,
            Status::Active { is_toggled: true } | Status::Hovered { is_toggled: true }
        );
        Style {
            background: if on { p.accent } else { Color { a: 0.45, ..p.muted } },
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: Color::WHITE,
            foreground_border_width: 0.0,
            foreground_border_color: Color::TRANSPARENT,
        }
    }
}

/// C# warning gradient: dist below threshold -> color.
/// blue(15) -> purple(10) -> red-purple(4) -> bright red(0)
/// Gradient unit colour: a fixed cool blue when the value is 15°C+ below the
/// threshold, shifting through a violet midpoint to the user-chosen `hot` colour
/// as the value reaches the threshold (`dist` = threshold − value).
pub fn gradient_color(dist: f64, hot: Color) -> Color {
    let cool = Color::from_rgb(0x00 as f32 / 255.0, 0x66 as f32 / 255.0, 0xCC as f32 / 255.0);
    let mid = Color::from_rgb(0x66 as f32 / 255.0, 0x33 as f32 / 255.0, 0xCC as f32 / 255.0);
    let t = ((15.0 - dist) / 15.0).clamp(0.0, 1.0) as f32;
    // Two-leg blend: cool→mid for the first half, mid→hot for the second, so the
    // ramp keeps a pleasing curve regardless of the chosen hot colour.
    if t < 0.5 {
        lerp(cool, mid, t * 2.0)
    } else {
        lerp(mid, hot, (t - 0.5) * 2.0)
    }
}

/// (name, bg, tile, accent, text, muted) — ported verbatim from ThemeApplier.cs
pub const THEME_PRESETS: [(&str, &str, &str, &str, &str, &str); 57] = [
    ("Dark (default)",      "#E61E1E22", "#FF2A2A30", "#FF00A8FF", "#FFE8E8EC", "#FF9A9AA8"),
    ("Light (default)",     "#FFF0F0F5", "#FFFFFFFF", "#FF0066CC", "#FF1C1C1E", "#FF6E6E73"),
    ("Catppuccin Mocha",    "#FF1E1E2E", "#FF313244", "#FF89B4FA", "#FFCDD6F4", "#FF6C7086"),
    ("One Dark",            "#FF282C34", "#FF21252B", "#FF61AFEF", "#FFABB2BF", "#FF5C6370"),
    ("Dracula",             "#FF282A36", "#FF44475A", "#FFBD93F9", "#FFF8F8F2", "#FF6272A4"),
    ("Tokyo Night",         "#FF1A1B2E", "#FF24283B", "#FF7AA2F7", "#FFC0CAF5", "#FF565F89"),
    ("Gruvbox",             "#FF282828", "#FF3C3836", "#FFD79921", "#FFEBDBB2", "#FFA89984"),
    ("Nord",                "#FF2E3440", "#FF3B4252", "#FF88C0D0", "#FFECEFF4", "#FF616E88"),
    ("Rosé Pine",           "#FF191724", "#FF1F1D2E", "#FFEB6F92", "#FFE0DEF4", "#FF6E6A86"),
    ("Kanagawa",            "#FF1F1F28", "#FF2A2A37", "#FF7E9CD8", "#FFDCD7BA", "#FF727169"),
    ("Everforest",          "#FF2D353B", "#FF343F44", "#FFA7C080", "#FFD3C6AA", "#FF859289"),
    ("Solarized Dark",      "#FF002B36", "#FF073642", "#FF268BD2", "#FFFDF6E3", "#FF657B83"),
    ("Monokai Pro",         "#FF2D2A2E", "#FF403E41", "#FFA9DC76", "#FFFCFCFA", "#FF727072"),
    ("Palenight",           "#FF292D3E", "#FF333747", "#FFC3E88D", "#FFEEEFFF", "#FF676E95"),
    ("Ayu Mirage",          "#FF1F2430", "#FF242B38", "#FFFFB454", "#FFCCCAC2", "#FF707A8C"),
    ("Poimandres",          "#FF1B1E28", "#FF252837", "#FF5DE4C7", "#FFE4F0FB", "#FF767C9D"),
    ("Horizon",             "#FF1C1E26", "#FF232530", "#FFE95678", "#FFECECEC", "#FF6C6F93"),
    ("Mellow",              "#FF1A1A19", "#FF252521", "#FFF0A868", "#FFDBDBB4", "#FF72726B"),
    ("Catppuccin Latte",    "#FFEFF1F5", "#FFCCD0DA", "#FF1E66F5", "#FF4C4F69", "#FF6C6F85"),
    ("Catppuccin Frappé",   "#FF303446", "#FF414559", "#FF8CAAEE", "#FFC6D0F5", "#FFA5ADCE"),
    ("Catppuccin Macchiato","#FF24273A", "#FF363A4F", "#FF8AADF4", "#FFCAD3F5", "#FFA5ADCB"),
    ("GitHub Dark",         "#FF0D1117", "#FF161B22", "#FF58A6FF", "#FFC9D1D9", "#FF8B949E"),
    ("GitHub Light",        "#FFFFFFFF", "#FFF6F8FA", "#FF0969DA", "#FF1F2328", "#FF656D76"),
    ("GitHub Dark Dimmed",  "#FF22272E", "#FF2D333B", "#FF539BF5", "#FFADBAC7", "#FF768390"),
    ("Solarized Light",     "#FFFDF6E3", "#FFEEE8D5", "#FF268BD2", "#FF586E75", "#FF93A1A1"),
    ("Gruvbox Light",       "#FFFBF1C7", "#FFEBDBB2", "#FFB57614", "#FF3C3836", "#FF7C6F64"),
    ("Ayu Light",           "#FFFAFAFA", "#FFF2F2F2", "#FFFA8D3E", "#FF5C6166", "#FF8A9199"),
    ("Ayu Dark",            "#FF0B0E14", "#FF131721", "#FFE6B450", "#FFBFBDB6", "#FF565B66"),
    ("Night Owl",           "#FF011627", "#FF112233", "#FF82AAFF", "#FFD6DEEB", "#FF637777"),
    ("Light Owl",           "#FFFBFBFB", "#FFF0F0F0", "#FF2AA298", "#FF403F53", "#FF989FB1"),
    ("Synthwave '84",       "#FF241B2F", "#FF2A2139", "#FFFF7EDB", "#FFFFFFFF", "#FF848BBD"),
    ("Atom One Light",      "#FFFAFAFA", "#FFEFEFEF", "#FF4078F2", "#FF383A42", "#FFA0A1A7"),
    ("Cobalt2",             "#FF193549", "#FF1F4662", "#FFFFC600", "#FFFFFFFF", "#FF0088FF"),
    ("Shades of Purple",    "#FF2D2B55", "#FF1E1E3F", "#FFFAD000", "#FFFFFFFF", "#FFA599E9"),
    ("Material Darker",     "#FF212121", "#FF2A2A2A", "#FFFF9800", "#FFEEFFFF", "#FF545454"),
    ("Panda",               "#FF292A2B", "#FF31353A", "#FFFF75B5", "#FFE6E6E6", "#FF676B79"),
    ("Oceanic Next",        "#FF1B2B34", "#FF232E38", "#FF6699CC", "#FFCDD3DE", "#FF65737E"),
    ("Snazzy Light",        "#FFFFFFFF", "#FFF7F8F9", "#FFFF5C57", "#FF333333", "#FF888888"),
    ("Navy & Copper",       "#FF0E2240", "#FF152D52", "#FFD4A14A", "#FFEFE6D3", "#FF8A9BB5"),
    ("Everforest Dark",     "#FF374145", "#FF2D353B", "#FFA7C080", "#FFD3C6AA", "#FF859289"),
    ("Evergreen",           "#FF0C140C", "#FF1A261A", "#FF6C9848", "#FFD4DCC8", "#FF688860"),
    ("Sandstone",           "#FF100E0A", "#FF1E1C16", "#FFB8A070", "#FFE0DCD0", "#FF807860"),
    ("Deep Current",        "#FF0A1014", "#FF141E24", "#FF5898A0", "#FFD0DCE0", "#FF608080"),
    ("Morning Dew",         "#FF0C0E0A", "#FF1A1E18", "#FFA8B880", "#FFDCE0D4", "#FF788870"),
    ("Hearthwood",          "#FF100C08", "#FF201A14", "#FFB87848", "#FFE0D8CC", "#FF887058"),
    ("Terracotta",          "#FF0E0C0C", "#FF1C1A1A", "#FFA86850", "#FFDCD8D4", "#FF806860"),
    ("Tidestone",           "#FF12100C", "#FF201E18", "#FF5898A0", "#FFDCD8CC", "#FF807868"),
    ("Forest Gold",         "#FF0C140E", "#FF1A2618", "#FFC8B870", "#FFD8E0D0", "#FF688860"),
    ("Inlet",               "#FF0A1214", "#FF142022", "#FFB87848", "#FFD0DCE0", "#FF607880"),
    ("Canopy",              "#FF0E100C", "#FF1C1E1A", "#FF4C8840", "#FFD8DCD0", "#FF788870"),
    ("Sage",                "#FF0E0C0A", "#FF1C1A18", "#FFA8C088", "#FFDCE0D4", "#FF787060"),
    ("Clay Coast",          "#FF0A0E12", "#FF141C22", "#FFA86850", "#FFD0D8DC", "#FF607078"),
    ("Dusk Harbor",         "#FF100E12", "#FF1E1C22", "#FF68A0A8", "#FFD8D8E0", "#FF787080"),
    ("Fern",                "#FF0A120A", "#FF162016", "#FF78B060", "#FFD4E0CC", "#FF588850"),
    ("Driftwood",           "#FF100E0A", "#FF1E1A16", "#FF889870", "#FFDCD8CC", "#FF787058"),
    ("Glacier",             "#FF0C0E10", "#FF1A1E22", "#FF78A8C0", "#FFD8DCE4", "#FF687880"),
    ("Amber Trail",         "#FF0E0A08", "#FF1E1610", "#FFC8A050", "#FFE0D8C8", "#FF806840"),
];

pub fn match_preset(s: &AppSettings) -> Option<usize> {
    THEME_PRESETS.iter().position(|(_, bg, tile, accent, text, muted)| {
        s.theme_bg.eq_ignore_ascii_case(bg)
            && s.theme_tile.eq_ignore_ascii_case(tile)
            && s.theme_accent.eq_ignore_ascii_case(accent)
            && s.theme_text.eq_ignore_ascii_case(text)
            && s.theme_muted.eq_ignore_ascii_case(muted)
    })
}

pub fn apply_preset(s: &mut AppSettings, idx: usize) {
    let (_, bg, tile, accent, text, muted) = THEME_PRESETS[idx % THEME_PRESETS.len()];
    s.theme_bg = bg.to_string();
    s.theme_tile = tile.to_string();
    s.theme_accent = accent.to_string();
    s.theme_text = text.to_string();
    s.theme_muted = muted.to_string();
}


#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum BorderSource { Transparent, Muted, Accent, Text }

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(default)]
pub struct SkinStyle {
    pub tile_radius: f32,
    pub widget_radius: f32,
    pub tile_border: f32,
    pub widget_border: f32,
    pub tile_spacing: f32,
    pub border_src: BorderSource,
    pub border_alpha: f32,
    pub accent_bar: f32,
    pub header_bar: f32,
    pub sheen: f32,
    /// Outer glow intensity (0..1) — renders an accent-tinted bloom around tiles
    /// and the widget frame. The big "bold skin" differentiator.
    #[serde(default)]
    pub glow: f32,
    /// Background gradient strength (0..1) — fades the tile fill toward a lighter
    /// top and an accent-tinted bottom for depth/colour.
    #[serde(default)]
    pub gradient: f32,
}

impl Default for SkinStyle {
    fn default() -> Self {
        // The "Default" skin — also the base for external skins that omit fields.
        SkinStyle {
            tile_radius: 12.0, widget_radius: 16.0,
            tile_border: 0.0, widget_border: 0.0, tile_spacing: 6.0,
            border_src: BorderSource::Transparent, border_alpha: 0.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.0,
        }
    }
}

impl SkinStyle {
    pub fn border_color(&self, p: &Palette) -> Color {
        let base = match self.border_src {
            BorderSource::Transparent => Color::TRANSPARENT,
            BorderSource::Muted => p.muted,
            BorderSource::Accent => p.accent,
            BorderSource::Text => p.text,
        };
        Color { a: base.a * self.border_alpha, ..base }
    }

    /// Clamp every field to a sane range so a hand-edited / downloaded skin file
    /// can never produce absurd geometry. Pure data — no code is executed.
    fn sanitized(mut self) -> Self {
        self.tile_radius = self.tile_radius.clamp(0.0, 50.0);
        self.widget_radius = self.widget_radius.clamp(0.0, 50.0);
        self.tile_border = self.tile_border.clamp(0.0, 10.0);
        self.widget_border = self.widget_border.clamp(0.0, 10.0);
        self.tile_spacing = self.tile_spacing.clamp(0.0, 30.0);
        self.border_alpha = self.border_alpha.clamp(0.0, 1.0);
        self.accent_bar = self.accent_bar.clamp(0.0, 10.0);
        self.header_bar = self.header_bar.clamp(0.0, 10.0);
        self.sheen = self.sheen.clamp(0.0, 1.0);
        self.glow = self.glow.clamp(0.0, 1.0);
        self.gradient = self.gradient.clamp(0.0, 1.0);
        self
    }
}

pub const SKIN_NAMES: [&str; 16] = [
    "Default","Minimal","Sharp","Glassmorphism","Retro",
    "Terminal","Holographic","Brutalist","Carbon","Neon",
    "Frosted","Cyberpunk","Paper","Ink","Aurora","Compact",
];

/// Resolve a skin by name: user-installed skins first, then the built-ins.
pub fn skin_style(name: &str) -> SkinStyle {
    if let Some(s) = external_skins().get(name) {
        return *s;
    }
    builtin_skin_style(name)
}

fn builtin_skin_style(name: &str) -> SkinStyle {
    // Values mirror the C# Styles/Skins/*.xaml exactly. `sheen` = the effective
    // C# overlay intensity (SkinSheenOpacity × SkinSheenAlpha). Partial C#
    // borders (e.g. bottom-only) map to a uniform border of the same thickness.
    match name {
        // ── Clean / minimal set ──
        "Default" => SkinStyle {
            tile_radius: 12.0, widget_radius: 16.0,
            tile_border: 0.0, widget_border: 0.0, tile_spacing: 6.0,
            border_src: BorderSource::Transparent, border_alpha: 0.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.15,
        },
        "Minimal" => SkinStyle {
            tile_radius: 0.0, widget_radius: 8.0,
            tile_border: 1.0, widget_border: 0.0, tile_spacing: 2.0,
            border_src: BorderSource::Muted, border_alpha: 1.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.0,
        },
        "Sharp" => SkinStyle {
            tile_radius: 0.0, widget_radius: 0.0,
            tile_border: 1.0, widget_border: 1.0, tile_spacing: 2.0,
            border_src: BorderSource::Muted, border_alpha: 1.0,
            accent_bar: 3.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.0,
        },
        "Brutalist" => SkinStyle {
            tile_radius: 0.0, widget_radius: 0.0,
            tile_border: 3.0, widget_border: 3.0, tile_spacing: 4.0,
            border_src: BorderSource::Text, border_alpha: 1.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.0,
        },
        "Paper" => SkinStyle {
            tile_radius: 4.0, widget_radius: 8.0,
            tile_border: 0.0, widget_border: 0.0, tile_spacing: 8.0,
            border_src: BorderSource::Transparent, border_alpha: 0.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.0,
        },
        "Ink" => SkinStyle {
            tile_radius: 0.0, widget_radius: 2.0,
            tile_border: 2.0, widget_border: 0.0, tile_spacing: 4.0,
            border_src: BorderSource::Text, border_alpha: 0.8,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.0,
        },
        "Compact" => SkinStyle {
            tile_radius: 4.0, widget_radius: 6.0,
            tile_border: 0.0, widget_border: 0.0, tile_spacing: 2.0,
            border_src: BorderSource::Transparent, border_alpha: 0.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.0, gradient: 0.0,
        },
        "Carbon" => SkinStyle {
            tile_radius: 6.0, widget_radius: 8.0,
            tile_border: 1.0, widget_border: 0.0, tile_spacing: 4.0,
            border_src: BorderSource::Muted, border_alpha: 0.5,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.12,
            glow: 0.0, gradient: 0.28,
        },
        // ── Soft / glassy set ──
        "Glassmorphism" => SkinStyle {
            tile_radius: 14.0, widget_radius: 18.0,
            tile_border: 1.5, widget_border: 0.0, tile_spacing: 10.0,
            border_src: BorderSource::Text, border_alpha: 0.67,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.53,
            glow: 0.18, gradient: 0.5,
        },
        "Frosted" => SkinStyle {
            tile_radius: 16.0, widget_radius: 20.0,
            tile_border: 0.0, widget_border: 0.0, tile_spacing: 8.0,
            border_src: BorderSource::Transparent, border_alpha: 0.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.126,
            glow: 0.12, gradient: 0.42,
        },
        "Aurora" => SkinStyle {
            tile_radius: 12.0, widget_radius: 16.0,
            tile_border: 0.0, widget_border: 0.0, tile_spacing: 8.0,
            border_src: BorderSource::Transparent, border_alpha: 0.0,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.18,
            glow: 0.3, gradient: 0.72,
        },
        "Retro" => SkinStyle {
            tile_radius: 4.0, widget_radius: 4.0,
            tile_border: 2.0, widget_border: 2.0, tile_spacing: 6.0,
            border_src: BorderSource::Accent, border_alpha: 1.0,
            accent_bar: 0.0, header_bar: 20.0, sheen: 0.0,
            glow: 0.0, gradient: 0.2,
        },
        // ── Bold / glowing set ──
        "Neon" => SkinStyle {
            tile_radius: 0.0, widget_radius: 2.0,
            tile_border: 1.5, widget_border: 0.0, tile_spacing: 8.0,
            border_src: BorderSource::Accent, border_alpha: 1.0,
            accent_bar: 4.0, header_bar: 0.0, sheen: 0.0,
            glow: 1.0, gradient: 0.0,
        },
        "Cyberpunk" => SkinStyle {
            tile_radius: 0.0, widget_radius: 0.0,
            tile_border: 1.0, widget_border: 0.0, tile_spacing: 3.0,
            border_src: BorderSource::Accent, border_alpha: 0.9,
            accent_bar: 5.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.8, gradient: 0.35,
        },
        "Holographic" => SkinStyle {
            tile_radius: 8.0, widget_radius: 10.0,
            tile_border: 2.0, widget_border: 0.0, tile_spacing: 6.0,
            border_src: BorderSource::Accent, border_alpha: 1.0,
            accent_bar: 3.0, header_bar: 0.0, sheen: 0.125,
            glow: 0.7, gradient: 0.5,
        },
        "Terminal" => SkinStyle {
            tile_radius: 0.0, widget_radius: 0.0,
            tile_border: 1.0, widget_border: 1.0, tile_spacing: 1.0,
            border_src: BorderSource::Accent, border_alpha: 0.6,
            accent_bar: 0.0, header_bar: 0.0, sheen: 0.0,
            glow: 0.35, gradient: 0.0,
        },
        _ => builtin_skin_style("Default"),
    }
}

// ── User-installed skins (data-only) ─────────────────────────────────────────
//
// Skins live as JSON files in `%APPDATA%\Fluxid\…\skins\*.json`. They are
// pure data (geometry numbers + a border source enum) — never code — parsed
// with serde, range-clamped, and unable to shadow a built-in skin name. Loaded
// once, lazily, on first use; drop a file in and restart to pick it up.

#[derive(serde::Deserialize)]
struct SkinFile {
    name: String,
    #[serde(flatten)]
    style: SkinStyle,
}

/// Directory holding user skin files.
pub fn skins_dir() -> std::path::PathBuf {
    fluid_core::settings::AppSettings::config_dir().join("skins")
}

fn external_skins() -> &'static HashMap<String, SkinStyle> {
    static EXTERNAL: OnceLock<HashMap<String, SkinStyle>> = OnceLock::new();
    EXTERNAL.get_or_init(read_skins_dir)
}

fn read_skins_dir() -> HashMap<String, SkinStyle> {
    let mut map = HashMap::new();
    let Ok(entries) = std::fs::read_dir(skins_dir()) else { return map };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(skin) = serde_json::from_str::<SkinFile>(&text) else { continue };
        let name = skin.name.trim().to_string();
        // Reject blanks and names that would shadow a built-in skin.
        if name.is_empty() || SKIN_NAMES.contains(&name.as_str()) {
            continue;
        }
        map.insert(name, skin.style.sanitized());
    }
    map
}

/// All available skin names: built-ins followed by sorted user skins.
pub fn skin_names() -> Vec<String> {
    let mut names: Vec<String> = SKIN_NAMES.iter().map(|s| s.to_string()).collect();
    let mut ext: Vec<String> = external_skins().keys().cloned().collect();
    ext.sort();
    names.extend(ext);
    names
}

/// Create the skins directory (and a commented example + README the first time)
/// and return its path. Used by the upcoming Download/skins dialog.
#[allow(dead_code)]
pub fn ensure_skins_dir() -> std::path::PathBuf {
    let dir = skins_dir();
    let _ = std::fs::create_dir_all(&dir);
    let example = dir.join("example-skin.json.txt");
    if !example.exists() {
        let _ = std::fs::write(&example, EXAMPLE_SKIN);
    }
    dir
}

#[allow(dead_code)]
const EXAMPLE_SKIN: &str = r#"// Fluxid skin file — copy this to "<YourName>.json" (drop the .txt),
// edit the values, and restart Fluxid. Skins are pure data: only the
// fields below are read, all are optional, and values are range-clamped.
//
//   border_src: "Transparent" | "Muted" | "Accent" | "Text"
//
// {
//   "name": "My Skin",
//   "tile_radius": 10.0,
//   "widget_radius": 14.0,
//   "tile_border": 1.0,
//   "widget_border": 0.0,
//   "tile_spacing": 6.0,
//   "border_src": "Accent",
//   "border_alpha": 1.0,
//   "accent_bar": 3.0,
//   "header_bar": 0.0,
//   "sheen": 0.1
// }
"#;

// ── Game theme packs (bundled, data-only) ────────────────────────────────────
//
// Franchise colour palettes ported verbatim from the C# app's /themes. Each
// theme is colours + a paired skin (`category`). Embedded at compile time and
// parsed with serde — pure data, no code, no network.

#[derive(serde::Deserialize, Clone)]
pub struct PackTheme {
    pub name: String,
    pub bg: String,
    pub tile: String,
    pub accent: String,
    pub text: String,
    pub muted: String,
    #[serde(default)]
    pub category: String,
}

#[derive(serde::Deserialize)]
struct ThemePackFile {
    franchise: String,
    themes: Vec<PackTheme>,
}

pub struct ThemePack {
    pub franchise: String,
    pub themes: Vec<PackTheme>,
}

const THEME_PACK_JSON: &[&str] = &[
    include_str!("../themes/amnesia.json"),
    include_str!("../themes/baldurs-gate-3.json"),
    include_str!("../themes/borderlands.json"),
    include_str!("../themes/crash-bandicoot.json"),
    include_str!("../themes/cyberpunk-2077.json"),
    include_str!("../themes/dayz.json"),
    include_str!("../themes/doom.json"),
    include_str!("../themes/fallout.json"),
    include_str!("../themes/hades.json"),
    include_str!("../themes/helldivers.json"),
    include_str!("../themes/hollow-knight.json"),
    include_str!("../themes/league-of-legends.json"),
    include_str!("../themes/mass-effect.json"),
    include_str!("../themes/minecraft.json"),
    include_str!("../themes/no-mans-sky.json"),
    include_str!("../themes/persona-5.json"),
    include_str!("../themes/runescape.json"),
    include_str!("../themes/spore.json"),
    include_str!("../themes/spyro.json"),
    include_str!("../themes/stardew-valley.json"),
    include_str!("../themes/stronghold-2.json"),
    include_str!("../themes/valheim.json"),
    include_str!("../themes/witcher.json"),
    include_str!("../themes/world-of-tanks.json"),
    include_str!("../themes/wow.json"),
];

/// All bundled game theme packs, sorted by franchise. Parsed once.
pub fn theme_packs() -> &'static Vec<ThemePack> {
    static PACKS: OnceLock<Vec<ThemePack>> = OnceLock::new();
    PACKS.get_or_init(|| {
        let mut packs: Vec<ThemePack> = THEME_PACK_JSON
            .iter()
            .filter_map(|j| serde_json::from_str::<ThemePackFile>(j).ok())
            .map(|f| ThemePack { franchise: f.franchise, themes: f.themes })
            .collect();
        packs.sort_by_key(|p| p.franchise.to_lowercase());
        packs
    })
}

/// Apply a pack theme: its five colours plus its paired skin (`category`).
pub fn apply_pack_theme(s: &mut AppSettings, t: &PackTheme) {
    s.theme_bg = t.bg.clone();
    s.theme_tile = t.tile.clone();
    s.theme_accent = t.accent.clone();
    s.theme_text = t.text.clone();
    s.theme_muted = t.muted.clone();
    if !t.category.trim().is_empty() {
        s.active_skin = t.category.clone();
    }
}
