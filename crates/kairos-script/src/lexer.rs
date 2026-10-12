use crate::Diagnostic;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Op(&'static str),
}

const OPS: [&str; 22] = [
    "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "(", ")", ":", ",", "=", "<", ">", "+", "-", "*", "/", "%", "$",
    "[",
];

/// Strips a `#` comment that is not inside a string literal.
pub fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' => return &line[..i],
            None => {}
        }
    }
    line
}

pub fn tokenize(text: &str, line: usize) -> Result<Vec<Tok>, Diagnostic> {
    let chars: Vec<char> = text.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\'' {
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(Diagnostic::new(line, "unterminated string")),
                    Some('\\') => {
                        match chars.get(i + 1) {
                            Some('n') => s.push('\n'),
                            Some(&e) => s.push(e),
                            None => return Err(Diagnostic::new(line, "unterminated string")),
                        }
                        i += 2;
                    }
                    Some(&q) if q == c => {
                        i += 1;
                        break;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        i += 1;
                    }
                }
            }
            toks.push(Tok::Str(s));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let n: String = chars[start..i].iter().collect();
            let tok = if n.contains('.') { n.parse().map(Tok::Float).ok() } else { n.parse().map(Tok::Int).ok() };
            toks.push(tok.ok_or_else(|| Diagnostic::new(line, format!("bad number `{n}`")))?);
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                i += 1;
            }
            toks.push(Tok::Ident(chars[start..i].iter().collect()));
        } else {
            let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let op = OPS
                .iter()
                .find(|op| rest.starts_with(**op))
                .ok_or_else(|| Diagnostic::new(line, format!("unexpected character `{c}`")))?;
            toks.push(Tok::Op(op));
            i += op.len();
        }
    }
    Ok(toks)
}
