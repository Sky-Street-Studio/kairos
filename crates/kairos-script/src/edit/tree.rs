use crate::ast::{Stmt, StmtKind};

use super::text::Lines;

pub(crate) struct Node<'a> {
    pub stmt: &'a Stmt,
    pub line: usize,
    pub end: usize,
    pub indent: usize,
    pub bodies: Vec<Body<'a>>,
}

pub(crate) struct Body<'a> {
    pub header: usize,
    pub nodes: Vec<Node<'a>>,
    // Read by the scene operations in Task 5; drop this allow once they land.
    #[allow(dead_code)]
    pub end: usize,
}

pub(crate) fn build<'a>(stmts: &'a [Stmt], lines: &Lines) -> Vec<Node<'a>> {
    nodes(stmts, lines.len(), lines)
}

fn nodes<'a>(stmts: &'a [Stmt], limit: usize, lines: &Lines) -> Vec<Node<'a>> {
    stmts
        .iter()
        .enumerate()
        .map(|(i, stmt)| {
            let bound = stmts.get(i + 1).map_or(limit, |next| next.line - 1);
            let end = lines.last_code(stmt.line, bound);
            let indent = lines.indent(stmt.line);
            let bodies = bodies(stmt, end, indent, lines);
            Node { stmt, line: stmt.line, end, indent, bodies }
        })
        .collect()
}

fn bodies<'a>(stmt: &'a Stmt, end: usize, indent: usize, lines: &Lines) -> Vec<Body<'a>> {
    let body = |header: usize, stmts: &'a [Stmt], bound: usize| {
        let end = lines.last_code(header, bound);
        Body { header, nodes: nodes(stmts, end, lines), end }
    };
    match &stmt.kind {
        StmtKind::Label { body: stmts, .. } => vec![body(stmt.line, stmts, end)],
        StmtKind::Menu { choices, .. } => choices
            .iter()
            .enumerate()
            .map(|(i, c)| body(c.line, &c.body, choices.get(i + 1).map_or(end, |n| n.line - 1)))
            .collect(),
        StmtKind::If(branches) => {
            let headers: Vec<usize> =
                (stmt.line..=end).filter(|&n| lines.is_code(n) && lines.indent(n) == indent).collect();
            branches
                .iter()
                .zip(&headers)
                .enumerate()
                .map(|(i, (b, &h))| body(h, &b.body, headers.get(i + 1).map_or(end, |n| n - 1)))
                .collect()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn find<'a, 'b>(nodes: &'b [Node<'a>], line: usize) -> Option<&'b Node<'a>> {
    nodes.iter().find_map(|n| if n.line == line { Some(n) } else { n.bodies.iter().find_map(|b| find(&b.nodes, line)) })
}

pub(crate) fn find_body<'a, 'b>(nodes: &'b [Node<'a>], header: usize) -> Option<&'b Body<'a>> {
    nodes.iter().find_map(|n| {
        n.bodies
            .iter()
            .find(|b| b.header == header)
            .or_else(|| n.bodies.iter().find_map(|b| find_body(&b.nodes, header)))
    })
}

pub(crate) fn parent_body<'a, 'b>(nodes: &'b [Node<'a>], line: usize) -> Option<&'b Body<'a>> {
    nodes.iter().find_map(|n| {
        n.bodies
            .iter()
            .find_map(|b| if b.nodes.iter().any(|c| c.line == line) { Some(b) } else { parent_body(&b.nodes, line) })
    })
}

pub(crate) fn label<'a, 'b>(nodes: &'b [Node<'a>], name: &str) -> Option<&'b Node<'a>> {
    nodes.iter().find(|n| matches!(&n.stmt.kind, StmtKind::Label { name: l, .. } if l == name))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "\
label start:
    \"One\"
    # comment
    if x:
        \"A\"
    elif y:
        \"B\"
    else:
        \"C\"

    menu:
        \"Go\":
            jump other
        \"Stay\" if z:
            \"Stayed\"
label other:
    return
";

    #[test]
    fn extents_cover_children_and_branch_lines() {
        let (stmts, diags) = crate::parse(SRC);
        assert!(diags.is_empty(), "{diags:?}");
        let lines = Lines::parse(SRC);
        let nodes = build(&stmts, &lines);
        assert_eq!((nodes[0].line, nodes[0].end), (1, 15));
        assert_eq!((nodes[1].line, nodes[1].end), (16, 17));
        let body = &nodes[0].bodies[0];
        let starts: Vec<_> = body.nodes.iter().map(|n| (n.line, n.end)).collect();
        assert_eq!(starts, [(2, 2), (4, 9), (11, 15)]);
        let if_node = &body.nodes[1];
        assert_eq!(if_node.bodies.iter().map(|b| b.header).collect::<Vec<_>>(), [4, 6, 8]);
        let menu = &body.nodes[2];
        assert_eq!(menu.bodies.iter().map(|b| (b.header, b.end)).collect::<Vec<_>>(), [(12, 13), (14, 15)]);
        assert_eq!(find(&nodes, 15).map(|n| n.line), Some(15));
        assert_eq!(find_body(&nodes, 14).map(|b| b.nodes.len()), Some(1));
        assert_eq!(parent_body(&nodes, 5).map(|b| b.header), Some(4));
        assert_eq!(label(&nodes, "other").map(|n| n.line), Some(16));
    }
}
