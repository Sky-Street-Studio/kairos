use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub color: Option<[u8; 3]>,
    pub size: Option<u16>,
    pub shake: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSpan {
    pub range: Range<usize>,
    pub style: TextStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextMarkup {
    pub plain: String,
    pub visible_chars: usize,
    pub spans: Vec<TextSpan>,
    pub waits: Vec<(usize, f32)>,
    pub ruby: Vec<(Range<usize>, Range<usize>)>,
}

pub fn parse_text_markup(text: &str) -> Result<TextMarkup, String> {
    let mut plain = String::new();
    let mut spans = Vec::new();
    let mut waits = Vec::new();
    let mut ruby_base_start = None;
    let mut ruby_reading_start = None;
    let mut ruby_bases = Vec::new();
    let mut ruby = Vec::new();
    let mut style = TextStyle::default();
    let mut stack: Vec<(String, TextStyle)> = Vec::new();
    let mut visible_chars = 0;
    let mut rest = text;
    while !rest.is_empty() {
        let Some(start) = rest.find('{') else {
            append(rest, style, &mut plain, &mut spans, &mut visible_chars);
            break;
        };
        let (before, after_start) = rest.split_at(start);
        append(before, style, &mut plain, &mut spans, &mut visible_chars);
        if let Some(literal) = after_start.strip_prefix("{{") {
            append("{", style, &mut plain, &mut spans, &mut visible_chars);
            rest = literal;
            continue;
        }
        let Some(end) = after_start.find('}') else {
            return Err("unclosed text tag".into());
        };
        let tag = &after_start[1..end];
        let (name, closing) = tag.strip_prefix('/').map_or((tag, false), |name| (name, true));
        let valid = if closing {
            matches!(name, "b" | "i" | "color" | "size" | "shake" | "rb" | "rt")
        } else {
            matches!(name, "b" | "i" | "shake" | "rb" | "rt")
                || name.strip_prefix("color=#").is_some_and(kairos_color)
                || name.strip_prefix("size=").is_some_and(|value| value.parse::<u16>().is_ok_and(|n| n > 0))
                || name
                    .strip_prefix("w=")
                    .is_some_and(|value| value.parse::<f32>().is_ok_and(|n| n.is_finite() && n >= 0.0))
        };
        if !valid {
            return Err(format!("invalid text tag `{{{tag}}}`"));
        }
        if let Some(value) = name.strip_prefix("w=") {
            if closing {
                return Err("inline wait tags cannot close".into());
            }
            waits.push((visible_chars, value.parse().unwrap_or_default()));
        } else if closing {
            let Some((opened, previous)) = stack.pop() else {
                return Err(format!("mismatched text tag `{{{tag}}}`"));
            };
            if opened != name {
                return Err(format!("mismatched text tag `{{{tag}}}`"));
            }
            style = previous;
            if name == "rb" {
                if let Some(start) = ruby_base_start.take() {
                    ruby_bases.push(start..plain.len());
                }
            } else if name == "rt"
                && let (Some(base), Some(start)) = (ruby_bases.pop(), ruby_reading_start.take())
            {
                ruby.push((base, start..plain.len()));
            }
        } else {
            stack.push((
                if name.starts_with("color=") {
                    "color"
                } else if name.starts_with("size=") {
                    "size"
                } else {
                    name
                }
                .into(),
                style,
            ));
            if name == "rb" {
                ruby_base_start = Some(plain.len());
            } else if name == "rt" {
                ruby_reading_start = Some(plain.len());
            }
            if let Some(value) = name.strip_prefix("color=#") {
                style.color = Some(parse_color(value));
            } else if let Some(value) = name.strip_prefix("size=") {
                style.size = value.parse().ok();
            } else {
                match name {
                    "b" => style.bold = true,
                    "i" => style.italic = true,
                    "shake" => style.shake = true,
                    _ => {}
                }
            }
        }
        rest = &after_start[end + 1..];
    }
    if let Some((tag, _)) = stack.last() {
        return Err(format!("unclosed text tag `{{{tag}}}`"));
    }
    let mut boundaries: Vec<usize> =
        ruby.iter().flat_map(|(base, reading)| [base.start, base.end, reading.start, reading.end]).collect();
    boundaries.sort_unstable();
    boundaries.dedup();
    let spans = spans
        .into_iter()
        .flat_map(|span| {
            let mut start = span.range.start;
            boundaries
                .iter()
                .copied()
                .filter(move |&boundary| boundary > span.range.start && boundary < span.range.end)
                .chain([span.range.end])
                .map(move |end| {
                    let piece = TextSpan { range: start..end, style: span.style };
                    start = end;
                    piece
                })
        })
        .collect();
    Ok(TextMarkup { plain, visible_chars, spans, waits, ruby })
}

pub fn normalize_text_for_id(text: &str) -> String {
    parse_text_markup(text).map_or_else(|_| text.to_string(), |markup| markup.plain)
}

fn append(text: &str, style: TextStyle, plain: &mut String, spans: &mut Vec<TextSpan>, visible_chars: &mut usize) {
    if text.is_empty() {
        return;
    }
    let unescaped = text.replace("}}", "}");
    let start = plain.len();
    plain.push_str(&unescaped);
    let end = plain.len();
    if let Some(last) = spans.last_mut().filter(|span| span.range.end == start && span.style == style) {
        last.range.end = end;
    } else {
        spans.push(TextSpan { range: start..end, style });
    }
    *visible_chars += unescaped.chars().count();
}

fn kairos_color(value: &str) -> bool {
    value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_color(value: &str) -> [u8; 3] {
    [0, 2, 4].map(|offset| u8::from_str_radix(&value[offset..offset + 2], 16).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_preserves_visible_runs_waits_and_ruby_ranges() {
        let parsed = parse_text_markup("{b}Hi{/b} {w=0.5}{rb}日{/rb}{rt}にち{/rt}").unwrap();
        assert_eq!(parsed.plain, "Hi 日にち");
        assert_eq!(parsed.visible_chars, 6);
        assert_eq!(parsed.waits, [(3, 0.5)]);
        assert_eq!(parsed.ruby, [(3..6, 6..12)]);
        assert!(parsed.spans[0].style.bold);
    }

    #[test]
    fn invalid_and_unbalanced_tags_are_reported() {
        for text in ["{bad}x", "{b}x", "{/b}x", "{color=#xyz}x{/color}", "{size=0}x"] {
            assert!(parse_text_markup(text).is_err(), "{text}");
        }
    }

    #[test]
    fn escaped_opening_and_closing_braces_are_unescaped() {
        let parsed = parse_text_markup("{{open}} and {{close}}").unwrap();
        assert_eq!(parsed.plain, "{open} and {close}");
    }

    #[test]
    fn ruby_ranges_are_split_into_styling_spans() {
        let parsed = parse_text_markup("A{rb}漢{/rb}{rt}kan{/rt}Z").unwrap();
        assert_eq!(parsed.spans.iter().map(|span| span.range.clone()).collect::<Vec<_>>(), [0..1, 1..4, 4..7, 7..8]);
    }
}
