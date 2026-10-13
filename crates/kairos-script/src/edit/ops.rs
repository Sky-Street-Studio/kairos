use serde::{Deserialize, Serialize};

use crate::ast::StmtKind;
use crate::lexer::strip_comment;

use super::blocks::{BlockKind, after_string, render};
use super::text::{Lines, quote};
use super::tree::{self, Node};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "camelCase")]
pub enum Place {
    After { line: usize },
    Start { header: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Op {
    Insert { path: String, place: Place, block: BlockKind },
    Replace { path: String, line: usize, block: BlockKind },
    Move { path: String, line: usize, place: Place },
    Delete { path: String, line: usize },
    SetChoiceText { path: String, line: usize, text: String },
    SetChoiceCondition { path: String, line: usize, condition: Option<String> },
    AddChoiceOption { path: String, line: usize, text: String },
    SetBranchCondition { path: String, line: usize, condition: String },
    AddBranch { path: String, line: usize, condition: Option<String> },
    AddScene { path: String, name: String, after: Option<String> },
    RenameScene { from: String, to: String },
    DeleteScene { label: String, remove_references: bool },
    DuplicateScene { label: String, name: String },
    Connect { from: String, to: String, choice: Option<String> },
    RemoveLink { from: String, to: String, choice: Option<String> },
    EndScene { label: String },
    SetCharacter { path: String, id: String, name: String, color: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub files: Vec<SourceFile>,
    pub focus: Option<Focus>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("this change would break the script: {0}")]
    WouldBreak(String),
}

pub(crate) struct Workspace {
    pub files: Vec<SourceFile>,
    original: Vec<SourceFile>,
}

impl Workspace {
    pub fn new(files: &[SourceFile]) -> Workspace {
        Workspace { files: files.to_vec(), original: files.to_vec() }
    }

    pub fn source(&self, path: &str) -> Result<&str, EditError> {
        self.files
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.source.as_str())
            .ok_or_else(|| EditError::NotFound(format!("{path} is not open")))
    }

    pub fn set(&mut self, path: &str, source: String) {
        if let Some(f) = self.files.iter_mut().find(|f| f.path == path) {
            f.source = source;
        }
    }

    pub fn finish(self, focus: Option<Focus>) -> Result<Applied, EditError> {
        let mut changed = Vec::new();
        for (f, before) in self.files.into_iter().zip(self.original) {
            if f.source == before.source {
                continue;
            }
            let (_, old) = crate::parse(&before.source);
            let (_, new) = crate::parse(&f.source);
            if new.len() > old.len() {
                let fresh = new.iter().find(|d| !old.iter().any(|o| o.message == d.message)).unwrap_or(&new[0]);
                return Err(EditError::WouldBreak(fresh.message.clone()));
            }
            changed.push(f);
        }
        Ok(Applied { files: changed, focus })
    }
}

pub fn apply(files: &[SourceFile], ops: &[Op]) -> Result<Applied, EditError> {
    let mut ws = Workspace::new(files);
    let mut focus = None;
    for op in ops {
        focus = Some(apply_one(&mut ws, op)?);
    }
    ws.finish(focus)
}

/// One step of indentation in this file: the first indented line's width, or 4.
pub(crate) fn step(lines: &Lines) -> usize {
    (1..=lines.len()).filter(|&n| lines.is_code(n)).map(|n| lines.indent(n)).find(|&i| i > 0).unwrap_or(4)
}

fn missing(line: usize) -> EditError {
    EditError::NotFound(format!("no statement starts on line {line}"))
}

/// Fails unless `header` heads a body of a statement whose kind passes `kind`, so a choice edit
/// can never rewrite a label or branch line.
fn owned_by(nodes: &[Node], header: usize, kind: impl Fn(&StmtKind) -> bool) -> Result<(), EditError> {
    tree::owner(nodes, header).filter(|n| kind(&n.stmt.kind)).map(|_| ()).ok_or_else(|| missing(header))
}

/// Where `place` puts new lines: the 1-based line to insert before, the indentation, and the line
/// of a lone `pass` there that the new lines replace.
fn resolve(nodes: &[Node], lines: &Lines, place: &Place) -> Result<(usize, usize, Option<usize>), EditError> {
    match place {
        Place::After { line } => {
            let n = tree::find(nodes, *line).ok_or_else(|| missing(*line))?;
            Ok((n.end + 1, n.indent, None))
        }
        Place::Start { header } => {
            let body = tree::find_body(nodes, *header).ok_or_else(|| missing(*header))?;
            let indent = body.nodes.first().map_or(lines.indent(*header) + step(lines), |n| n.indent);
            let pass = match body.nodes.as_slice() {
                [only] if matches!(only.stmt.kind, StmtKind::Pass) => Some(only.line),
                _ => None,
            };
            Ok((header + 1, indent, pass))
        }
    }
}

fn reindent(text: &[String], from: usize, to: usize) -> Vec<String> {
    text.iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                let cut = l.len() - l.trim_start().len();
                format!("{}{}", " ".repeat(to), &l[cut.min(from)..])
            }
        })
        .collect()
}

fn insert_lines(lines: &mut Lines, at: usize, pass: Option<usize>, new: Vec<String>) -> usize {
    match pass {
        Some(p) => {
            lines.splice(p, p, new);
            p
        }
        None => {
            lines.splice(at, at - 1, new);
            at
        }
    }
}

/// The comment ending `s`, with the spaces before its `#`, or "" when there is none.
fn trailing_comment(s: &str) -> &str {
    &s[strip_comment(s).trim_end().len()..]
}

fn apply_one(ws: &mut Workspace, op: &Op) -> Result<Focus, EditError> {
    if let Some(focus) = super::scenes::apply_scene_op(ws, op)? {
        return Ok(focus);
    }
    let path = match op {
        Op::Insert { path, .. }
        | Op::Replace { path, .. }
        | Op::Move { path, .. }
        | Op::Delete { path, .. }
        | Op::SetChoiceText { path, .. }
        | Op::SetChoiceCondition { path, .. }
        | Op::AddChoiceOption { path, .. }
        | Op::SetBranchCondition { path, .. }
        | Op::AddBranch { path, .. } => path.clone(),
        _ => unreachable!("scene ops are handled above"),
    };
    let source = ws.source(&path)?.to_string();
    let (stmts, _) = crate::parse(&source);
    let mut lines = Lines::parse(&source);
    let nodes = tree::build(&stmts, &lines);
    let step = step(&lines);
    let line = match op {
        Op::Insert { place, block, .. } => {
            let (at, indent, pass) = resolve(&nodes, &lines, place)?;
            insert_lines(&mut lines, at, pass, render(block, indent, step))
        }
        Op::Replace { line, block, .. } => {
            let n = tree::find(&nodes, *line).ok_or_else(|| missing(*line))?;
            if !n.bodies.is_empty() || matches!(block, BlockKind::Choice { .. } | BlockKind::If { .. }) {
                return Err(EditError::Invalid("choices and conditions change through their own options".into()));
            }
            let mut new = render(block, n.indent, step);
            let comment = trailing_comment(lines.text(n.line));
            if n.line == n.end && new.len() == 1 && !comment.is_empty() {
                new[0].push_str(comment);
            }
            lines.splice(n.line, n.end, new);
            n.line
        }
        Op::Delete { line, .. } => {
            let n = tree::find(&nodes, *line).ok_or_else(|| missing(*line))?;
            let alone = tree::parent_body(&nodes, *line).is_some_and(|b| b.nodes.len() == 1);
            let replacement = if alone { vec![format!("{}pass", " ".repeat(n.indent))] } else { Vec::new() };
            lines.splice(n.line, n.end, replacement);
            n.line.saturating_sub(1).max(1)
        }
        Op::Move { line, place, .. } => {
            let n = tree::find(&nodes, *line).ok_or_else(|| missing(*line))?;
            let target = match place {
                Place::After { line } | Place::Start { header: line } => *line,
            };
            if (n.line..=n.end).contains(&target) {
                return Err(EditError::Invalid("a block cannot move inside itself".into()));
            }
            let text: Vec<String> = (n.line..=n.end).map(|l| lines.text(l).to_string()).collect();
            let (from, removed) = (n.indent, n.end - n.line + 1);
            let alone = tree::parent_body(&nodes, *line).is_some_and(|b| b.nodes.len() == 1);
            let filler = if alone { vec![format!("{}pass", " ".repeat(from))] } else { Vec::new() };
            let shift = removed - filler.len();
            lines.splice(n.line, n.end, filler);
            let place = match place {
                Place::After { line: l } if *l > n.end => Place::After { line: l - shift },
                Place::Start { header: h } if *h > n.end => Place::Start { header: h - shift },
                other => other.clone(),
            };
            let source = lines.join();
            let (stmts, _) = crate::parse(&source);
            let nodes = tree::build(&stmts, &lines);
            let (at, indent, pass) = resolve(&nodes, &lines, &place)?;
            insert_lines(&mut lines, at, pass, reindent(&text, from, indent))
        }
        Op::SetChoiceText { line, text, .. } => {
            owned_by(&nodes, *line, |k| matches!(k, StmtKind::Menu { .. }))?;
            let raw = lines.text(*line).to_string();
            let start = raw.len() - raw.trim_start().len();
            let rest = after_string(&raw[start..]);
            lines.splice(*line, *line, vec![format!("{}{}{rest}", &raw[..start], quote(text))]);
            *line
        }
        Op::SetChoiceCondition { line, condition, .. } => {
            owned_by(&nodes, *line, |k| matches!(k, StmtKind::Menu { .. }))?;
            let raw = lines.text(*line).to_string();
            let start = raw.len() - raw.trim_start().len();
            let after = after_string(&raw[start..]);
            let literal = &raw[start..raw.len() - after.len()];
            let cond = condition.as_ref().map(|c| format!(" if {c}")).unwrap_or_default();
            lines.splice(*line, *line, vec![format!("{}{literal}{cond}:{}", &raw[..start], trailing_comment(after))]);
            *line
        }
        Op::AddChoiceOption { line, text, .. } => {
            let n = tree::find(&nodes, *line)
                .filter(|n| matches!(n.stmt.kind, StmtKind::Menu { .. }))
                .ok_or_else(|| missing(*line))?;
            let option_indent = n.bodies.first().map_or(n.indent + step, |b| lines.indent(b.header));
            let new = vec![
                format!("{}{}:", " ".repeat(option_indent), quote(text)),
                format!("{}pass", " ".repeat(option_indent + step)),
            ];
            lines.splice(n.end + 1, n.end, new);
            n.end + 1
        }
        Op::SetBranchCondition { line, condition, .. } => {
            owned_by(&nodes, *line, |k| matches!(k, StmtKind::If(_)))?;
            let raw = lines.text(*line).to_string();
            let head = raw.trim_start();
            if head.starts_with("else") {
                return Err(EditError::Invalid("`otherwise` has no condition".into()));
            }
            let keyword = if head.starts_with("elif") { "elif" } else { "if" };
            let indent = raw.len() - head.len();
            let comment = trailing_comment(&raw);
            lines.splice(*line, *line, vec![format!("{}{keyword} {condition}:{comment}", " ".repeat(indent))]);
            *line
        }
        Op::AddBranch { line, condition, .. } => {
            let n = tree::find(&nodes, *line)
                .filter(|n| matches!(n.stmt.kind, StmtKind::If(_)))
                .ok_or_else(|| missing(*line))?;
            let last = n.bodies.last().map_or(n.line, |b| b.header);
            if lines.text(last).trim_start().starts_with("else") {
                return Err(EditError::Invalid("this condition already has an `otherwise`".into()));
            }
            let head = condition.as_ref().map_or_else(|| "else:".into(), |c| format!("elif {c}:"));
            let pad = " ".repeat(n.indent);
            lines.splice(n.end + 1, n.end, vec![format!("{pad}{head}"), format!("{pad}{}pass", " ".repeat(step))]);
            n.end + 1
        }
        _ => unreachable!("scene ops are handled above"),
    };
    ws.set(&path, lines.join());
    Ok(Focus { path, line })
}
