use std::sync::Arc;

use glyphon::cosmic_text::{Align, Style};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight, fontdb};

use crate::{Color, TextAlign, TextSpec};

pub const DEFAULT_FAMILY: &str = "Lato";
const LINE_HEIGHT: f32 = 1.35;

/// The font database. Only the bundled font and fonts added with [`Fonts::load`] are used, so
/// text looks the same on every platform, including the web.
pub struct Fonts {
    pub(crate) system: FontSystem,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSize {
    pub width: f32,
    pub height: f32,
    pub lines: usize,
}

impl Fonts {
    pub fn new() -> Fonts {
        let mut db = fontdb::Database::new();
        db.load_font_data(include_bytes!("../fonts/Lato-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../fonts/Lato-Bold.ttf").to_vec());
        load_system_cjk_fallback(&mut db);
        db.set_sans_serif_family(DEFAULT_FAMILY);
        Fonts { system: FontSystem::new_with_locale_and_db("en-US".into(), db) }
    }

    /// Registers a TrueType/OpenType font and returns its family name.
    pub fn load(&mut self, data: Vec<u8>) -> Option<String> {
        let ids = self.system.db_mut().load_font_source(fontdb::Source::Binary(Arc::new(data)));
        let face = self.system.db().face(*ids.first()?)?;
        face.families.first().map(|(name, _)| name.clone())
    }

    /// Lays out `text` at virtual size; `max_width` wraps at word boundaries.
    pub fn measure(&mut self, text: &str, size: f32, family: Option<&str>, max_width: Option<f32>) -> TextSize {
        let spec = TextSpec { text: text.into(), size, family: family.map(String::from), ..TextSpec::default() };
        let buffer = self.layout(&spec, max_width, 1.0);
        let mut out = TextSize { width: 0.0, height: 0.0, lines: 0 };
        for run in buffer.layout_runs() {
            out.width = out.width.max(run.line_w);
            out.height = run.line_top + run.line_height;
            out.lines += 1;
        }
        out
    }

    /// The text of each wrapped line, for tests and pagination.
    pub fn wrap(&mut self, text: &str, size: f32, width: f32) -> Vec<String> {
        let spec = TextSpec { text: text.into(), size, ..TextSpec::default() };
        let buffer = self.layout(&spec, Some(width), 1.0);
        buffer
            .layout_runs()
            .map(|run| {
                let (Some(first), Some(last)) = (run.glyphs.first(), run.glyphs.last()) else { return String::new() };
                run.text[first.start..last.end].trim_end().to_string()
            })
            .collect()
    }

    pub(crate) fn layout(&mut self, spec: &TextSpec, width: Option<f32>, scale: f32) -> Buffer {
        let size = spec.size * scale;
        let mut buffer = Buffer::new(&mut self.system, Metrics::new(size, size * LINE_HEIGHT));
        buffer.set_size(width.map(|w| w * scale), None);
        let family = spec.family.as_deref().map(Family::Name).unwrap_or(Family::SansSerif);
        let weight = if spec.bold { Weight::BOLD } else { Weight::NORMAL };
        let attrs = Attrs::new().family(family).weight(weight).color(glyph_color(spec.color));
        let align = match spec.align {
            TextAlign::Left => None,
            TextAlign::Center => Some(Align::Center),
        };
        if let Ok(markup) = kairos_script::parse_text_markup(&spec.text) {
            let mut remaining = spec.reveal.unwrap_or(usize::MAX);
            let mut rich = Vec::new();
            for span in &markup.spans {
                let text = &markup.plain[span.range.clone()];
                let mut style = span.style;
                if markup
                    .ruby
                    .iter()
                    .any(|(_, reading)| reading.start <= span.range.start && span.range.end <= reading.end)
                {
                    style.size.get_or_insert((size * 0.5).round().max(1.0) as u16);
                }
                let styled = span_attrs(attrs.clone(), style, size);
                let hidden = styled.clone().color(glyph_color(Color { a: 0, ..spec.color }));
                let visible = text.char_indices().nth(remaining).map_or(text.len(), |(i, _)| i);
                rich.push((&text[..visible], styled));
                rich.push((&text[visible..], hidden));
                remaining = remaining.saturating_sub(text.chars().count());
            }
            buffer.set_rich_text(rich, &attrs, Shaping::Advanced, align);
        } else {
            buffer.set_text(&spec.text, &attrs, Shaping::Advanced, align);
        }
        buffer.shape_until_scroll(&mut self.system, false);
        buffer
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_system_cjk_fallback(db: &mut fontdb::Database) {
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "C:/Windows/Fonts/msyh.ttc",
    ];
    for path in candidates {
        if let Ok(font) = std::fs::read(path) {
            db.load_font_data(font);
            return;
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk_fallback(_: &mut fontdb::Database) {}

fn span_attrs<'a>(attrs: Attrs<'a>, style: kairos_script::TextStyle, base_size: f32) -> Attrs<'a> {
    let mut attrs = attrs.weight(if style.bold { Weight::BOLD } else { Weight::NORMAL }).style(if style.italic {
        Style::Italic
    } else {
        Style::Normal
    });
    if let Some([r, g, b]) = style.color {
        attrs = attrs.color(glyph_color(Color::rgb(r, g, b)));
    }
    if let Some(size) = style.size {
        let size = size as f32;
        attrs = attrs.metrics(Metrics::new(size, size * LINE_HEIGHT));
    } else {
        attrs = attrs.metrics(Metrics::new(base_size, base_size * LINE_HEIGHT));
    }
    attrs
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts::new()
    }
}

pub(crate) fn glyph_color(c: Color) -> glyphon::Color {
    glyphon::Color::rgba(c.r, c.g, c.b, c.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_wrap_inside_the_given_width() {
        let mut fonts = Fonts::new();
        let text = "Kairos runs your story in Rust, so rollback and saves just work, even after edits.";
        let one_line = fonts.measure(text, 28.0, None, None);
        assert_eq!(one_line.lines, 1);
        let wrapped = fonts.measure(text, 28.0, None, Some(400.0));
        assert!(wrapped.lines >= 3, "{wrapped:?}");
        assert!(wrapped.width <= 400.0, "{wrapped:?}");
        assert!((wrapped.height - wrapped.lines as f32 * 28.0 * LINE_HEIGHT).abs() < 0.5);
    }

    #[test]
    fn wrapping_breaks_between_words_and_keeps_every_word() {
        let mut fonts = Fonts::new();
        let text = "alpha beta gamma delta epsilon zeta eta theta";
        let lines = fonts.wrap(text, 24.0, 200.0);
        assert!(lines.len() > 1, "{lines:?}");
        assert_eq!(lines.join(" "), text);
    }

    #[test]
    fn long_cjk_runs_wrap_between_characters() {
        let mut fonts = Fonts::new();
        let lines = fonts.wrap("日本語を話します", 24.0, 60.0);
        assert!(lines.len() > 1, "{lines:?}");
        assert_eq!(lines.concat(), "日本語を話します");
    }

    #[test]
    fn explicit_newlines_start_new_lines() {
        let mut fonts = Fonts::new();
        assert_eq!(fonts.wrap("one\ntwo", 24.0, 1000.0), ["one", "two"]);
    }

    #[test]
    fn layout_displays_text_without_markup_delimiters() {
        let mut fonts = Fonts::new();
        assert_eq!(fonts.wrap("{b}Hello{/b}", 24.0, 1000.0), ["Hello"]);
    }

    #[test]
    fn typewriter_reveal_keeps_the_full_layout() {
        let mut fonts = Fonts::new();
        let text = "alpha beta gamma delta epsilon zeta eta theta";
        let full = TextSpec { text: text.into(), size: 24.0, ..TextSpec::default() };
        let partial = TextSpec { reveal: Some(3), ..full.clone() };
        let lines = |fonts: &mut Fonts, spec: &TextSpec| fonts.layout(spec, Some(200.0), 1.0).layout_runs().count();
        assert_eq!(lines(&mut fonts, &full), lines(&mut fonts, &partial));
    }
}
