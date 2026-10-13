//! Parser for the Kairos script language (`.kai`).

pub mod ast;
pub mod edit;
mod lexer;
mod parser;
mod text;

pub use ast::*;
pub use parser::{parse, parse_expr};
pub use text::{TextMarkup, TextSpan, TextStyle, parse_text_markup};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: usize,
    pub message: String,
}

impl Diagnostic {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Diagnostic { line, message: message.into() }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
