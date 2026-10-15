use crate::lexer::strip_comment;

pub(crate) struct Lines {
    pub lines: Vec<String>,
    eol: &'static str,
    trailing: bool,
}

impl Lines {
    pub fn parse(src: &str) -> Lines {
        let eol = if src.contains("\r\n") { "\r\n" } else { "\n" };
        let trailing = src.ends_with('\n');
        let mut lines: Vec<String> = src.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect();
        if trailing || src.is_empty() {
            lines.pop();
        }
        Lines { lines, eol, trailing }
    }

    pub fn join(&self) -> String {
        let mut out = self.lines.join(self.eol);
        if self.trailing && !self.lines.is_empty() {
            out.push_str(self.eol);
        }
        out
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn text(&self, n: usize) -> &str {
        self.lines.get(n - 1).map(String::as_str).unwrap_or("")
    }

    pub fn indent(&self, n: usize) -> usize {
        let t = self.text(n);
        t.char_indices().find(|(_, character)| !character.is_whitespace()).map_or(t.len(), |(index, _)| index)
    }

    pub fn is_code(&self, n: usize) -> bool {
        !strip_comment(self.text(n)).trim().is_empty()
    }

    /// The last code line in `from..=to`, or `from` when there is none after it.
    pub fn last_code(&self, from: usize, to: usize) -> usize {
        (from..=to.min(self.len())).rev().find(|&n| self.is_code(n)).unwrap_or(from)
    }

    /// Replaces the 1-based inclusive range `first..=last`; `last == first - 1` inserts before `first`.
    pub fn splice(&mut self, first: usize, last: usize, with: Vec<String>) {
        self.lines.splice(first - 1..last, with);
    }
}

pub(crate) fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const KEYWORDS: &[&str] = &[
    "label",
    "menu",
    "if",
    "elif",
    "else",
    "jump",
    "call",
    "return",
    "show",
    "hide",
    "scene",
    "play",
    "queue",
    "stop",
    "pause",
    "with",
    "define",
    "default",
    "character",
    "transform",
    "voice",
    "open",
    "quit",
    "pass",
    "not",
    "and",
    "or",
    "in",
    "true",
    "false",
    "none",
];

/// Turns a name a person typed into a label: lowercase letters, digits and `_`, unique among `taken`.
/// A typed keyword such as "Return" gets a suffix so the label never reads like a statement; the
/// `scene` fallback for a name with no usable characters is kept as is.
pub fn label_from_name(name: &str, taken: &[String]) -> String {
    let mut base = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            base.push(c);
        } else if !base.ends_with('_') && !base.is_empty() {
            base.push('_');
        }
    }
    let mut base = base.trim_end_matches('_').to_string();
    let keyword = KEYWORDS.contains(&base.as_str());
    if base.is_empty() {
        base = "scene".into();
    }
    if base.starts_with(|c: char| c.is_ascii_digit()) {
        base.insert(0, '_');
    }
    if !keyword && !taken.contains(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}_{n}")).find(|c| !taken.contains(c)).expect("some suffix is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_round_trip_crlf_and_missing_final_newline() {
        for src in ["a\r\nb\r\n", "a\nb", "", "a\n\n# c\n"] {
            assert_eq!(Lines::parse(src).join(), src);
        }
    }

    #[test]
    fn splice_inserts_and_replaces_with_one_based_ranges() {
        let mut l = Lines::parse("a\nb\nc\n");
        l.splice(2, 2, vec!["B".into(), "B2".into()]);
        assert_eq!(l.join(), "a\nB\nB2\nc\n");
        l.splice(1, 0, vec!["top".into()]);
        assert_eq!(l.join(), "top\na\nB\nB2\nc\n");
        l.splice(5, 5, vec![]);
        assert_eq!(l.join(), "top\na\nB\nB2\n");
    }

    #[test]
    fn code_lines_skip_blanks_and_comments() {
        let l = Lines::parse("label a:\n    # note\n\n    \"x\"  # trailing\n");
        assert!(l.is_code(1) && !l.is_code(2) && !l.is_code(3) && l.is_code(4));
        assert_eq!(l.last_code(1, 4), 4);
        assert_eq!(l.last_code(1, 3), 1);
        assert_eq!(l.indent(4), 4);
    }

    #[test]
    fn quote_escapes_like_the_lexer_reads() {
        assert_eq!(quote(r#"say "hi" \ now"#), r#""say \"hi\" \\ now""#);
        assert_eq!(quote("two\nlines"), r#""two\nlines""#);
    }

    #[test]
    fn scene_names_become_unique_labels() {
        let taken = vec!["beach_day".to_string(), "beach_day_2".to_string()];
        assert_eq!(label_from_name("Beach Day", &taken), "beach_day_3");
        assert_eq!(label_from_name("  Café — night!", &[]), "caf_night");
        assert_eq!(label_from_name("2nd act", &[]), "_2nd_act");
        assert_eq!(label_from_name("!!!", &[]), "scene");
        assert_eq!(label_from_name("Return", &[]), "return_2");
        assert_eq!(label_from_name("None", &[]), "none_2");
    }
}
