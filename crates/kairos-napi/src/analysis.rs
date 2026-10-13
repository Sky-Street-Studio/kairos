use std::collections::HashSet;

use kairos_script::{Stmt, StmtKind};

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub path: String,
    pub source: String,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct LineDiagnostic {
    pub line: u32,
    pub message: String,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub diagnostics: Vec<LineDiagnostic>,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiagnostic {
    pub path: String,
    pub line: u32,
    pub message: String,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    pub diagnostics: Vec<FileDiagnostic>,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineLabel {
    pub name: String,
    pub line: u32,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineCharacter {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub line: u32,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub labels: Vec<OutlineLabel>,
    pub characters: Vec<OutlineCharacter>,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub label: String,
    pub path: String,
    pub line: u32,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    #[cfg_attr(not(test), napi(ts_type = "'jump' | 'call' | 'choice' | 'fallthrough'"))]
    pub kind: String,
    pub text: Option<String>,
}

#[cfg_attr(not(test), napi_derive::napi(object))]
#[derive(Debug, Clone, PartialEq)]
pub struct StoryGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub fn parse(source: &str) -> ParseResult {
    let (_, diags) = kairos_script::parse(source);
    ParseResult {
        diagnostics: diags.into_iter().map(|d| LineDiagnostic { line: d.line as u32, message: d.message }).collect(),
    }
}

/// Files are compiled in path order, matching how the CLI and player load `script/*.kai`.
fn sorted(files: &[SourceFile]) -> Vec<&SourceFile> {
    let mut sorted: Vec<&SourceFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    sorted
}

pub fn check_project(files: &[SourceFile]) -> CheckResult {
    let files = sorted(files);
    let mut diagnostics = Vec::new();
    let mut parsed = Vec::new();
    for file in &files {
        let (stmts, diags) = kairos_script::parse(&file.source);
        diagnostics.extend(diags.into_iter().map(|d| FileDiagnostic {
            path: file.path.clone(),
            line: d.line as u32,
            message: d.message,
        }));
        parsed.push(stmts);
    }
    let (_, diags) = kairos_vm::compile(&parsed);
    diagnostics.extend(diags.into_iter().map(|(i, d)| FileDiagnostic {
        path: files[i].path.clone(),
        line: d.line as u32,
        message: d.message,
    }));
    diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    CheckResult { diagnostics }
}

pub fn outline(source: &str) -> Outline {
    let (stmts, _) = kairos_script::parse(source);
    let mut out = Outline { labels: Vec::new(), characters: Vec::new() };
    collect_outline(&stmts, &mut out);
    out
}

fn collect_outline(stmts: &[Stmt], out: &mut Outline) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Label { name, body } => {
                out.labels.push(OutlineLabel { name: name.clone(), line: stmt.line as u32 });
                collect_outline(body, out);
            }
            StmtKind::Character { id, name, color } => out.characters.push(OutlineCharacter {
                id: id.clone(),
                name: name.clone(),
                color: color.clone(),
                line: stmt.line as u32,
            }),
            _ => {}
        }
    }
}

struct LabelRef<'a> {
    name: &'a str,
    path: &'a str,
    line: usize,
    body: &'a [Stmt],
}

pub fn story_graph(files: &[SourceFile]) -> StoryGraph {
    let files = sorted(files);
    let parsed: Vec<Vec<Stmt>> = files.iter().map(|f| kairos_script::parse(&f.source).0).collect();
    let mut labels = Vec::new();
    for (file, stmts) in files.iter().zip(&parsed) {
        collect_labels(stmts, &file.path, &mut labels);
    }
    let known: HashSet<&str> = labels.iter().map(|l| l.name).collect();

    let mut edges = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        collect_edges(label.name, label.body, None, &mut edges);
        if !terminates(label.body)
            && let Some(next) = labels.get(i + 1)
        {
            edges.push(GraphEdge {
                from: label.name.into(),
                to: next.name.into(),
                kind: "fallthrough".into(),
                text: None,
            });
        }
    }
    let mut seen = HashSet::new();
    edges.retain(|e| {
        known.contains(e.to.as_str()) && seen.insert((e.from.clone(), e.to.clone(), e.kind.clone(), e.text.clone()))
    });

    let mut names = HashSet::new();
    let nodes = labels
        .iter()
        .filter(|l| names.insert(l.name))
        .map(|l| GraphNode { label: l.name.into(), path: l.path.into(), line: l.line as u32 })
        .collect();
    StoryGraph { nodes, edges }
}

fn collect_labels<'a>(stmts: &'a [Stmt], path: &'a str, out: &mut Vec<LabelRef<'a>>) {
    for stmt in stmts {
        if let StmtKind::Label { name, body } = &stmt.kind {
            out.push(LabelRef { name, path, line: stmt.line, body });
            collect_labels(body, path, out);
        }
    }
}

fn collect_edges(from: &str, stmts: &[Stmt], choice: Option<&str>, out: &mut Vec<GraphEdge>) {
    for stmt in stmts {
        let edge = |to: &str, kind: &str| GraphEdge {
            from: from.into(),
            to: to.into(),
            kind: kind.into(),
            text: choice.map(str::to_string),
        };
        match &stmt.kind {
            StmtKind::Jump(to) => out.push(edge(to, if choice.is_some() { "choice" } else { "jump" })),
            StmtKind::Call(to) => out.push(edge(to, "call")),
            StmtKind::If(branches) => {
                for branch in branches {
                    collect_edges(from, &branch.body, choice, out);
                }
            }
            StmtKind::Menu { choices, .. } => {
                for c in choices {
                    collect_edges(from, &c.body, Some(&c.text), out);
                }
            }
            _ => {}
        }
    }
}

/// Whether control can never run off the end of `stmts`. Nested labels are separate nodes, so
/// they are skipped.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, source: &str) -> SourceFile {
        SourceFile { path: path.into(), source: source.into() }
    }

    fn edge(from: &str, to: &str, kind: &str, text: Option<&str>) -> GraphEdge {
        GraphEdge { from: from.into(), to: to.into(), kind: kind.into(), text: text.map(Into::into) }
    }

    #[test]
    fn graph_has_jump_call_choice_and_fallthrough_edges() {
        let src = "\
label start:
    \"Hi.\"
    menu:
        \"Go\":
            jump about
        \"Ask\":
            call meet
            jump about
label about:
    \"About.\"
label meet:
    return
";
        let g = story_graph(&[file("script/a.kai", src)]);
        let labels: Vec<_> = g.nodes.iter().map(|n| (n.label.as_str(), n.line)).collect();
        assert_eq!(labels, [("start", 1), ("about", 9), ("meet", 11)]);
        assert_eq!(
            g.edges,
            [
                edge("start", "about", "choice", Some("Go")),
                edge("start", "meet", "call", Some("Ask")),
                edge("start", "about", "choice", Some("Ask")),
                edge("about", "meet", "fallthrough", None),
            ]
        );
    }

    #[test]
    fn menu_without_exit_falls_through() {
        let src = "label a:\n    menu:\n        \"x\":\n            jump b\n        \"y\":\n            pass\nlabel b:\n    return\n";
        let g = story_graph(&[file("a.kai", src)]);
        assert!(g.edges.contains(&edge("a", "b", "fallthrough", None)));
        assert!(g.edges.contains(&edge("a", "b", "choice", Some("x"))));
    }

    #[test]
    fn if_with_else_that_always_jumps_does_not_fall_through() {
        let src = "label a:\n    if x:\n        jump c\n    else:\n        return\nlabel b:\n    jump c\nlabel c:\n    return\n";
        let g = story_graph(&[file("a.kai", src)]);
        assert_eq!(g.edges, [edge("a", "c", "jump", None), edge("b", "c", "jump", None)]);
    }

    #[test]
    fn files_are_ordered_by_path_and_fall_through_across_files() {
        let g = story_graph(&[
            file("script/b.kai", "label second:\n    return\n"),
            file("script/a.kai", "label first:\n    \"hi\"\n"),
        ]);
        assert_eq!(g.nodes[0].path, "script/a.kai");
        assert_eq!(g.edges, [edge("first", "second", "fallthrough", None)]);
    }

    #[test]
    fn unknown_targets_and_duplicates_are_dropped() {
        let src = "label a:\n    if x:\n        jump b\n    jump b\n    jump nowhere\nlabel b:\n    return\n";
        let g = story_graph(&[file("a.kai", src)]);
        assert_eq!(g.edges, [edge("a", "b", "jump", None)]);
    }

    #[test]
    fn graph_includes_title_flow_labels() {
        let src = "label splashscreen:\n    return\nlabel main_menu:\n    jump start\nlabel start:\n    return\n";
        let g = story_graph(&[file("script/a.kai", src)]);
        let labels: Vec<_> = g.nodes.iter().map(|node| node.label.as_str()).collect();
        assert_eq!(labels, ["splashscreen", "main_menu", "start"]);
        assert_eq!(g.edges, [edge("main_menu", "start", "jump", None)]);
    }

    #[test]
    fn graph_of_example_project() {
        let src = include_str!("../../../examples/hello-kairos/script/script.kai");
        let g = story_graph(&[file("script/script.kai", src)]);
        let names: Vec<_> = g.nodes.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(names, ["splashscreen", "start", "about", "meet_kai", "goodbye"]);
        assert!(g.edges.contains(&edge("start", "about", "choice", Some("Tell me about the engine"))));
        assert!(g.edges.contains(&edge("start", "meet_kai", "call", Some("Who else is here?"))));
        assert!(g.edges.contains(&edge("about", "goodbye", "jump", None)));
        assert!(!g.edges.iter().any(|e| e.kind == "fallthrough"));
    }

    #[test]
    fn check_project_reports_paths_and_lines() {
        let r = check_project(&[file("script/a.kai", "label a:\n    jump nowhere\n    e \"hi\"\n")]);
        let got: Vec<_> = r.diagnostics.iter().map(|d| (d.path.as_str(), d.line, d.message.as_str())).collect();
        assert_eq!(got, [("script/a.kai", 2, "unknown label `nowhere`"), ("script/a.kai", 3, "unknown character `e`")]);
    }

    #[test]
    fn outline_lists_labels_and_characters() {
        let o = outline("character e \"Eileen\" color=\"#fff\"\nlabel a:\n    return\n");
        assert_eq!(o.labels, [OutlineLabel { name: "a".into(), line: 2 }]);
        assert_eq!(o.characters[0].color.as_deref(), Some("#fff"));
        assert_eq!(parse("label a\n").diagnostics.len(), 1);
    }
}
