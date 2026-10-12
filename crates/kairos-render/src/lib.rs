//! A wgpu renderer for a fixed-size virtual screen. Callers describe each frame as layers of
//! draw commands; nothing is retained between frames except textures and the transition snapshot.

mod gpu;
pub mod layout;
mod text;

pub use gpu::Renderer;
pub use layout::{Letterbox, Rect};
pub use text::{DEFAULT_FAMILY, Fonts, TextSize};

/// An sRGB colour with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`.
    pub fn hex(s: &str) -> Option<Color> {
        let s = s.strip_prefix('#')?;
        let digit = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok();
        let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            3 => Some(Color::rgb(digit(0)? * 17, digit(1)? * 17, digit(2)? * 17)),
            6 => Some(Color::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Color::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }

    pub fn with_alpha(self, opacity: f32) -> Color {
        Color { a: (self.a as f32 * opacity.clamp(0.0, 1.0)).round() as u8, ..self }
    }

    pub(crate) fn linear(self) -> [f32; 4] {
        let f = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        [f(self.r), f(self.g), f(self.b), self.a as f32 / 255.0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextSpec {
    pub text: String,
    /// Wrap width and clip bounds; with [`TextAlign::Center`] the text is also centred vertically.
    pub rect: Rect,
    pub size: f32,
    pub color: Color,
    pub family: Option<String>,
    pub bold: bool,
    pub align: TextAlign,
    /// Characters shown so far by a typewriter effect; the rest is laid out but invisible.
    pub reveal: Option<usize>,
}

impl Default for TextSpec {
    fn default() -> Self {
        TextSpec {
            text: String::new(),
            rect: Rect::default(),
            size: 24.0,
            color: Color::WHITE,
            family: None,
            bold: false,
            align: TextAlign::Left,
            reveal: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Draw {
    /// A rectangle with a vertical gradient and optional rounded corners.
    Fill {
        rect: Rect,
        top: Color,
        bottom: Color,
        radius: f32,
    },
    Image {
        name: String,
        rect: Rect,
        opacity: f32,
    },
    Text(TextSpec),
}

impl Draw {
    pub fn fill(rect: Rect, color: Color) -> Draw {
        Draw::Fill { rect, top: color, bottom: color, radius: 0.0 }
    }
}

/// Fills and images draw in order, then the layer's text on top.
pub type Layer = Vec<Draw>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    /// Background and sprites; captured for transitions.
    pub scene: Layer,
    /// Interface layers, back to front, drawn over the scene without transitions.
    pub ui: Vec<Layer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    /// Old frame to black, then black to the new frame.
    Fade,
    /// Crossfade from the old frame to the new one.
    Dissolve,
}

impl TransitionKind {
    /// Maps a script transition name. `move` crossfades, since sprites have no motion tweening yet.
    pub fn named(name: &str) -> Option<(TransitionKind, f32)> {
        match name {
            "fade" => Some((TransitionKind::Fade, 1.0)),
            "dissolve" => Some((TransitionKind::Dissolve, 0.5)),
            "move" | "ease" => Some((TransitionKind::Dissolve, 0.5)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colours_parse_in_three_lengths() {
        assert_eq!(Color::hex("#c8ffc8"), Some(Color::rgb(200, 255, 200)));
        assert_eq!(Color::hex("#fff"), Some(Color::WHITE));
        assert_eq!(Color::hex("#00000080"), Some(Color::rgba(0, 0, 0, 128)));
        assert_eq!(Color::hex("c8ffc8"), None);
        assert_eq!(Color::hex("#zzzzzz"), None);
    }

    #[test]
    fn transition_names_map_to_kinds_and_durations() {
        assert_eq!(TransitionKind::named("fade"), Some((TransitionKind::Fade, 1.0)));
        assert_eq!(TransitionKind::named("dissolve"), Some((TransitionKind::Dissolve, 0.5)));
        assert_eq!(TransitionKind::named("move").map(|t| t.0), Some(TransitionKind::Dissolve));
        assert_eq!(TransitionKind::named("wipeleft"), None);
    }
}
