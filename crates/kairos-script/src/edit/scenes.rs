use serde::{Deserialize, Serialize};

use crate::ast::StmtKind;

use super::ops::{EditError, Focus, Op, SourceFile, Workspace, step};
use super::text::{Lines, quote};
use super::tree::{self, Node};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    Jump,
    Call,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub path: String,
    pub line: usize,
    pub kind: RefKind,
    pub choice: Option<String>,
}

pub fn references(files: &[SourceFile], label: &str) -> Vec<Reference> {
    let mut out = Vec::new();
    for f in files {
        let (stmts, _) = crate::parse(&f.source);
        let lines = Lines::parse(&f.source);
        collect_refs(&tree::build(&stmts, &lines), label, &f.path, None, &mut out);
    }
    out
}

fn collect_refs(nodes: &[Node], label: &str, path: &str, choice: Option<&str>, out: &mut Vec<Reference>) {
    for n in nodes {
        match &n.stmt.kind {
            StmtKind::Jump(t) | StmtKind::Call(t) if t == label => out.push(Reference {
                path: path.into(),
                line: n.line,
                kind: if matches!(n.stmt.kind, StmtKind::Jump(_)) { RefKind::Jump } else { RefKind::Call },
                choice: choice.map(str::to_string),
            }),
            StmtKind::Menu { choices, .. } => {
                for (c, b) in choices.iter().zip(&n.bodies) {
                    collect_refs(&b.nodes, label, path, Some(&c.text), out);
                }
            }
            _ => n.bodies.iter().for_each(|b| collect_refs(&b.nodes, label, path, None, out)),
        }
    }
}

fn label_names(ws: &Workspace) -> Vec<String> {
    ws.files
        .iter()
        .flat_map(|f| crate::parse(&f.source).0)
        .filter_map(|s| match s.kind {
            StmtKind::Label { name, .. } => Some(name),
            _ => None,
        })
        .collect()
}

fn refuse_taken(ws: &Workspace, name: &str) -> Result<(), EditError> {
    if label_names(ws).iter().any(|l| l == name) {
        return Err(EditError::Invalid(format!("a scene named `{name}` already exists")));
    }
    Ok(())
}

fn defining_file(ws: &Workspace, label: &str) -> Result<String, EditError> {
    ws.files
        .iter()
        .find(|f| {
            crate::parse(&f.source).0.iter().any(|s| matches!(&s.kind, StmtKind::Label { name, .. } if name == label))
        })
        .map(|f| f.path.clone())
        .ok_or_else(|| EditError::NotFound(format!("there is no scene `{label}`")))
}

/// Applies `edit` to the lines and statement tree of `path` and stores the result.
fn with_file<T>(
    ws: &mut Workspace,
    path: &str,
    edit: impl FnOnce(&mut Lines, &[Node], usize) -> Result<T, EditError>,
) -> Result<T, EditError> {
    let source = ws.source(path)?.to_string();
    let (stmts, _) = crate::parse(&source);
    let mut lines = Lines::parse(&source);
    let nodes = tree::build(&stmts, &lines);
    let step = step(&lines);
    let out = edit(&mut lines, &nodes, step)?;
    ws.set(path, lines.join());
    Ok(out)
}

fn scene_text(name: &str, step: usize) -> Vec<String> {
    let pad = " ".repeat(step);
    vec![format!("label {name}:"), format!("{pad}\"\""), format!("{pad}return")]
}

fn append_to_body(lines: &mut Lines, label: &Node, text: Vec<String>, step: usize) -> usize {
    let body = &label.bodies[0];
    let indent = body.nodes.first().map_or(label.indent + step, |n| n.indent);
    let new = text.into_iter().map(|l| format!("{}{l}", " ".repeat(indent))).collect();
    let at = body.end + 1;
    lines.splice(at, at - 1, new);
    at
}

/// The lines that remove one `jump`/`call` to `to` from `nodes`: the whole choice when that
/// statement is all the choice does, and the whole menu when that was its last choice.
fn removal(nodes: &[Node], to: &str, choice: Option<&str>) -> Option<(usize, usize)> {
    for n in nodes {
        match &n.stmt.kind {
            StmtKind::Jump(t) | StmtKind::Call(t) if t == to && choice.is_none() => return Some((n.line, n.end)),
            StmtKind::Menu { choices, .. } => {
                for (c, b) in choices.iter().zip(&n.bodies) {
                    let only_link = b.nodes.len() == 1
                        && matches!(&b.nodes[0].stmt.kind, StmtKind::Jump(t) | StmtKind::Call(t) if t == to);
                    let wanted = choice.is_none_or(|text| text == c.text);
                    if wanted && only_link {
                        return Some(if choices.len() == 1 { (n.line, n.end) } else { (b.header, b.end) });
                    }
                    if wanted
                        && choice.is_some()
                        && let Some(r) = removal(&b.nodes, to, None)
                    {
                        return Some(r);
                    }
                }
            }
            _ => {}
        }
        for b in &n.bodies {
            if let Some(r) = removal(&b.nodes, to, choice) {
                return Some(r);
            }
        }
    }
    None
}

/// Removes `first..=last`, leaving `pass` when that empties the body around it so it still parses.
fn cut(lines: &mut Lines, nodes: &[Node], first: usize, last: usize) {
    let emptied =
        tree::parent_body(nodes, first).is_some_and(|b| b.nodes.iter().all(|c| (first..=last).contains(&c.line)));
    let filler = if emptied { vec![format!("{}pass", " ".repeat(lines.indent(first)))] } else { Vec::new() };
    lines.splice(first, last, filler);
}

pub(crate) fn apply_scene_op(ws: &mut Workspace, op: &Op) -> Result<Option<Focus>, EditError> {
    let focus = match op {
        Op::AddScene { path, name, after } => {
            refuse_taken(ws, name)?;
            let was_empty = ws.source(path)?.is_empty();
            let line = with_file(ws, path, |lines, nodes, step| {
                let mut text = scene_text(name, step);
                let at = match after.as_deref().and_then(|a| tree::label(nodes, a)) {
                    Some(n) => n.end + 1,
                    None => lines.len() + 1,
                };
                let blank = usize::from(!was_empty);
                if !was_empty {
                    text.insert(0, String::new());
                }
                lines.splice(at, at - 1, text);
                Ok(at + blank)
            })?;
            // An empty source has no line ending to keep; end it the way every other script does.
            if was_empty {
                let source = format!("{}\n", ws.source(path)?);
                ws.set(path, source);
            }
            Focus { path: path.clone(), line }
        }
        Op::RenameScene { from, to } => {
            refuse_taken(ws, to)?;
            let home = defining_file(ws, from)?;
            let paths: Vec<String> = ws.files.iter().map(|f| f.path.clone()).collect();
            let mut focus = None;
            for path in paths {
                let file = SourceFile { path: path.clone(), source: ws.source(&path)?.into() };
                let refs: Vec<usize> = references(&[file], from).iter().map(|r| r.line).collect();
                with_file(ws, &path, |lines, nodes, _| {
                    for line in refs {
                        let renamed = rename_target(lines.text(line), from, to);
                        lines.splice(line, line, vec![renamed]);
                    }
                    if let Some(n) = tree::label(nodes, from) {
                        let renamed = lines.text(n.line).replacen(&format!("label {from}"), &format!("label {to}"), 1);
                        lines.splice(n.line, n.line, vec![renamed]);
                        focus = Some(Focus { path: path.clone(), line: n.line });
                    }
                    Ok(())
                })?;
            }
            focus.unwrap_or(Focus { path: home, line: 1 })
        }
        Op::DeleteScene { label, remove_references } => {
            let home = defining_file(ws, label)?;
            let line = with_file(ws, &home, |lines, nodes, _| {
                let n = tree::label(nodes, label).expect("defining_file found it");
                let first = if n.line > 1 && lines.text(n.line - 1).trim().is_empty() { n.line - 1 } else { n.line };
                lines.splice(first, n.end, Vec::new());
                Ok(first.max(1))
            })?;
            if *remove_references {
                let paths: Vec<String> = ws.files.iter().map(|f| f.path.clone()).collect();
                for path in paths {
                    while with_file(ws, &path, |lines, nodes, _| {
                        let Some((first, last)) = removal(nodes, label, None) else { return Ok(false) };
                        cut(lines, nodes, first, last);
                        Ok(true)
                    })? {}
                }
            }
            Focus { path: home, line }
        }
        Op::DuplicateScene { label, name } => {
            refuse_taken(ws, name)?;
            let home = defining_file(ws, label)?;
            let line = with_file(ws, &home, |lines, nodes, _| {
                let n = tree::label(nodes, label).expect("defining_file found it");
                let mut copy: Vec<String> = (n.line..=n.end).map(|l| lines.text(l).to_string()).collect();
                copy[0] = copy[0].replacen(&format!("label {label}"), &format!("label {name}"), 1);
                copy.insert(0, String::new());
                lines.splice(n.end + 1, n.end, copy);
                Ok(n.end + 2)
            })?;
            Focus { path: home, line }
        }
        Op::Connect { from, to, choice } => {
            let home = defining_file(ws, from)?;
            let line = with_file(ws, &home, |lines, nodes, step| {
                let n = tree::label(nodes, from).expect("defining_file found it");
                let last = n.bodies[0].nodes.last();
                match choice {
                    None => match last.filter(|l| matches!(&l.stmt.kind, StmtKind::Jump(t) if t == to)) {
                        Some(jump) => Ok(jump.line),
                        None => Ok(append_to_body(lines, n, vec![format!("jump {to}")], step)),
                    },
                    Some(text) => match last.filter(|l| matches!(l.stmt.kind, StmtKind::Menu { .. })) {
                        Some(menu) => {
                            let option = menu.bodies.first().map_or(menu.indent + step, |b| lines.indent(b.header));
                            let new = vec![
                                format!("{}{}:", " ".repeat(option), quote(text)),
                                format!("{}jump {to}", " ".repeat(option + step)),
                            ];
                            lines.splice(menu.end + 1, menu.end, new);
                            Ok(menu.end + 1)
                        }
                        None => Ok(append_to_body(
                            lines,
                            n,
                            vec![
                                "menu:".into(),
                                format!("{}{}:", " ".repeat(step), quote(text)),
                                format!("{}jump {to}", " ".repeat(step * 2)),
                            ],
                            step,
                        )),
                    },
                }
            })?;
            Focus { path: home, line }
        }
        Op::RemoveLink { from, to, choice } => {
            let home = defining_file(ws, from)?;
            let line = with_file(ws, &home, |lines, nodes, _| {
                let n = tree::label(nodes, from).expect("defining_file found it");
                let (first, last) = removal(&n.bodies[0].nodes, to, choice.as_deref())
                    .ok_or_else(|| EditError::NotFound(format!("`{from}` does not lead to `{to}` that way")))?;
                cut(lines, nodes, first, last);
                Ok(n.line)
            })?;
            Focus { path: home, line }
        }
        Op::EndScene { label } => {
            let home = defining_file(ws, label)?;
            let line = with_file(ws, &home, |lines, nodes, step| {
                let n = tree::label(nodes, label).expect("defining_file found it");
                Ok(append_to_body(lines, n, vec!["return".into()], step))
            })?;
            Focus { path: home, line }
        }
        Op::SetCharacter { path, id, name, color } => {
            let line = with_file(ws, path, |lines, nodes, _| {
                let color = color.as_ref().map(|c| format!(" color={}", quote(c))).unwrap_or_default();
                let text = format!("character {id} {}{color}", quote(name));
                let existing =
                    nodes.iter().find(|n| matches!(&n.stmt.kind, StmtKind::Character { id: i, .. } if i == id));
                if let Some(n) = existing {
                    lines.splice(n.line, n.line, vec![text]);
                    return Ok(n.line);
                }
                let at = nodes
                    .iter()
                    .filter(|n| matches!(n.stmt.kind, StmtKind::Character { .. }))
                    .map(|n| n.end + 1)
                    .max()
                    .unwrap_or(1);
                lines.splice(at, at - 1, vec![text]);
                Ok(at)
            })?;
            Focus { path: path.clone(), line }
        }
        _ => return Ok(None),
    };
    Ok(Some(focus))
}

/// Rewrites the target of a `jump`/`call` line, leaving prefixes such as `day_two` alone.
fn rename_target(raw: &str, from: &str, to: &str) -> String {
    for kw in ["jump", "call"] {
        let pat = format!("{kw} {from}");
        if let Some(i) = raw.find(&pat) {
            let after = &raw[i + pat.len()..];
            if after.chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_')) {
                return format!("{}{kw} {to}{after}", &raw[..i]);
            }
        }
    }
    raw.to_string()
}
