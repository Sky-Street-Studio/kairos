use serde::{Deserialize, Serialize};

use crate::ast::{Stmt, StmtKind};

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

/// The lines that remove one `jump`/`call` to `to` from `nodes`. Without `choice`, a link outside
/// any choice wins over one inside a choice.
fn removal(nodes: &[Node], to: &str, choice: Option<&str>) -> Option<(usize, usize)> {
    match choice {
        None => plain_link(nodes, to).or_else(|| choice_link(nodes, to, None)),
        Some(_) => choice_link(nodes, to, choice),
    }
}

fn links_to(n: &Node, to: &str) -> bool {
    matches!(&n.stmt.kind, StmtKind::Jump(t) | StmtKind::Call(t) if t == to)
}

fn plain_link(nodes: &[Node], to: &str) -> Option<(usize, usize)> {
    nodes.iter().find_map(|n| {
        if links_to(n, to) {
            Some((n.line, n.end))
        } else if matches!(n.stmt.kind, StmtKind::Menu { .. }) {
            None
        } else {
            n.bodies.iter().find_map(|b| plain_link(&b.nodes, to))
        }
    })
}

/// A link inside a matching choice: the whole choice when the link is all it does, and the whole
/// menu when that was its last choice.
fn choice_link(nodes: &[Node], to: &str, choice: Option<&str>) -> Option<(usize, usize)> {
    nodes.iter().find_map(|n| {
        let StmtKind::Menu { choices, .. } = &n.stmt.kind else {
            return n.bodies.iter().find_map(|b| choice_link(&b.nodes, to, choice));
        };
        choices.iter().zip(&n.bodies).find_map(|(c, b)| {
            if choice.is_none_or(|text| text == c.text) {
                if let [only] = b.nodes.as_slice()
                    && links_to(only, to)
                {
                    return Some(if choices.len() == 1 { (n.line, n.end) } else { (b.header, b.end) });
                }
                if let Some(r) = plain_link(&b.nodes, to) {
                    return Some(r);
                }
            }
            choice_link(&b.nodes, to, choice)
        })
    })
}

/// Removes `first..=last`. An emptied scene gets `return` so it ends rather than falling into the
/// next scene; an emptied choice or branch gets `pass` so it still parses.
fn cut(lines: &mut Lines, nodes: &[Node], first: usize, last: usize) {
    let emptied = tree::parent_body(nodes, first)
        .filter(|b| b.nodes.iter().all(|c| (first..=last).contains(&c.line)))
        .map(|b| tree::owner(nodes, b.header).is_some_and(|o| matches!(o.stmt.kind, StmtKind::Label { .. })));
    let filler = match emptied {
        Some(scene) => vec![format!("{}{}", " ".repeat(lines.indent(first)), if scene { "return" } else { "pass" })],
        None => Vec::new(),
    };
    lines.splice(first, last, filler);
}

fn body<'a>(label: &Node<'a>) -> &'a [Stmt] {
    match &label.stmt.kind {
        StmtKind::Label { body, .. } => body,
        _ => &[],
    }
}

/// Whether control never runs off the end of `stmts`. Matches the story graph's fall-through rule.
fn terminates(stmts: &[Stmt]) -> bool {
    let Some(last) = stmts.iter().rfind(|s| !matches!(s.kind, StmtKind::Label { .. })) else {
        return false;
    };
    match &last.kind {
        StmtKind::Jump(_) | StmtKind::Return | StmtKind::Quit => true,
        StmtKind::If(branches) => {
            branches.last().is_some_and(|b| b.cond.is_none()) && branches.iter().all(|b| terminates(&b.body))
        }
        StmtKind::Menu { choices, .. } => !choices.is_empty() && choices.iter().all(|c| terminates(&c.body)),
        _ => false,
    }
}

/// The label `label` falls into when it runs off its end: the next label in compile order, which is
/// file order within a file and path order across files.
fn successor(ws: &Workspace, label: &str) -> Option<String> {
    fn collect(stmts: &[Stmt], out: &mut Vec<String>) {
        for s in stmts {
            if let StmtKind::Label { name, body } = &s.kind {
                out.push(name.clone());
                collect(body, out);
            }
        }
    }
    let mut files: Vec<&SourceFile> = ws.files.iter().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut order = Vec::new();
    for f in files {
        collect(&crate::parse(&f.source).0, &mut order);
    }
    order.iter().position(|l| l == label).and_then(|i| order.get(i + 1).cloned())
}

/// Ends `label` explicitly when it would run off its end, so a scene inserted after it does not
/// take over where it used to lead.
fn seal(ws: &mut Workspace, label: &str) -> Result<(), EditError> {
    let exit = successor(ws, label).map_or_else(|| "return".into(), |next| format!("jump {next}"));
    let home = defining_file(ws, label)?;
    with_file(ws, &home, |lines, nodes, step| {
        let n = tree::label(nodes, label).expect("defining_file found it");
        if !terminates(body(n)) {
            append_to_body(lines, n, vec![exit], step);
        }
        Ok(())
    })
}

fn refuse_unknown(ws: &Workspace, label: &str) -> Result<(), EditError> {
    if label_names(ws).iter().any(|l| l == label) {
        return Ok(());
    }
    Err(EditError::NotFound(format!("there is no scene `{label}`")))
}

fn ends_in_exit<'a, 'b>(label: &'b Node<'a>) -> Option<&'b Node<'a>> {
    label.bodies[0]
        .nodes
        .last()
        .filter(|n| matches!(n.stmt.kind, StmtKind::Return | StmtKind::Jump(_) | StmtKind::Quit))
}

pub(crate) fn apply_scene_op(ws: &mut Workspace, op: &Op) -> Result<Option<Focus>, EditError> {
    let focus = match op {
        Op::AddScene { path, name, after } => {
            refuse_taken(ws, name)?;
            let source = ws.source(path)?;
            let was_empty = source.is_empty();
            let labels: Vec<String> = crate::parse(source)
                .0
                .into_iter()
                .filter_map(|s| match s.kind {
                    StmtKind::Label { name, .. } => Some(name),
                    _ => None,
                })
                .collect();
            let before = match after {
                Some(a) if labels.contains(a) => Some(a.clone()),
                Some(a) => return Err(EditError::NotFound(format!("there is no scene `{a}` in {path}"))),
                None => labels.last().cloned(),
            };
            if let Some(b) = &before {
                seal(ws, b)?;
            }
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
                Ok(first.min(lines.len()).max(1))
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
            seal(ws, label)?;
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
            refuse_unknown(ws, to)?;
            let line = with_file(ws, &home, |lines, nodes, step| {
                let n = tree::label(nodes, from).expect("defining_file found it");
                let exit = ends_in_exit(n);
                let Some(text) = choice else {
                    return match exit.map(|e| (e, &e.stmt.kind)) {
                        Some((e, StmtKind::Jump(t))) if t == to => Ok(e.line),
                        Some((_, StmtKind::Jump(t))) => Err(EditError::Invalid(format!(
                            "This scene already ends by going to `{t}`. Add it as a choice, or remove that arrow first."
                        ))),
                        Some((_, StmtKind::Quit)) => Err(EditError::Invalid(
                            "This scene already ends the game. Add it as a choice, or remove that ending first.".into(),
                        )),
                        Some((e, _)) => {
                            lines.splice(e.line, e.end, vec![format!("{}jump {to}", " ".repeat(e.indent))]);
                            Ok(e.line)
                        }
                        None if terminates(body(n)) => Err(EditError::Invalid(
                            "Every path through this scene already leads somewhere. Add it as a choice instead.".into(),
                        )),
                        None => Ok(append_to_body(lines, n, vec![format!("jump {to}")], step)),
                    };
                };
                let nodes = &n.bodies[0].nodes;
                let before_exit = nodes.len() - usize::from(exit.is_some());
                let menu = before_exit
                    .checked_sub(1)
                    .map(|i| &nodes[i])
                    .filter(|m| matches!(m.stmt.kind, StmtKind::Menu { .. }));
                if let Some(menu) = menu {
                    let option = menu.bodies.first().map_or(menu.indent + step, |b| lines.indent(b.header));
                    let new = vec![
                        format!("{}{}:", " ".repeat(option), quote(text)),
                        format!("{}jump {to}", " ".repeat(option + step)),
                    ];
                    lines.splice(menu.end + 1, menu.end, new);
                    return Ok(menu.end + 1);
                }
                let new = vec![
                    "menu:".into(),
                    format!("{}{}:", " ".repeat(step), quote(text)),
                    format!("{}jump {to}", " ".repeat(step * 2)),
                ];
                match exit {
                    Some(e) => {
                        let pad = " ".repeat(e.indent);
                        lines.splice(e.line, e.line - 1, new.into_iter().map(|l| format!("{pad}{l}")).collect());
                        Ok(e.line)
                    }
                    None => Ok(append_to_body(lines, n, new, step)),
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
                if terminates(body(n)) {
                    return Ok(n.bodies[0].nodes.last().map_or(n.line, |l| l.line));
                }
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
    let code = raw.trim_start();
    let indent = &raw[..raw.len() - code.len()];
    for kw in ["jump", "call"] {
        let Some(rest) = code.strip_prefix(kw) else { continue };
        let target = rest.trim_start();
        let gap = &rest[..rest.len() - target.len()];
        if !gap.is_empty()
            && let Some(after) = target.strip_prefix(from)
            && after.chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        {
            return format!("{indent}{kw}{gap}{to}{after}");
        }
    }
    raw.to_string()
}
