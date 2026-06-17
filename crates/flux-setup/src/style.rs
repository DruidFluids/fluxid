//! Installer visual style — a centered, modern wizard that uses Flux's
//! built-in **"Dark (default)"** palette so setup looks like the app it
//! installs. Colors come from `THEME_PRESETS[0]` in
//! `flux-widget/src/style.rs`: bg `#1E1E22`, accent `#00A8FF`, text `#E8E8EC`,
//! muted `#9A9AA8`.

use iced::widget::canvas::{self, path, Frame, LineCap, LineJoin, Path, Stroke};
use iced::widget::{button, container, text};
use iced::{Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

pub const BG: Color = rgb(0x18, 0x18, 0x1C);
pub const ACCENT: Color = rgb(0x00, 0xA8, 0xFF);
pub const ACCENT_HOVER: Color = rgb(0x2C, 0xB6, 0xFF);
pub const TEXT: Color = rgb(0xE8, 0xE8, 0xEC);
pub const MUTED: Color = rgb(0x9A, 0x9A, 0xA8);
pub const DANGER: Color = rgb(0xFF, 0x6B, 0x6B);

const BTN_BG: Color = rgb(0x26, 0x26, 0x2C);
const BTN_BG_HOVER: Color = rgb(0x32, 0x32, 0x3A);
const BORDER_DIM: Color = rgb(0x4A, 0x4A, 0x54);
const DIVIDER: Color = rgb(0x30, 0x30, 0x38);
const SEG_EMPTY: Color = rgb(0x3A, 0x3A, 0x44);

/// The app theme: a custom dark palette built from the widget's defaults so
/// iced's stock widget styling (radios, checkboxes) already lands on the Flux
/// accent without per-widget overrides.
pub fn theme() -> Theme {
    Theme::custom(
        "Flux Dark".to_string(),
        iced::theme::Palette {
            background: BG,
            text: TEXT,
            primary: ACCENT,
            success: ACCENT,
            danger: DANGER,
        },
    )
}

// ── containers ──

pub fn root(_t: &Theme) -> container::Style {
    container::Style {
        background: Some(BG.into()),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

/// A 1px horizontal divider line.
pub fn divider(_t: &Theme) -> container::Style {
    container::Style {
        background: Some(DIVIDER.into()),
        ..container::Style::default()
    }
}

/// A step-indicator segment: accent when reached, dim otherwise.
pub fn segment(filled: bool) -> impl Fn(&Theme) -> container::Style {
    move |_t| container::Style {
        background: Some(if filled { ACCENT } else { SEG_EMPTY }.into()),
        border: Border {
            radius: 2.5.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

// ── text ──

/// White page heading.
pub fn heading(_t: &Theme) -> text::Style {
    text::Style { color: Some(TEXT) }
}
pub fn body(_t: &Theme) -> text::Style {
    text::Style { color: Some(TEXT) }
}
pub fn muted(_t: &Theme) -> text::Style {
    text::Style { color: Some(MUTED) }
}
pub fn accent_text(_t: &Theme) -> text::Style {
    text::Style { color: Some(ACCENT) }
}
pub fn danger(_t: &Theme) -> text::Style {
    text::Style { color: Some(DANGER) }
}

// ── buttons (outlined, rounded) ──

/// Primary action (Next / Install / Close): accent fill, dark text.
pub fn primary(_t: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => ACCENT_HOVER,
        button::Status::Disabled => Color { a: 0.30, ..ACCENT },
        button::Status::Active => ACCENT,
    };
    button::Style {
        background: Some(bg.into()),
        text_color: rgb(0x0A, 0x0A, 0x0E),
        border: Border {
            radius: 9.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Secondary action (Cancel / Back): outlined, subtle fill, light text.
pub fn secondary(_t: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => BTN_BG_HOVER,
        _ => BTN_BG,
    };
    button::Style {
        background: Some(bg.into()),
        text_color: TEXT,
        border: Border {
            color: BORDER_DIM,
            width: 1.0,
            radius: 9.0.into(),
        },
        ..button::Style::default()
    }
}

// ── accent icon badge (canvas) ──

/// A circular accent badge with a white "activity" pulse — the wizard's logo
/// mark, echoing the look of the running widget.
struct Badge;

impl<M> canvas::Program<M> for Badge {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, Size::new(bounds.width, bounds.height));
        let size = bounds.width.min(bounds.height);
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let r = size / 2.0;

        frame.fill(&Path::circle(center, r), ACCENT);

        // ECG-style pulse line, points as fractions of the radius.
        let pts = [
            (-0.55, 0.0),
            (-0.28, 0.0),
            (-0.13, 0.30),
            (0.0, -0.42),
            (0.14, 0.20),
            (0.28, 0.0),
            (0.55, 0.0),
        ];
        let mut b = path::Builder::new();
        for (i, (dx, dy)) in pts.iter().enumerate() {
            let p = Point::new(center.x + dx * r, center.y + dy * r);
            if i == 0 {
                b.move_to(p);
            } else {
                b.line_to(p);
            }
        }
        frame.stroke(
            &b.build(),
            Stroke::default()
                .with_width((size * 0.07).max(2.0))
                .with_color(Color::WHITE)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round),
        );

        vec![frame.into_geometry()]
    }
}

/// The accent icon badge as a fixed-size element.
pub fn badge<'a, M: 'a>() -> Element<'a, M> {
    canvas::Canvas::new(Badge)
        .width(Length::Fixed(56.0))
        .height(Length::Fixed(56.0))
        .into()
}
