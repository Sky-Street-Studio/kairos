# Visual Story Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let people without a coding background build a visual novel from the story graph and a block editor, while the script files stay the single source of truth and the code view stays one click away.

**Architecture:** A new `kairos_script::edit` module turns scripts into blocks and applies edit operations that change only the affected lines. The napi addon exposes it as JSON in, JSON out. The editor store sends every visual change through one `edit(ops)` action that updates buffers and pushes onto a project-wide undo history shared with the code view. The graph and the new scene tab are UIs over those ops. The preview gains a "seek" that plays a scene up to a block.

**Tech Stack:** Rust (kairos-script, kairos-player, napi-rs 3, serde_json), TypeScript + React 19 + zustand + @xyflow/react + Monaco in Electron, vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-13-visual-story-editor-design.md`

## Global Constraints

- Use `bun`/`bunx` for JS tooling; never `npm`, `npx`, `yarn` or `pnpm`.
- Rust commands run with `export PATH=/opt/homebrew/opt/rustup/bin:$PATH` first.
- Commit messages follow the repo style: one plain sentence, no prefix, no AI trailer (e.g. `Add the script edit engine`).
- Comments only explain why; no section labels, narration or restated code.
- Before each commit: `cargo fmt`, `cargo clippy --workspace --all-targets` clean, `bun run lint`, `bun run fmt:check`, `bun run typecheck`.
- Generated indentation is 4 spaces per level unless the surrounding block uses a different width, in which case it matches the sibling statements.
- Edits preserve the file's line ending (`\n` or `\r\n`) and whether it ends with a newline.
- Field edits in the block editor are debounced to 300 ms.
- The undo history keeps 200 entries and resets when a project opens.
- Graph positions persist in `<project>/.kairos/editor.json` as `{ "graph": { "<label>": { "x": number, "y": number } } }`.

## Deviations from the spec, recorded in Task 1

- Blocks are identified by their 1-based first line in the source they were read from, not the stable statement ID. Every edit is applied to the exact source the blocks came from, so a line identifies a statement; the stable ID lives in `kairos-vm`, which `kairos-script` cannot depend on.
- Staleness is detected in the store: after `applyEdits` returns, the store compares the buffers it sent with the current buffers. If they differ, the edit is dropped (not retried, because line-based ops computed against old text are unsafe on new text) and the scene tab reloads its blocks.
- Code-view typing joins the shared history as entries coalesced per file after 1 s without typing, instead of at Monaco's internal undo boundaries.

## Review Focus

1. **CRLF files and files without a trailing newline.** Expect edits to keep `\r\n` and the missing final newline. Pinned in Task 1 (`lines_round_trip_crlf_and_missing_final_newline`) and Task 4 (`insert_keeps_crlf`).
2. **Bodies indented with 2 spaces instead of 4.** Expect inserted blocks to match their siblings' indentation. Pinned in Task 4 (`insert_matches_sibling_indent`).
3. **Dialogue containing quotes, backslashes, newlines and non-ASCII text.** Expect it to round-trip through block → script → block unchanged. Pinned in Task 3 (`render_round_trips_tricky_text`).
4. **Editing a file that already has an unrelated parse error.** Expect the edit to succeed, and only edits that add new errors to be rejected. Pinned in Task 4 (`edits_allowed_when_file_already_has_errors`).
5. **Renaming a scene whose name prefixes another (`day` vs `day_two`).** Expect only exact references to change. Pinned in Task 5 (`rename_changes_only_exact_references`).

## File Structure

Rust:

- `crates/kairos-script/src/edit/mod.rs`: public API: `SourceFile`, `EditError`, `Applied`, `Focus`, re-exports.
- `crates/kairos-script/src/edit/text.rs`: `Lines` (split and join preserving line endings), indentation and code-line helpers, `quote`, `label_from_name`.
- `crates/kairos-script/src/edit/tree.rs`: `Node`/`Body` index of statements with line extents, built from the parser output.
- `crates/kairos-script/src/edit/blocks.rs`: `Block`, `BlockKind`, `scene_blocks`, `render`.
- `crates/kairos-script/src/edit/ops.rs`: `Op`, `Place`, `apply` for statement ops.
- `crates/kairos-script/src/edit/scenes.rs`: scene and link ops, `references`.
- `crates/kairos-script/tests/edit.rs`: integration and property tests.
- `crates/kairos-napi/src/edit.rs`: JSON bridge.
- `crates/kairos-player/src/game.rs`, `player.rs`, `view.rs`, `web.rs`: `seek_to` and `sprite_at`.

Editor:

- `apps/editor/src/shared/edit-types.ts`: TS mirror of the Rust JSON types.
- `apps/editor/src/main/edit.ts`: IPC handlers for edits, the editor layout file and the start scene.
- `apps/editor/src/renderer/src/store/history-slice.ts`: undo/redo.
- `apps/editor/src/renderer/src/store/edit-slice.ts`: `edit(ops)`, graph positions.
- `apps/editor/src/renderer/src/features/graph/*`: graph tools.
- `apps/editor/src/renderer/src/features/scene/*`: block editor.
- Deleted: `apps/editor/src/shared/edits.ts` and `edits.test.ts`, the `ProjectEdit` type and `applyEdit`.

---

### Task 1: Text helpers and scene names

**Files:**
- Create: `crates/kairos-script/src/edit/mod.rs`, `crates/kairos-script/src/edit/text.rs`
- Modify: `crates/kairos-script/src/lib.rs`, `docs/superpowers/specs/2026-10-13-visual-story-editor-design.md`
- Test: `crates/kairos-script/src/edit/text.rs` (unit tests)

**Interfaces:**
- Produces:
  - `pub(crate) struct Lines { pub lines: Vec<String>, eol: &'static str, trailing: bool }`
    - `Lines::parse(&str) -> Lines`
    - `Lines::join(&self) -> String`
    - `Lines::text(&self, n: usize) -> &str` (1-based)
    - `Lines::indent(&self, n) -> usize`
    - `Lines::is_code(&self, n) -> bool`
    - `Lines::last_code(&self, from: usize, to: usize) -> usize`
    - `Lines::splice(&mut self, first: usize, last: usize, with: Vec<String>)`, which replaces the inclusive 1-based range; `last = first - 1` inserts before `first`
  - `pub(crate) fn quote(&str) -> String`
  - `pub fn label_from_name(name: &str, taken: &[String]) -> String`

- [ ] **Step 1: Update the spec with the three deviations**

In `docs/superpowers/specs/2026-10-13-visual-story-editor-design.md`:

- Replace the `id` bullet under `scene_blocks` with: "`line` is the block's 1-based first line in the source it was read from. Every edit is applied to that exact source, so a line identifies a statement."
- Replace the `base` bullet under `apply` with: "The store detects staleness: if the buffers changed while an edit was in flight, the edit is dropped and the scene tab reloads its blocks."
- In "Errors and undo", replace "Code-view typing pushes entries at Monaco's undo boundaries" with "Code-view typing pushes one entry per file after 1 s without typing".
- Replace the `Stale` bullet under "Editor store" with the staleness sentence above.

- [ ] **Step 2: Write the failing tests**

`crates/kairos-script/src/edit/text.rs`:

```rust
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
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p kairos-script --lib edit::text`
Expected: compile errors (`Lines`, `quote`, `label_from_name` not found).

- [ ] **Step 4: Implement**

`crates/kairos-script/src/edit/mod.rs`:

```rust
//! Structured edits to `.kai` source: blocks for the visual editor and operations that change only
//! the lines they touch.

mod text;

pub use text::label_from_name;
```

`crates/kairos-script/src/lib.rs`: add `pub mod edit;` after `pub mod ast;`.

`crates/kairos-script/src/edit/text.rs` (above the tests):

```rust
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
        t.len() - t.trim_start().len()
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

/// Turns a name a person typed into a label: lowercase letters, digits and `_`, unique among `taken`.
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
    if base.is_empty() {
        base = "scene".into();
    }
    if base.starts_with(|c: char| c.is_ascii_digit()) {
        base.insert(0, '_');
    }
    if !taken.contains(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}_{n}")).find(|c| !taken.contains(c)).expect("some suffix is free")
}
```

`"Café — night!"` lowercases to `café — night!`. `é` is not ASCII alphanumeric, so it becomes `_` and the result is `caf_night`, which matches the test.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p kairos-script --lib edit::text`
Expected: 5 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/kairos-script/src docs/superpowers/specs
git commit -m "Add line and scene-name helpers for script edits"
```

---

### Task 2: Statement tree with line extents

**Files:**
- Create: `crates/kairos-script/src/edit/tree.rs`
- Modify: `crates/kairos-script/src/edit/mod.rs` (add `mod tree;`)
- Test: unit tests in `tree.rs`

**Interfaces:**
- Consumes: `Lines` from Task 1.
- Produces:
  - `pub(crate) struct Node<'a> { pub stmt: &'a Stmt, pub line: usize, pub end: usize, pub indent: usize, pub bodies: Vec<Body<'a>> }`
  - `pub(crate) struct Body<'a> { pub header: usize, pub nodes: Vec<Node<'a>>, pub end: usize }`
  - `pub(crate) fn build<'a>(stmts: &'a [Stmt], lines: &Lines) -> Vec<Node<'a>>`
  - `pub(crate) fn find<'a, 'b>(nodes: &'b [Node<'a>], line: usize) -> Option<&'b Node<'a>>` (the node starting at `line`, searched recursively)
  - `pub(crate) fn find_body<'a, 'b>(nodes: &'b [Node<'a>], header: usize) -> Option<&'b Body<'a>>`
  - `pub(crate) fn label<'a, 'b>(nodes: &'b [Node<'a>], name: &str) -> Option<&'b Node<'a>>`
  - `pub(crate) fn parent_body<'a, 'b>(nodes: &'b [Node<'a>], line: usize) -> Option<&'b Body<'a>>` (the body directly containing the node at `line`)

Bodies:
- A `label` has one body whose `header` is the label line.
- A `menu` has one body per choice whose `header` is the choice line. A menu with a caption still lists only its choices.
- An `if` has one body per branch whose `header` is the `if`, `elif` or `else` line.
- `end` is always the last code line, so trailing comments and blank lines belong to whatever follows.

- [ ] **Step 1: Write the failing tests**

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kairos-script --lib edit::tree`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
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
    nodes.iter().find_map(|n| {
        if n.line == line {
            Some(n)
        } else {
            n.bodies.iter().find_map(|b| find(&b.nodes, line))
        }
    })
}

pub(crate) fn find_body<'a, 'b>(nodes: &'b [Node<'a>], header: usize) -> Option<&'b Body<'a>> {
    nodes.iter().find_map(|n| {
        n.bodies.iter().find(|b| b.header == header).or_else(|| n.bodies.iter().find_map(|b| find_body(&b.nodes, header)))
    })
}

pub(crate) fn parent_body<'a, 'b>(nodes: &'b [Node<'a>], line: usize) -> Option<&'b Body<'a>> {
    nodes.iter().find_map(|n| {
        n.bodies.iter().find_map(|b| {
            if b.nodes.iter().any(|c| c.line == line) { Some(b) } else { parent_body(&b.nodes, line) }
        })
    })
}

pub(crate) fn label<'a, 'b>(nodes: &'b [Node<'a>], name: &str) -> Option<&'b Node<'a>> {
    nodes.iter().find(|n| matches!(&n.stmt.kind, StmtKind::Label { name: l, .. } if l == name))
}
```

The `elif`/`else` headers are found by indentation because `Branch` carries no line number. The parser only accepts them at the `if`'s indent, so the scan is exact.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kairos-script --lib edit::tree`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/kairos-script/src/edit
git commit -m "Index script statements by their line extents"
```

---

### Task 3: Blocks and rendering

**Files:**
- Create: `crates/kairos-script/src/edit/blocks.rs`
- Modify: `crates/kairos-script/src/edit/mod.rs`, `crates/kairos-script/Cargo.toml` (add `serde_json = "1"` under `[dev-dependencies]`)
- Test: unit tests in `blocks.rs`

**Interfaces:**
- Consumes: `Lines`, `quote` (Task 1); `build`, `label`, `Node` (Task 2).
- Produces (serde: `#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]`):

```rust
pub struct Block { pub line: usize, pub end_line: usize, #[serde(flatten)] pub kind: BlockKind }
pub enum BlockKind {
    Dialogue { character: Option<String>, text: String },
    Background { image: Vec<String>, transition: Option<String> },
    Show { image: Vec<String>, position: Option<String>, transition: Option<String> },
    Hide { character: String, transition: Option<String> },
    Audio { channel: String, asset: String, looped: bool, fade_in: Option<f64> },
    StopAudio { channel: String, fade_out: Option<f64> },
    Pause { seconds: Option<f64> },
    Choice { options: Vec<ChoiceOption> },
    GoTo { scene: String },
    Call { scene: String },
    SetVariable { name: String, op: SetOp, value: String },
    If { branches: Vec<IfBranch> },
    End,
    Code { text: String },
}
pub struct ChoiceOption { pub line: usize, pub text: String, pub condition: Option<String>, pub blocks: Vec<Block> }
pub struct IfBranch { pub line: usize, pub condition: Option<String>, pub blocks: Vec<Block> }
pub enum SetOp { Set, Add, Subtract, Toggle }   // serde rename_all = "camelCase"
pub struct Scene { pub label: String, pub line: usize, pub blocks: Vec<Block> }
pub fn scene_blocks(source: &str, label: &str) -> Option<Scene>
pub(crate) fn render(kind: &BlockKind, indent: usize, step: usize) -> Vec<String>
```

`render` writes empty bodies as `pass`. For `Code`, `text` holds the statement's lines with the statement's own indentation removed, and `render` re-indents them.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<BlockKind> {
        scene_blocks(src, "start").unwrap().blocks.into_iter().map(|b| b.kind).collect()
    }

    #[test]
    fn friendly_statements_become_blocks() {
        let src = "label start:\n    scene bg room with fade\n    show e happy at left\n    e \"Hi\"\n    \"Narration\"\n    play music \"theme.ogg\" loop fadein 1.5\n    stop music fadeout 2\n    pause 1\n    hide e\n    $ n += 1\n    $ seen = not seen\n    jump other\n    call side\n    return\n";
        assert_eq!(
            kinds(src),
            [
                BlockKind::Background { image: vec!["bg".into(), "room".into()], transition: Some("fade".into()) },
                BlockKind::Show { image: vec!["e".into(), "happy".into()], position: Some("left".into()), transition: None },
                BlockKind::Dialogue { character: Some("e".into()), text: "Hi".into() },
                BlockKind::Dialogue { character: None, text: "Narration".into() },
                BlockKind::Audio { channel: "music".into(), asset: "theme.ogg".into(), looped: true, fade_in: Some(1.5) },
                BlockKind::StopAudio { channel: "music".into(), fade_out: Some(2.0) },
                BlockKind::Pause { seconds: Some(1.0) },
                BlockKind::Hide { character: "e".into(), transition: None },
                BlockKind::SetVariable { name: "n".into(), op: SetOp::Add, value: "1".into() },
                BlockKind::SetVariable { name: "seen".into(), op: SetOp::Toggle, value: String::new() },
                BlockKind::GoTo { scene: "other".into() },
                BlockKind::Call { scene: "side".into() },
                BlockKind::End,
            ]
        );
    }

    #[test]
    fn choices_and_ifs_nest_with_conditions_as_text() {
        let src = "label start:\n    menu:\n        \"Go\" if key and n > 1:\n            jump out\n        \"Stay\":\n            \"Ok\"\n    if n > 2:\n        \"Big\"\n    else:\n        pass\n";
        let blocks = scene_blocks(src, "start").unwrap().blocks;
        let BlockKind::Choice { options } = &blocks[0].kind else { panic!() };
        assert_eq!((options[0].line, options[0].text.as_str(), options[0].condition.as_deref()), (3, "Go", Some("key and n > 1")));
        assert_eq!(options[1].blocks[0].kind, BlockKind::Dialogue { character: None, text: "Ok".into() });
        let BlockKind::If { branches } = &blocks[1].kind else { panic!() };
        assert_eq!(branches.iter().map(|b| (b.line, b.condition.clone())).collect::<Vec<_>>(), [(7, Some("n > 2".into())), (9, None)]);
        assert_eq!(branches[1].blocks[0].kind, BlockKind::Code { text: "pass".into() });
    }

    #[test]
    fn everything_else_is_a_code_block_with_its_exact_text() {
        let src = "label start:\n    $ xs.append(1)\n    $ d[\"k\"] = 2\n    \"Eileen\" \"Named speaker\"\n    queue music \"a\"\n    menu:\n        \"Caption\"\n        \"Only\":\n            pass\n    with dissolve\n";
        let texts: Vec<_> = kinds(src)
            .into_iter()
            .map(|k| match k {
                BlockKind::Code { text } => text,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(texts[0], "$ xs.append(1)");
        assert_eq!(texts[4], "menu:\n    \"Caption\"\n    \"Only\":\n        pass");
        assert_eq!(texts.len(), 6);
    }

    #[test]
    fn render_writes_canonical_script() {
        let r = |k: BlockKind| render(&k, 4, 4).join("\n");
        assert_eq!(r(BlockKind::Dialogue { character: Some("e".into()), text: "Hi".into() }), "    e \"Hi\"");
        assert_eq!(r(BlockKind::SetVariable { name: "seen".into(), op: SetOp::Toggle, value: String::new() }), "    $ seen = not seen");
        assert_eq!(
            r(BlockKind::Choice { options: vec![ChoiceOption { line: 0, text: "Go".into(), condition: Some("key".into()), blocks: vec![] }] }),
            "    menu:\n        \"Go\" if key:\n            pass"
        );
        assert_eq!(r(BlockKind::Code { text: "if a:\n    pass".into() }), "    if a:\n        pass");
        assert_eq!(r(BlockKind::Audio { channel: "music".into(), asset: "t.ogg".into(), looped: true, fade_in: None }), "    play music \"t.ogg\" loop");
    }

    #[test]
    fn render_round_trips_tricky_text() {
        for text in ["She said \"no\".", "back\\slash", "two\nlines", "日本語 ✨", ""] {
            let kind = BlockKind::Dialogue { character: Some("e".into()), text: text.into() };
            let src = format!("label start:\n{}\n", render(&kind, 4, 4).join("\n"));
            assert_eq!(kinds(&src), [kind]);
        }
    }

    #[test]
    fn blocks_serialize_with_a_kind_tag() {
        let block = Block { line: 2, end_line: 2, kind: BlockKind::GoTo { scene: "x".into() } };
        assert_eq!(serde_json::to_string(&block).unwrap(), r#"{"line":2,"endLine":2,"kind":"goTo","scene":"x"}"#);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kairos-script --lib edit::blocks`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
use serde::{Deserialize, Serialize};

use crate::ast::{AssignOp, Expr, Speaker, StmtKind, UnOp};
use crate::lexer::strip_comment;

use super::text::{Lines, quote};
use super::tree::{self, Node};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub line: usize,
    pub end_line: usize,
    #[serde(flatten)]
    pub kind: BlockKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BlockKind {
    Dialogue { character: Option<String>, text: String },
    Background { image: Vec<String>, transition: Option<String> },
    Show { image: Vec<String>, position: Option<String>, transition: Option<String> },
    Hide { character: String, transition: Option<String> },
    Audio { channel: String, asset: String, looped: bool, fade_in: Option<f64> },
    StopAudio { channel: String, fade_out: Option<f64> },
    Pause { seconds: Option<f64> },
    Choice { options: Vec<ChoiceOption> },
    GoTo { scene: String },
    Call { scene: String },
    SetVariable { name: String, op: SetOp, value: String },
    If { branches: Vec<IfBranch> },
    End,
    Code { text: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceOption {
    pub line: usize,
    pub text: String,
    pub condition: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IfBranch {
    pub line: usize,
    pub condition: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetOp {
    Set,
    Add,
    Subtract,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub label: String,
    pub line: usize,
    pub blocks: Vec<Block>,
}

pub fn scene_blocks(source: &str, label: &str) -> Option<Scene> {
    let (stmts, _) = crate::parse(source);
    let lines = Lines::parse(source);
    let nodes = tree::build(&stmts, &lines);
    let node = tree::label(&nodes, label)?;
    Some(Scene { label: label.into(), line: node.line, blocks: blocks(&node.bodies[0].nodes, &lines) })
}

pub(crate) fn blocks(nodes: &[Node], lines: &Lines) -> Vec<Block> {
    nodes.iter().map(|n| Block { line: n.line, end_line: n.end, kind: kind(n, lines) }).collect()
}

fn code(n: &Node, lines: &Lines) -> BlockKind {
    let text = (n.line..=n.end)
        .map(|l| {
            let t = lines.text(l);
            if t.len() >= n.indent && t[..n.indent].trim().is_empty() { &t[n.indent..] } else { t.trim_start() }
        })
        .collect::<Vec<_>>()
        .join("\n");
    BlockKind::Code { text: text.trim_end().into() }
}

fn header(lines: &Lines, line: usize) -> &str {
    strip_comment(lines.text(line)).trim()
}

/// The text after a leading string literal, e.g. ` if key:` in `"Go" if key:`.
fn after_string(s: &str) -> &str {
    let quote = s.chars().next().unwrap_or('"');
    let mut escaped = false;
    for (i, c) in s.char_indices().skip(1) {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            c if c == quote => return &s[i + 1..],
            _ => {}
        }
    }
    ""
}

fn condition_of_choice(lines: &Lines, line: usize) -> Option<String> {
    let rest = after_string(header(lines, line)).trim().trim_end_matches(':').trim();
    rest.strip_prefix("if ").map(|c| c.trim().to_string())
}

fn condition_of_branch(lines: &Lines, line: usize) -> Option<String> {
    let h = header(lines, line).trim_end_matches(':').trim();
    h.strip_prefix("if ").or_else(|| h.strip_prefix("elif ")).map(|c| c.trim().to_string())
}

fn assigned_value(lines: &Lines, line: usize) -> String {
    let h = header(lines, line);
    let ops = ["+=", "-=", "="];
    let at = ops.iter().filter_map(|op| h.find(op).map(|i| i + op.len())).min().unwrap_or(h.len());
    h[at..].trim().to_string()
}

fn kind(n: &Node, lines: &Lines) -> BlockKind {
    match &n.stmt.kind {
        StmtKind::Say { speaker: None, text } => BlockKind::Dialogue { character: None, text: text.clone() },
        StmtKind::Say { speaker: Some(Speaker::Character(id)), text } => {
            BlockKind::Dialogue { character: Some(id.clone()), text: text.clone() }
        }
        StmtKind::Scene { image: Some(image), with } => BlockKind::Background { image: image.clone(), transition: with.clone() },
        StmtKind::Show { image, at, with } => {
            BlockKind::Show { image: image.clone(), position: at.clone(), transition: with.clone() }
        }
        StmtKind::Hide { tag, with } => BlockKind::Hide { character: tag.clone(), transition: with.clone() },
        StmtKind::Play { channel, asset, looped, queue: false, fadein } if ["music", "sound", "voice"].contains(&channel.as_str()) => {
            BlockKind::Audio { channel: channel.clone(), asset: asset.clone(), looped: *looped, fade_in: *fadein }
        }
        StmtKind::Stop { channel, fadeout } => BlockKind::StopAudio { channel: channel.clone(), fade_out: *fadeout },
        StmtKind::Pause(seconds) => BlockKind::Pause { seconds: *seconds },
        StmtKind::Menu { caption: None, choices } => BlockKind::Choice {
            options: choices
                .iter()
                .zip(&n.bodies)
                .map(|(c, b)| ChoiceOption {
                    line: c.line,
                    text: c.text.clone(),
                    condition: condition_of_choice(lines, c.line),
                    blocks: blocks(&b.nodes, lines),
                })
                .collect(),
        },
        StmtKind::Jump(scene) => BlockKind::GoTo { scene: scene.clone() },
        StmtKind::Call(scene) => BlockKind::Call { scene: scene.clone() },
        StmtKind::Return => BlockKind::End,
        StmtKind::Assign { name, index, op, value } if index.is_empty() => {
            let toggle = matches!(value, Expr::Unary(UnOp::Not, v) if matches!(&**v, Expr::Var(x) if x == name));
            let op = match op {
                AssignOp::Set if toggle => SetOp::Toggle,
                AssignOp::Set => SetOp::Set,
                AssignOp::Add => SetOp::Add,
                AssignOp::Sub => SetOp::Subtract,
                _ => return code(n, lines),
            };
            let value = if op == SetOp::Toggle { String::new() } else { assigned_value(lines, n.line) };
            BlockKind::SetVariable { name: name.clone(), op, value }
        }
        StmtKind::If(_) => BlockKind::If {
            branches: n
                .bodies
                .iter()
                .map(|b| IfBranch { line: b.header, condition: condition_of_branch(lines, b.header), blocks: blocks(&b.nodes, lines) })
                .collect(),
        },
        _ => code(n, lines),
    }
}

fn number(n: f64) -> String {
    if n.fract() == 0.0 { format!("{}", n as i64) } else { n.to_string() }
}

pub(crate) fn render(kind: &BlockKind, indent: usize, step: usize) -> Vec<String> {
    let pad = " ".repeat(indent);
    let body = |blocks: &[Block], out: &mut Vec<String>| {
        if blocks.is_empty() {
            out.push(format!("{}pass", " ".repeat(indent + step * 2)));
        }
        for b in blocks {
            out.extend(render(&b.kind, indent + step * 2, step));
        }
    };
    let with = |t: &Option<String>| t.as_ref().map(|t| format!(" with {t}")).unwrap_or_default();
    let one = |s: String| vec![format!("{pad}{s}")];
    match kind {
        BlockKind::Dialogue { character: Some(c), text } => one(format!("{c} {}", quote(text))),
        BlockKind::Dialogue { character: None, text } => one(quote(text)),
        BlockKind::Background { image, transition } => one(format!("scene {}{}", image.join(" "), with(transition))),
        BlockKind::Show { image, position, transition } => one(format!(
            "show {}{}{}",
            image.join(" "),
            position.as_ref().map(|p| format!(" at {p}")).unwrap_or_default(),
            with(transition)
        )),
        BlockKind::Hide { character, transition } => one(format!("hide {character}{}", with(transition))),
        BlockKind::Audio { channel, asset, looped, fade_in } => one(format!(
            "play {channel} {}{}{}",
            quote(asset),
            if *looped { " loop" } else { "" },
            fade_in.map(|f| format!(" fadein {}", number(f))).unwrap_or_default()
        )),
        BlockKind::StopAudio { channel, fade_out } => {
            one(format!("stop {channel}{}", fade_out.map(|f| format!(" fadeout {}", number(f))).unwrap_or_default()))
        }
        BlockKind::Pause { seconds } => one(seconds.map(|s| format!("pause {}", number(s))).unwrap_or_else(|| "pause".into())),
        BlockKind::Choice { options } => {
            let mut out = one("menu:".into());
            for o in options {
                let cond = o.condition.as_ref().map(|c| format!(" if {c}")).unwrap_or_default();
                out.push(format!("{}{}{cond}:", " ".repeat(indent + step), quote(&o.text)));
                body(&o.blocks, &mut out);
            }
            out
        }
        BlockKind::GoTo { scene } => one(format!("jump {scene}")),
        BlockKind::Call { scene } => one(format!("call {scene}")),
        BlockKind::SetVariable { name, op, value } => one(match op {
            SetOp::Set => format!("$ {name} = {value}"),
            SetOp::Add => format!("$ {name} += {value}"),
            SetOp::Subtract => format!("$ {name} -= {value}"),
            SetOp::Toggle => format!("$ {name} = not {name}"),
        }),
        BlockKind::If { branches } => {
            let mut out = Vec::new();
            for (i, b) in branches.iter().enumerate() {
                let head = match (&b.condition, i) {
                    (Some(c), 0) => format!("if {c}:"),
                    (Some(c), _) => format!("elif {c}:"),
                    (None, _) => "else:".to_string(),
                };
                out.push(format!("{pad}{head}"));
                if b.blocks.is_empty() {
                    out.push(format!("{}pass", " ".repeat(indent + step)));
                }
                for block in &b.blocks {
                    out.extend(render(&block.kind, indent + step, step));
                }
            }
            out
        }
        BlockKind::End => one("return".into()),
        BlockKind::Code { text } => text.lines().map(|l| if l.is_empty() { String::new() } else { format!("{pad}{l}") }).collect(),
    }
}
```

Choice bodies sit two levels deep (`menu:`, then the option, then the body), so `body` indents them by `step * 2`. `If` bodies sit one level deep.

`mod.rs`: add `mod blocks; mod tree;` and `pub use blocks::{Block, BlockKind, ChoiceOption, IfBranch, Scene, SetOp, scene_blocks};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kairos-script --lib edit::blocks`
Expected: 6 passed. If `everything_else_is_a_code_block_with_its_exact_text` reports a different count, print `texts` and check that `with dissolve` and the captioned menu each came through as one Code block.

- [ ] **Step 5: Commit**

```bash
git add crates/kairos-script
git commit -m "Read scenes as blocks and render blocks back to script"
```

---

### Task 4: Statement operations

**Files:**
- Create: `crates/kairos-script/src/edit/ops.rs`
- Modify: `crates/kairos-script/src/edit/mod.rs`
- Test: `crates/kairos-script/tests/edit.rs` (new), and add `proptest.workspace = true` to `[dev-dependencies]` in `crates/kairos-script/Cargo.toml`

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces (serde `tag = "op"`, camelCase):

```rust
pub struct SourceFile { pub path: String, pub source: String }   // Serialize + Deserialize
pub enum Place { After { line: usize }, Start { header: usize } } // tag = "at"
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
    // scene ops are added in Task 5
}
pub struct Focus { pub path: String, pub line: usize }
pub struct Applied { pub files: Vec<SourceFile>, pub focus: Option<Focus> } // only changed files
pub enum EditError { NotFound(String), Invalid(String), WouldBreak(String) } // Display via thiserror
pub fn apply(files: &[SourceFile], ops: &[Op]) -> Result<Applied, EditError>
```

Rules:
- Ops run in order, and each op's line numbers refer to the result of the ops before it.
- `Place::After { line }` inserts after the statement starting at `line`, at its indentation.
- `Place::Start { header }` inserts as the first statement of the body opened at `header` (a label, choice or branch line). A body holding only `pass` has the `pass` replaced.
- `Replace` refuses `Choice` and `If` targets and blocks with `Invalid`; containers change through the dedicated ops.
- `Delete` leaves `pass` behind when it empties a body.
- `Move` refuses a place inside the moved statement.
- After all ops, every changed file is reparsed. If any file has more diagnostics than before, the call fails with `WouldBreak(first new message)`.

- [ ] **Step 1: Write the failing tests**

`crates/kairos-script/tests/edit.rs`:

```rust
use kairos_script::edit::{BlockKind, EditError, Op, Place, SourceFile, apply, scene_blocks};

fn file(source: &str) -> Vec<SourceFile> {
    vec![SourceFile { path: "script/a.kai".into(), source: source.into() }]
}

fn run(source: &str, ops: Vec<Op>) -> String {
    apply(&file(source), &ops).unwrap().files.into_iter().next().map(|f| f.source).unwrap_or_else(|| source.into())
}

fn say(text: &str) -> BlockKind {
    BlockKind::Dialogue { character: None, text: text.into() }
}

const P: &str = "script/a.kai";

#[test]
fn insert_after_and_at_start_keep_comments_and_blank_lines() {
    let src = "# intro\nlabel start:\n    \"One\"  # keep\n\n    \"Two\"\n";
    let out = run(src, vec![
        Op::Insert { path: P.into(), place: Place::After { line: 3 }, block: say("1.5") },
        Op::Insert { path: P.into(), place: Place::Start { header: 2 }, block: say("Zero") },
    ]);
    assert_eq!(out, "# intro\nlabel start:\n    \"Zero\"\n    \"One\"  # keep\n    \"1.5\"\n\n    \"Two\"\n");
}

#[test]
fn insert_keeps_crlf() {
    let out = run("label start:\r\n    \"One\"\r\n", vec![Op::Insert { path: P.into(), place: Place::After { line: 2 }, block: say("Two") }]);
    assert_eq!(out, "label start:\r\n    \"One\"\r\n    \"Two\"\r\n");
}

#[test]
fn insert_matches_sibling_indent() {
    let out = run("label start:\n  \"One\"\n", vec![Op::Insert { path: P.into(), place: Place::After { line: 2 }, block: say("Two") }]);
    assert_eq!(out, "label start:\n  \"One\"\n  \"Two\"\n");
}

#[test]
fn start_of_a_pass_body_replaces_the_pass() {
    let src = "label start:\n    menu:\n        \"Go\":\n            pass\n";
    let out = run(src, vec![Op::Insert { path: P.into(), place: Place::Start { header: 3 }, block: say("Went") }]);
    assert_eq!(out, "label start:\n    menu:\n        \"Go\":\n            \"Went\"\n");
}

#[test]
fn replace_keeps_a_trailing_comment_and_refuses_containers() {
    let src = "label start:\n    \"Old\"  # note\n    menu:\n        \"Go\":\n            pass\n";
    let out = run(src, vec![Op::Replace { path: P.into(), line: 2, block: say("New") }]);
    assert_eq!(out.lines().nth(1), Some("    \"New\"  # note"));
    let err = apply(&file(src), &[Op::Replace { path: P.into(), line: 3, block: say("x") }]).unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
}

#[test]
fn delete_leaves_pass_in_an_emptied_body_and_moves_reindent() {
    let src = "label start:\n    \"A\"\n    menu:\n        \"Go\":\n            \"B\"\n";
    assert_eq!(run(src, vec![Op::Delete { path: P.into(), line: 5 }]), "label start:\n    \"A\"\n    menu:\n        \"Go\":\n            pass\n");
    assert_eq!(
        run(src, vec![Op::Move { path: P.into(), line: 2, place: Place::Start { header: 4 } }]),
        "label start:\n    menu:\n        \"Go\":\n            \"A\"\n            \"B\"\n"
    );
    let err = apply(&file(src), &[Op::Move { path: P.into(), line: 3, place: Place::Start { header: 4 } }]).unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
}

#[test]
fn choice_and_branch_ops_edit_headers_only() {
    let src = "label start:\n    menu:\n        \"Go\" if a:  # c\n            jump x\n    if n > 1:\n        pass\nlabel x:\n    return\n";
    let out = run(src, vec![
        Op::SetChoiceText { path: P.into(), line: 3, text: "Leave \"now\"".into() },
        Op::SetChoiceCondition { path: P.into(), line: 3, condition: None },
        Op::AddChoiceOption { path: P.into(), line: 2, text: "Stay".into() },
        Op::SetBranchCondition { path: P.into(), line: 7, condition: "n > 2".into() },
        Op::AddBranch { path: P.into(), line: 7, condition: None },
    ]);
    assert_eq!(
        out,
        "label start:\n    menu:\n        \"Leave \\\"now\\\"\":  # c\n            jump x\n        \"Stay\":\n            pass\n    if n > 2:\n        pass\n    else:\n        pass\nlabel x:\n    return\n"
    );
}

#[test]
fn edits_that_break_parsing_are_rejected_but_existing_errors_are_tolerated() {
    let broken = BlockKind::Code { text: "menu:".into() };
    let err = apply(&file("label start:\n    \"A\"\n"), &[Op::Insert { path: P.into(), place: Place::After { line: 2 }, block: broken }]).unwrap_err();
    assert!(matches!(err, EditError::WouldBreak(_)));
}

#[test]
fn edits_allowed_when_file_already_has_errors() {
    let src = "label start:\n    \"A\"\nlabel other:\n    bogus statement\n";
    let out = run(src, vec![Op::Insert { path: P.into(), place: Place::After { line: 2 }, block: say("B") }]);
    assert!(out.contains("    \"B\"\nlabel other:"));
}

#[test]
fn unknown_lines_are_not_found() {
    let err = apply(&file("label start:\n    \"A\"\n"), &[Op::Delete { path: P.into(), line: 9 }]).unwrap_err();
    assert!(matches!(err, EditError::NotFound(_)));
}

proptest::proptest! {
    #[test]
    fn random_inserts_moves_and_deletes_always_parse(ops in proptest::collection::vec((0u8..3, 0usize..40, 0usize..40), 1..20)) {
        let mut source = include_str!("../../../examples/hello-kairos/script/script.kai").to_string();
        let (_, before) = kairos_script::parse(&source);
        for (kind, a, b) in ops {
            let scene = scene_blocks(&source, "start").unwrap();
            let lines: Vec<usize> = scene.blocks.iter().map(|b| b.line).collect();
            let pick = |i: usize| lines[i % lines.len()];
            let op = match kind {
                0 => Op::Insert { path: P.into(), place: Place::After { line: pick(a) }, block: say("inserted") },
                1 => Op::Move { path: P.into(), line: pick(a), place: Place::After { line: pick(b) } },
                _ if lines.len() > 1 => Op::Delete { path: P.into(), line: pick(a) },
                _ => continue,
            };
            if let Ok(applied) = apply(&file(&source), &[op]) {
                if let Some(f) = applied.files.into_iter().next() { source = f.source; }
            }
            let (_, after) = kairos_script::parse(&source);
            proptest::prop_assert!(after.len() <= before.len(), "{after:?}\n{source}");
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kairos-script --test edit`
Expected: compile errors (`apply`, `Op`, ... not found).

- [ ] **Step 3: Implement `ops.rs`**

```rust
use serde::{Deserialize, Serialize};

use crate::ast::StmtKind;

use super::blocks::{BlockKind, render};
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

/// Where `place` puts new lines: the 1-based line to insert before, the indentation, and whether
/// a lone `pass` at that spot should be replaced.
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

fn apply_one(ws: &mut Workspace, op: &Op) -> Result<Focus, EditError> {
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
            let raw = lines.text(n.line);
            let comment = &raw[crate::lexer::strip_comment(raw).trim_end().len()..];
            if n.line == n.end && new.len() == 1 && !comment.trim().is_empty() {
                new[0].push_str(comment);
            }
            lines.splice(n.line, n.end, new);
            n.line
        }
        Op::Delete { line, .. } => {
            let n = tree::find(&nodes, *line).ok_or_else(|| missing(*line))?;
            let body = tree::parent_body(&nodes, *line);
            let alone = body.is_some_and(|b| b.nodes.len() == 1);
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
            let body = tree::parent_body(&nodes, *line);
            let alone = body.is_some_and(|b| b.nodes.len() == 1);
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
            let moved = reindent(&text, from, indent);
            insert_lines(&mut lines, at, pass, moved)
        }
        Op::SetChoiceText { line, text, .. } => {
            let raw = lines.text(*line).to_string();
            let start = raw.len() - raw.trim_start().len();
            let rest = super::blocks::after_string_pub(&raw[start..]).to_string();
            lines.splice(*line, *line, vec![format!("{}{}{rest}", &raw[..start], quote(text))]);
            *line
        }
        Op::SetChoiceCondition { line, condition, .. } => {
            let raw = lines.text(*line).to_string();
            let start = raw.len() - raw.trim_start().len();
            let after = super::blocks::after_string_pub(&raw[start..]);
            let literal = &raw[start..raw.len() - after.len()];
            let comment = &after[crate::lexer::strip_comment(after).len()..];
            let cond = condition.as_ref().map(|c| format!(" if {c}")).unwrap_or_default();
            lines.splice(*line, *line, vec![format!("{}{literal}{cond}:{comment}", &raw[..start])]);
            *line
        }
        Op::AddChoiceOption { line, text, .. } => {
            let n = tree::find(&nodes, *line).filter(|n| matches!(n.stmt.kind, StmtKind::Menu { .. })).ok_or_else(|| missing(*line))?;
            let option_indent = n.bodies.first().map_or(n.indent + step, |b| lines.indent(b.header));
            let new = vec![format!("{}{}:", " ".repeat(option_indent), quote(text)), format!("{}pass", " ".repeat(option_indent + step))];
            lines.splice(n.end + 1, n.end, new);
            n.end + 1
        }
        Op::SetBranchCondition { line, condition, .. } => {
            let raw = lines.text(*line).to_string();
            let indent = raw.len() - raw.trim_start().len();
            let keyword = if raw.trim_start().starts_with("elif") { "elif" } else { "if" };
            if raw.trim_start().starts_with("else") {
                return Err(EditError::Invalid("`otherwise` has no condition".into()));
            }
            let comment = &raw[crate::lexer::strip_comment(&raw).len()..];
            lines.splice(*line, *line, vec![format!("{}{keyword} {condition}:{comment}", " ".repeat(indent))]);
            *line
        }
        Op::AddBranch { line, condition, .. } => {
            let n = tree::find(&nodes, *line).filter(|n| matches!(n.stmt.kind, StmtKind::If(_))).ok_or_else(|| missing(*line))?;
            let last = n.bodies.last().map(|b| b.header).unwrap_or(n.line);
            if lines.text(last).trim_start().starts_with("else") {
                return Err(EditError::Invalid("this condition already has an `otherwise`".into()));
            }
            let head = condition.as_ref().map(|c| format!("elif {c}:")).unwrap_or_else(|| "else:".into());
            let pad = " ".repeat(n.indent);
            lines.splice(n.end + 1, n.end, vec![format!("{pad}{head}"), format!("{pad}{}pass", " ".repeat(step))]);
            n.end + 1
        }
    };
    ws.set(&path, lines.join());
    Ok(Focus { path, line })
}
```

In `blocks.rs`, expose the helper: `pub(crate) fn after_string_pub(s: &str) -> &str { after_string(s) }`. Rename it to `after_string` and make the original `pub(crate)` instead if the reviewer prefers one name.

`mod.rs`: `mod ops;` and `pub use ops::{Applied, EditError, Focus, Op, Place, SourceFile, apply};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kairos-script --test edit`
Expected: 11 passed, including the property test. If `choice_and_branch_ops_edit_headers_only` fails on line numbers, recheck the rule that each op's lines refer to the previous op's output: after `AddChoiceOption` inserts 2 lines, the `if` moves from line 5 to line 7, which is why the test uses 7.

- [ ] **Step 5: Commit**

```bash
git add crates/kairos-script
git commit -m "Apply block edits that change only the lines they touch"
```

---

### Task 5: Scene and link operations

**Files:**
- Create: `crates/kairos-script/src/edit/scenes.rs`
- Modify: `crates/kairos-script/src/edit/ops.rs` (new `Op` variants dispatch to `scenes`), `mod.rs`
- Test: `crates/kairos-script/tests/edit.rs`

**Interfaces:**
- Produces, as new `Op` variants:
  - `AddScene { path, name, after: Option<String> }`
  - `RenameScene { from, to }`
  - `DeleteScene { label, remove_references: bool }`
  - `DuplicateScene { label, name }`
  - `Connect { from, to, choice: Option<String> }`
  - `RemoveLink { from, to, choice: Option<String> }`
  - `EndScene { label }`
  - `SetCharacter { path, id, name, color: Option<String> }`
- Produces `pub struct Reference { pub path: String, pub line: usize, pub kind: RefKind, pub choice: Option<String> }`, where `RefKind` is `Jump | Call` (camelCase serde).
- Produces `pub fn references(files: &[SourceFile], label: &str) -> Vec<Reference>`. `choice` is the option text when the `jump`/`call` sits directly in a choice body.

Scene ops find their label in whichever file defines it. `AddScene` refuses a taken name (`Invalid`); the UI calls `label_from_name` first. A new scene is `label name:` plus `    ""` plus `    return`, preceded by one blank line unless the file is empty.

- [ ] **Step 1: Write the failing tests** (append to `tests/edit.rs`)

```rust
use kairos_script::edit::{RefKind, references};

fn files(a: &str, b: &str) -> Vec<SourceFile> {
    vec![SourceFile { path: "script/a.kai".into(), source: a.into() }, SourceFile { path: "script/b.kai".into(), source: b.into() }]
}

fn run_all(fs: &[SourceFile], ops: Vec<Op>) -> Vec<String> {
    let applied = apply(fs, &ops).unwrap();
    fs.iter().map(|f| applied.files.iter().find(|c| c.path == f.path).map_or(f.source.clone(), |c| c.source.clone())).collect()
}

#[test]
fn add_scene_after_a_label_or_at_the_end() {
    let fs = files("label start:\n    jump day\nlabel day:\n    return\n", "");
    let out = run_all(&fs, vec![Op::AddScene { path: "script/a.kai".into(), name: "beach".into(), after: Some("start".into()) }]);
    assert_eq!(out[0], "label start:\n    jump day\n\nlabel beach:\n    \"\"\n    return\nlabel day:\n    return\n");
    let out = run_all(&fs, vec![Op::AddScene { path: "script/b.kai".into(), name: "end".into(), after: None }]);
    assert_eq!(out[1], "label end:\n    \"\"\n    return\n");
    assert!(matches!(apply(&fs, &[Op::AddScene { path: "script/a.kai".into(), name: "day".into(), after: None }]), Err(EditError::Invalid(_))));
}

#[test]
fn rename_changes_only_exact_references() {
    let fs = files(
        "label day:\n    jump day_two\n    call day\nlabel day_two:\n    jump day\n",
        "label other:\n    menu:\n        \"Back\":\n            jump day\n",
    );
    let out = run_all(&fs, vec![Op::RenameScene { from: "day".into(), to: "morning".into() }]);
    assert_eq!(out[0], "label morning:\n    jump day_two\n    call morning\nlabel day_two:\n    jump morning\n");
    assert_eq!(out[1], "label other:\n    menu:\n        \"Back\":\n            jump morning\n");
}

#[test]
fn references_report_jumps_calls_and_choices() {
    let fs = files("label a:\n    jump x\n    call x\n", "label b:\n    menu:\n        \"Go\":\n            jump x\nlabel x:\n    return\n");
    let refs: Vec<_> = references(&fs, "x").into_iter().map(|r| (r.path, r.line, r.kind, r.choice)).collect();
    assert_eq!(refs, [
        ("script/a.kai".into(), 2, RefKind::Jump, None),
        ("script/a.kai".into(), 3, RefKind::Call, None),
        ("script/b.kai".into(), 4, RefKind::Jump, Some("Go".into())),
    ]);
}

#[test]
fn delete_scene_can_remove_links_to_it() {
    let fs = files(
        "label a:\n    \"Hi\"\n    jump x\n    menu:\n        \"Go\":\n            jump x\n        \"Stay\":\n            \"Ok\"\n",
        "label x:\n    return\n",
    );
    let out = run_all(&fs, vec![Op::DeleteScene { label: "x".into(), remove_references: true }]);
    assert_eq!(out[0], "label a:\n    \"Hi\"\n    menu:\n        \"Stay\":\n            \"Ok\"\n");
    assert_eq!(out[1], "");
    let out = run_all(&fs, vec![Op::DeleteScene { label: "x".into(), remove_references: false }]);
    assert!(out[0].contains("jump x"));
}

#[test]
fn connect_adds_jumps_and_choices_and_remove_link_undoes_them() {
    let fs = files("label a:\n    \"Hi\"\nlabel b:\n    return\nlabel c:\n    return\n", "");
    let out = run_all(&fs, vec![
        Op::Connect { from: "a".into(), to: "b".into(), choice: Some("To B".into()) },
        Op::Connect { from: "a".into(), to: "c".into(), choice: Some("To C".into()) },
    ]);
    assert_eq!(out[0], "label a:\n    \"Hi\"\n    menu:\n        \"To B\":\n            jump b\n        \"To C\":\n            jump c\nlabel b:\n    return\nlabel c:\n    return\n");
    let fs2 = files(&out[0], "");
    let out = run_all(&fs2, vec![Op::RemoveLink { from: "a".into(), to: "b".into(), choice: Some("To B".into()) }]);
    assert!(!out[0].contains("To B") && out[0].contains("To C"));
    let out = run_all(&fs, vec![Op::Connect { from: "a".into(), to: "b".into(), choice: None }, Op::EndScene { label: "b".into() }]);
    assert!(out[0].starts_with("label a:\n    \"Hi\"\n    jump b\n"));
}

#[test]
fn duplicate_and_set_character() {
    let fs = files("character e \"Eileen\"\nlabel a:\n    e \"Hi\"\n", "");
    let out = run_all(&fs, vec![
        Op::DuplicateScene { label: "a".into(), name: "a_copy".into() },
        Op::SetCharacter { path: "script/a.kai".into(), id: "e".into(), name: "Eve".into(), color: Some("#ffcc00".into()) },
        Op::SetCharacter { path: "script/a.kai".into(), id: "k".into(), name: "Kai".into(), color: None },
    ]);
    assert_eq!(out[0], "character e \"Eve\" color=\"#ffcc00\"\ncharacter k \"Kai\"\nlabel a:\n    e \"Hi\"\n\nlabel a_copy:\n    e \"Hi\"\n");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p kairos-script --test edit`
Expected: compile errors for the new variants and `references`.

- [ ] **Step 3: Implement `scenes.rs` and wire the variants**

Add the eight variants to `Op` in `ops.rs`. In `apply_one`, dispatch them first:

```rust
if let Some(focus) = super::scenes::apply_scene_op(ws, op)? {
    return Ok(focus);
}
```

Add a `_ => unreachable!("scene ops are handled above")` arm to the path match. Make `Workspace` and its methods `pub(crate)`.

`scenes.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::ast::StmtKind;

use super::ops::{EditError, Focus, Op, SourceFile, Workspace, step};
use super::text::{Lines, quote};
use super::tree::{self, Body, Node};

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

fn defining_file(ws: &Workspace, label: &str) -> Result<String, EditError> {
    ws.files
        .iter()
        .find(|f| crate::parse(&f.source).0.iter().any(|s| matches!(&s.kind, StmtKind::Label { name, .. } if name == label)))
        .map(|f| f.path.clone())
        .ok_or_else(|| EditError::NotFound(format!("there is no scene `{label}`")))
}

/// Applies `edit` to the lines and statement tree of `path` and stores the result.
fn with_file<T>(ws: &mut Workspace, path: &str, edit: impl FnOnce(&mut Lines, &[Node], usize) -> Result<T, EditError>) -> Result<T, EditError> {
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

/// Removes one `jump`/`call` to `to` from `nodes`: the whole choice when that statement is all
/// the choice does, and the whole menu when that was its last choice.
fn removal(nodes: &[Node], to: &str, choice: Option<&str>) -> Option<(usize, usize)> {
    for n in nodes {
        match &n.stmt.kind {
            StmtKind::Jump(t) | StmtKind::Call(t) if t == to && choice.is_none() => return Some((n.line, n.end)),
            StmtKind::Menu { choices, .. } => {
                for (c, b) in choices.iter().zip(&n.bodies) {
                    let only_link = b.nodes.len() == 1 && matches!(&b.nodes[0].stmt.kind, StmtKind::Jump(t) | StmtKind::Call(t) if t == to);
                    let wanted = choice.is_none_or(|text| text == c.text);
                    if wanted && only_link {
                        return Some(if choices.len() == 1 { (n.line, n.end) } else { (b.header, b.end) });
                    }
                    if wanted && choice.is_some() {
                        if let Some(r) = removal(&b.nodes, to, None) {
                            return Some(r);
                        }
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

pub(crate) fn apply_scene_op(ws: &mut Workspace, op: &Op) -> Result<Option<Focus>, EditError> {
    let focus = match op {
        Op::AddScene { path, name, after } => {
            if label_names(ws).contains(name) {
                return Err(EditError::Invalid(format!("a scene named `{name}` already exists")));
            }
            let line = with_file(ws, path, |lines, nodes, step| {
                let mut text = scene_text(name, step);
                let at = match after.as_deref().and_then(|a| tree::label(nodes, a)) {
                    Some(n) => n.end + 1,
                    None => lines.len() + 1,
                };
                if lines.len() > 0 {
                    text.insert(0, String::new());
                }
                let blank = usize::from(lines.len() > 0);
                lines.splice(at, at - 1, text);
                Ok(at + blank)
            })?;
            Focus { path: path.clone(), line }
        }
        Op::RenameScene { from, to } => {
            if label_names(ws).contains(to) {
                return Err(EditError::Invalid(format!("a scene named `{to}` already exists")));
            }
            let home = defining_file(ws, from)?;
            let paths: Vec<String> = ws.files.iter().map(|f| f.path.clone()).collect();
            let mut focus = None;
            for path in paths {
                let refs: Vec<usize> = references(&[SourceFile { path: path.clone(), source: ws.source(&path)?.into() }], from).iter().map(|r| r.line).collect();
                with_file(ws, &path, |lines, nodes, _| {
                    for line in refs {
                        let raw = lines.text(line).to_string();
                        let renamed = rename_target(&raw, from, to);
                        lines.splice(line, line, vec![renamed]);
                    }
                    if let Some(n) = tree::label(nodes, from) {
                        let raw = lines.text(n.line).to_string();
                        lines.splice(n.line, n.line, vec![raw.replacen(&format!("label {from}"), &format!("label {to}"), 1)]);
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
                    loop {
                        let changed = with_file(ws, &path, |lines, nodes, _| {
                            let Some((first, last)) = removal(nodes, label, None) else { return Ok(false) };
                            lines.splice(first, last, Vec::new());
                            Ok(true)
                        })?;
                        if !changed {
                            break;
                        }
                    }
                }
            }
            Focus { path: home, line }
        }
        Op::DuplicateScene { label, name } => {
            if label_names(ws).contains(name) {
                return Err(EditError::Invalid(format!("a scene named `{name}` already exists")));
            }
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
                    None => {
                        if last.is_some_and(|l| matches!(&l.stmt.kind, StmtKind::Jump(t) if t == to)) {
                            return Ok(last.unwrap().line);
                        }
                        Ok(append_to_body(lines, n, vec![format!("jump {to}")], step))
                    }
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
                let emptied = n.bodies[0].nodes.len() == 1 && (first, last) == (n.bodies[0].nodes[0].line, n.bodies[0].nodes[0].end);
                let filler = if emptied { vec![format!("{}pass", " ".repeat(lines.indent(first)))] } else { Vec::new() };
                lines.splice(first, last, filler);
                Ok(n.line)
            })?;
            Focus { path: home, line }
        }
        Op::EndScene { label } => {
            let home = defining_file(ws, label)?;
            let line = with_file(ws, &home, |lines, nodes, step| {
                Ok(append_to_body(lines, tree::label(nodes, label).expect("defining_file found it"), vec!["return".into()], step))
            })?;
            Focus { path: home, line }
        }
        Op::SetCharacter { path, id, name, color } => {
            let line = with_file(ws, path, |lines, nodes, _| {
                let text = format!(
                    "character {id} {}{}",
                    quote(name),
                    color.as_ref().map(|c| format!(" color={}", quote(c))).unwrap_or_default()
                );
                let existing = nodes.iter().find(|n| matches!(&n.stmt.kind, StmtKind::Character { id: i, .. } if i == id));
                if let Some(n) = existing {
                    lines.splice(n.line, n.line, vec![text]);
                    return Ok(n.line);
                }
                let at = nodes.iter().filter(|n| matches!(n.stmt.kind, StmtKind::Character { .. })).map(|n| n.end + 1).max().unwrap_or(1);
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
```

`Body` is imported for clarity in signatures. If clippy flags it as unused, remove that import.

`mod.rs`: `mod scenes;` and `pub use scenes::{RefKind, Reference, references};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p kairos-script`
Expected: all pass, including the earlier snapshot and unit tests.

- [ ] **Step 5: Commit**

```bash
git add crates/kairos-script
git commit -m "Add scene, link and character edits across script files"
```

---

### Task 6: napi bridge, IPC and shared types

**Files:**
- Create: `crates/kairos-napi/src/edit.rs`, `apps/editor/src/shared/edit-types.ts`, `apps/editor/src/main/edit.ts`
- Modify: `crates/kairos-napi/src/lib.rs`, `crates/kairos-napi/Cargo.toml` (add `serde_json = "1"`), `apps/editor/src/main/analysis.ts` (Addon interface), `apps/editor/src/main/index.ts`, `apps/editor/src/preload/index.ts`, `apps/editor/src/shared/types.ts`
- Test: unit tests in `crates/kairos-napi/src/edit.rs`; `apps/editor/src/main/edit.test.ts`

**Interfaces:**
- Produces (napi, JSON strings in and out):
  - `sceneBlocks(source: string, label: string): string`, returning `Scene | null` JSON
  - `applyEdits(filesJson: string, opsJson: string): string`, returning `{ "ok": Applied } | { "error": string }` JSON
  - `references(filesJson: string, label: string): string`
  - `labelFromName(name: string, takenJson: string): string`
- Produces in TS `edit-types.ts`: `Block`, `BlockKind` (a discriminated union on `kind`), `ChoiceOption`, `IfBranch`, `SetOp`, `Scene`, `Place`, `EditOp` (discriminated on `op`), `Applied`, `Reference`, `EditorLayout`.
- Produces `KairosApi.edit`:
  - `blocks(source: string, label: string): Promise<Scene | null>`
  - `apply(files: SourceFile[], ops: EditOp[]): Promise<Applied>`, which rejects with the plain error message
  - `references(files: SourceFile[], label: string): Promise<Reference[]>`
  - `labelFromName(name: string, taken: string[]): Promise<string>`
  - `readLayout(): Promise<EditorLayout>`
  - `writeLayout(layout: EditorLayout): Promise<void>`
  - `setStart(label: string): Promise<void>`

- [ ] **Step 1: Write the failing Rust test** in `crates/kairos-napi/src/edit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shapes_match_the_editor_types() {
        let blocks = scene_blocks_json("label start:\n    e \"Hi\"\n", "start");
        assert_eq!(blocks, r#"{"label":"start","line":1,"blocks":[{"line":2,"endLine":2,"kind":"dialogue","character":"e","text":"Hi"}]}"#);
        let files = r#"[{"path":"a.kai","source":"label start:\n    \"A\"\n"}]"#;
        let ops = r#"[{"op":"insert","path":"a.kai","place":{"at":"after","line":2},"block":{"kind":"end"}}]"#;
        assert_eq!(
            apply_edits_json(files, ops),
            r#"{"ok":{"files":[{"path":"a.kai","source":"label start:\n    \"A\"\n    return\n"}],"focus":{"path":"a.kai","line":3}}}"#
        );
        assert!(apply_edits_json(files, r#"[{"op":"delete","path":"a.kai","line":9}]"#).starts_with(r#"{"error":"#));
        assert_eq!(label_from_name_json("Beach Day", "[]"), "beach_day");
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p kairos-napi edit`
Expected: compile errors.

- [ ] **Step 3: Implement the bridge**

`crates/kairos-napi/src/edit.rs`:

```rust
use kairos_script::edit::{self, Op, SourceFile};

pub fn scene_blocks_json(source: &str, label: &str) -> String {
    serde_json::to_string(&edit::scene_blocks(source, label)).expect("blocks serialize")
}

pub fn apply_edits_json(files: &str, ops: &str) -> String {
    let parsed = serde_json::from_str::<Vec<SourceFile>>(files)
        .map_err(|e| format!("bad files: {e}"))
        .and_then(|f| serde_json::from_str::<Vec<Op>>(ops).map(|o| (f, o)).map_err(|e| format!("bad edit: {e}")));
    let result = parsed.and_then(|(f, o)| edit::apply(&f, &o).map_err(|e| e.to_string()));
    match result {
        Ok(applied) => serde_json::json!({ "ok": applied }).to_string(),
        Err(message) => serde_json::json!({ "error": message }).to_string(),
    }
}

pub fn references_json(files: &str, label: &str) -> String {
    let files: Vec<SourceFile> = serde_json::from_str(files).unwrap_or_default();
    serde_json::to_string(&edit::references(&files, label)).expect("references serialize")
}

pub fn label_from_name_json(name: &str, taken: &str) -> String {
    let taken: Vec<String> = serde_json::from_str(taken).unwrap_or_default();
    edit::label_from_name(name, &taken)
}
```

`lib.rs`: add `mod edit;`. Inside `mod bindings`, add:

```rust
#[napi(js_name = "sceneBlocks")]
pub fn scene_blocks(source: String, label: String) -> String {
    crate::edit::scene_blocks_json(&source, &label)
}

#[napi(js_name = "applyEdits")]
pub fn apply_edits(files_json: String, ops_json: String) -> String {
    crate::edit::apply_edits_json(&files_json, &ops_json)
}

#[napi(js_name = "references")]
pub fn references(files_json: String, label: String) -> String {
    crate::edit::references_json(&files_json, &label)
}

#[napi(js_name = "labelFromName")]
pub fn label_from_name(name: String, taken_json: String) -> String {
    crate::edit::label_from_name_json(&name, &taken_json)
}
```

`serde_json::json!` needs `Applied: Serialize`, which Task 4 derives.

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p kairos-napi`
Expected: all pass. Then `bun run build:napi` regenerates `index.d.ts` and `index.js`. Commit those too.

- [ ] **Step 5: Write the TS types** in `apps/editor/src/shared/edit-types.ts`:

```ts
import type { SourceFile } from "./types";

export type SetOp = "set" | "add" | "subtract" | "toggle";

export interface ChoiceOption {
  line: number;
  text: string;
  condition: string | null;
  blocks: Block[];
}

export interface IfBranch {
  line: number;
  condition: string | null;
  blocks: Block[];
}

export type BlockKind =
  | { kind: "dialogue"; character: string | null; text: string }
  | { kind: "background"; image: string[]; transition: string | null }
  | { kind: "show"; image: string[]; position: string | null; transition: string | null }
  | { kind: "hide"; character: string; transition: string | null }
  | { kind: "audio"; channel: string; asset: string; looped: boolean; fadeIn: number | null }
  | { kind: "stopAudio"; channel: string; fadeOut: number | null }
  | { kind: "pause"; seconds: number | null }
  | { kind: "choice"; options: ChoiceOption[] }
  | { kind: "goTo"; scene: string }
  | { kind: "call"; scene: string }
  | { kind: "setVariable"; name: string; op: SetOp; value: string }
  | { kind: "if"; branches: IfBranch[] }
  | { kind: "end" }
  | { kind: "code"; text: string };

export type Block = BlockKind & { line: number; endLine: number };

export interface Scene {
  label: string;
  line: number;
  blocks: Block[];
}

export type Place = { at: "after"; line: number } | { at: "start"; header: number };

export type EditOp =
  | { op: "insert"; path: string; place: Place; block: BlockKind }
  | { op: "replace"; path: string; line: number; block: BlockKind }
  | { op: "move"; path: string; line: number; place: Place }
  | { op: "delete"; path: string; line: number }
  | { op: "setChoiceText"; path: string; line: number; text: string }
  | { op: "setChoiceCondition"; path: string; line: number; condition: string | null }
  | { op: "addChoiceOption"; path: string; line: number; text: string }
  | { op: "setBranchCondition"; path: string; line: number; condition: string }
  | { op: "addBranch"; path: string; line: number; condition: string | null }
  | { op: "addScene"; path: string; name: string; after: string | null }
  | { op: "renameScene"; from: string; to: string }
  | { op: "deleteScene"; label: string; removeReferences: boolean }
  | { op: "duplicateScene"; label: string; name: string }
  | { op: "connect"; from: string; to: string; choice: string | null }
  | { op: "removeLink"; from: string; to: string; choice: string | null }
  | { op: "endScene"; label: string }
  | { op: "setCharacter"; path: string; id: string; name: string; color: string | null };

export interface Applied {
  files: SourceFile[];
  focus: { path: string; line: number } | null;
}

export interface Reference {
  path: string;
  line: number;
  kind: "jump" | "call";
  choice: string | null;
}

export interface EditorLayout {
  graph: Record<string, { x: number; y: number }>;
}
```

- [ ] **Step 6: Write the failing main-process test** in `apps/editor/src/main/edit.test.ts`:

```ts
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { readLayout, setStartLabel, writeLayout } from "./edit";

describe("editor files", () => {
  it("round-trips the graph layout and tolerates a missing file", async () => {
    const root = await mkdtemp(join(tmpdir(), "kairos-layout-"));
    expect(await readLayout(root)).toEqual({ graph: {} });
    await writeLayout(root, { graph: { start: { x: 10, y: 20 } } });
    expect(await readLayout(root)).toEqual({ graph: { start: { x: 10, y: 20 } } });
  });

  it("changes only the start key in kairos.toml", async () => {
    const root = await mkdtemp(join(tmpdir(), "kairos-start-"));
    await writeFile(join(root, "kairos.toml"), 'name = "P"\nstart = "start"\n\n[window]\nwidth = 1280\n');
    await setStartLabel(root, "beach");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe('name = "P"\nstart = "beach"\n\n[window]\nwidth = 1280\n');
    await writeFile(join(root, "kairos.toml"), 'name = "P"\n');
    await setStartLabel(root, "beach");
    expect(await readFile(join(root, "kairos.toml"), "utf8")).toBe('name = "P"\nstart = "beach"\n');
  });
});
```

Run: `bun run --cwd apps/editor test src/main/edit.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 7: Implement** `apps/editor/src/main/edit.ts`:

```ts
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { Applied, EditOp, EditorLayout, Reference, Scene } from "@shared/edit-types";
import type { SourceFile } from "@shared/types";

import { napi } from "./analysis";

export function sceneBlocks(source: string, label: string): Scene | null {
  return JSON.parse(napi().sceneBlocks(source, label)) as Scene | null;
}

export function applyEdits(files: SourceFile[], ops: EditOp[]): Applied {
  const result = JSON.parse(napi().applyEdits(JSON.stringify(files), JSON.stringify(ops))) as
    | { ok: Applied }
    | { error: string };
  if ("error" in result) throw new Error(result.error);
  return result.ok;
}

export function references(files: SourceFile[], label: string): Reference[] {
  return JSON.parse(napi().references(JSON.stringify(files), label)) as Reference[];
}

export function labelFromName(name: string, taken: string[]): string {
  return napi().labelFromName(name, JSON.stringify(taken));
}

export async function readLayout(root: string): Promise<EditorLayout> {
  const raw = await readFile(join(root, ".kairos/editor.json"), "utf8").catch(() => null);
  const parsed = raw ? (JSON.parse(raw) as Partial<EditorLayout>) : {};
  return { graph: parsed.graph ?? {} };
}

export async function writeLayout(root: string, layout: EditorLayout): Promise<void> {
  await mkdir(join(root, ".kairos"), { recursive: true });
  await writeFile(join(root, ".kairos/editor.json"), `${JSON.stringify(layout, null, 2)}\n`, "utf8");
}

/** Edits the `start` line in place so the rest of `kairos.toml` keeps its formatting. */
export async function setStartLabel(root: string, label: string): Promise<void> {
  const path = join(root, "kairos.toml");
  const text = await readFile(path, "utf8");
  const line = `start = ${JSON.stringify(label)}`;
  const next = /^start\s*=.*$/m.test(text)
    ? text.replace(/^start\s*=.*$/m, line)
    : text.replace(/^(name\s*=.*\n)/m, `$1${line}\n`);
  await writeFile(path, next, "utf8");
}
```

Extend the `Addon` interface in `analysis.ts` with `sceneBlocks(source: string, label: string): string; applyEdits(files: string, ops: string): string; references(files: string, label: string): string; labelFromName(name: string, taken: string): string;`.

`index.ts`: register the handlers. `projectRoot()` comes from `./project`. Throw an Error with a plain message when no project is open.

```ts
ipcMain.handle("edit:blocks", (_e, source: string, label: string) => sceneBlocks(source, label));
ipcMain.handle("edit:apply", (_e, files: SourceFile[], ops: EditOp[]) => applyEdits(files, ops));
ipcMain.handle("edit:references", (_e, files: SourceFile[], label: string) => references(files, label));
ipcMain.handle("edit:label-from-name", (_e, name: string, taken: string[]) => labelFromName(name, taken));
ipcMain.handle("edit:read-layout", () => readLayout(requireOpenRoot()));
ipcMain.handle("edit:write-layout", (_e, layout: EditorLayout) => writeLayout(requireOpenRoot(), layout));
ipcMain.handle("edit:set-start", (_e, label: string) => setStartLabel(requireOpenRoot(), label));
```

with `const requireOpenRoot = () => projectRoot() ?? (() => { throw new Error("Open a project first"); })();`.

`preload/index.ts`: add an `edit` group that mirrors the channels. Electron puts the main-process error message inside its own wrapper, so strip that prefix:

```ts
edit: {
  blocks: (source, label) => ipcRenderer.invoke("edit:blocks", source, label),
  apply: (files, ops) =>
    ipcRenderer.invoke("edit:apply", files, ops).catch((e: Error) => {
      throw new Error(e.message.replace(/^Error invoking remote method '[^']+': (Error: )?/, ""));
    }),
  references: (files, label) => ipcRenderer.invoke("edit:references", files, label),
  labelFromName: (name, taken) => ipcRenderer.invoke("edit:label-from-name", name, taken),
  readLayout: () => ipcRenderer.invoke("edit:read-layout"),
  writeLayout: (layout) => ipcRenderer.invoke("edit:write-layout", layout),
  setStart: (label) => ipcRenderer.invoke("edit:set-start", label),
},
```

`shared/types.ts`: add the matching `edit` member to `KairosApi`, importing the types from `./edit-types`.

`project.ts`: when reading the project tree, skip `.kairos`. `walk` already skips dot-entries, so check that `.kairos` doesn't appear in the tree.

- [ ] **Step 8: Run all editor checks**

Run: `bun run --cwd apps/editor test && bun run typecheck`
Expected: PASS. `store.test.ts`'s `fakeApi` needs an `edit` member to typecheck. Add one whose functions throw `new Error("unused")`; Task 7 replaces it.

- [ ] **Step 9: Commit**

```bash
git add crates/kairos-napi apps/editor/src
git commit -m "Expose script edits, scene blocks and editor layout to the editor"
```

---

### Task 7: Edit action and shared undo history

**Files:**
- Create: `apps/editor/src/renderer/src/store/history-slice.ts`, `apps/editor/src/renderer/src/store/edit-slice.ts`
- Modify: `store/types.ts`, `store/index.ts`, `store/project-slice.ts`, `store/store.test.ts`, `features/editor/editor-view.tsx`, `features/characters/character-manager.tsx`, `features/stage/asset-drag.ts`, `features/graph/story-graph.tsx`, `main/index.ts`, `main/project.ts`, `shared/types.ts`, `preload/index.ts`
- Delete: `apps/editor/src/shared/edits.ts`, `apps/editor/src/shared/edits.test.ts`
- Test: `store/store.test.ts`

**Interfaces:**
- Consumes: `KairosApi.edit` (Task 6).
- Produces in `AppState`:
  - `edit(ops: EditOp[]): Promise<{ path: string; line: number } | null>`. It returns the focus, or `null` when the edit was rejected or stale.
  - `undo(): void`, `redo(): void`, `canUndo: boolean`, `canRedo: boolean`
  - `editError: string | null`, the last rejection message for the UI
  - `layout` is already taken by `UiSlice`, so the graph layout is `graphLayout: EditorLayout`, with `setNodePosition(label: string, x: number, y: number): void` and `resetGraphLayout(): void`
- History entry: `{ files: Record<string, { before: string; after: string }>; kind: "visual" | "code"; at: number }`.

- [ ] **Step 1: Write the failing tests** (add to `store.test.ts`)

Replace `fakeApi`'s `edit` member with a fake that implements `apply` for the two ops the tests use:

```ts
edit: {
  blocks: vi.fn(async () => null),
  apply: vi.fn(async (files: SourceFile[], ops: EditOp[]) => {
    const op = ops[0]!;
    if (op.op === "endScene" && op.label === "missing") throw new Error("there is no scene `missing`");
    const file = files.find((f) => f.path === "script/a.kai")!;
    if (op.op === "connect") return { files: [{ path: file.path, source: insertJump(file.source, op.from, op.to) }], focus: { path: file.path, line: 3 } };
    return { files: [{ path: file.path, source: `${file.source}# edited\n` }], focus: { path: file.path, line: 1 } };
  }),
  references: vi.fn(async () => []),
  labelFromName: vi.fn(async (name: string) => name.toLowerCase().replace(/\W+/g, "_")),
  readLayout: vi.fn(async () => ({ graph: { start: { x: 5, y: 6 } } })),
  writeLayout: vi.fn(async () => undefined),
  setStart: vi.fn(async () => undefined),
},
```

`insertJump` was imported from `@shared/edits`, which is being deleted. Inline a test-only helper instead:

```ts
const insertJump = (source: string, from: string, to: string) =>
  source.replace(new RegExp(`(label ${from}:\\n(?:    .*\\n)*)`), `$1    jump ${to}\n`);
```

New tests:

```ts
it("applies visual edits to buffers and undoes them with code edits in one history", async () => {
  await store.getState().edit([{ op: "endScene", label: "start" }]);
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# edited\n`);
  store.getState().setSource("script/a.kai", `${SCRIPT}# edited\n# typed\n`);
  expect(store.getState().canUndo).toBe(true);
  store.getState().undo();
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# edited\n`);
  store.getState().undo();
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(SCRIPT);
  store.getState().redo();
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# edited\n`);
});

it("coalesces typing into one history entry per pause", () => {
  vi.useFakeTimers();
  store.getState().setSource("script/a.kai", `${SCRIPT}a`);
  store.getState().setSource("script/a.kai", `${SCRIPT}ab`);
  vi.advanceTimersByTime(1500);
  store.getState().setSource("script/a.kai", `${SCRIPT}abc`);
  store.getState().undo();
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}ab`);
  store.getState().undo();
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(SCRIPT);
});

it("reports rejected edits without touching buffers", async () => {
  expect(await store.getState().edit([{ op: "endScene", label: "missing" }])).toBeNull();
  expect(store.getState().editError).toBe("there is no scene `missing`");
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(SCRIPT);
});

it("drops edits whose buffers changed while they were in flight", async () => {
  const pending = store.getState().edit([{ op: "endScene", label: "start" }]);
  store.getState().setSource("script/a.kai", `${SCRIPT}# typed meanwhile\n`);
  expect(await pending).toBeNull();
  expect(store.getState().buffers["script/a.kai"]!.source).toBe(`${SCRIPT}# typed meanwhile\n`);
});

it("loads and persists graph positions", async () => {
  expect(store.getState().graphLayout.graph.start).toEqual({ x: 5, y: 6 });
  store.getState().setNodePosition("end", 1, 2);
  expect(env.api.edit.writeLayout).toHaveBeenCalledWith({ graph: { start: { x: 5, y: 6 }, end: { x: 1, y: 2 } } });
});

it("resets history when a project opens", async () => {
  await store.getState().edit([{ op: "endScene", label: "start" }]);
  await store.getState().openProject("/p");
  expect(store.getState().canUndo).toBe(false);
});
```

Update the existing connect test to call `edit` and expect `api.edit.apply`, since `api.project.applyEdit` no longer exists:

```ts
it("connects labels through a visual edit", async () => {
  await store.getState().edit([{ op: "connect", from: "start", to: "end", choice: null }]);
  expect(store.getState().buffers["script/a.kai"]!.source).toContain("jump end");
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `bun run --cwd apps/editor test src/renderer/src/store`
Expected: FAIL (`edit`, `undo` ... not functions).

- [ ] **Step 3: Implement**

`store/types.ts` additions:

```ts
import type { EditOp, EditorLayout } from "@shared/edit-types";

export interface HistoryEntry {
  files: Record<string, { before: string; after: string }>;
  kind: "visual" | "code";
  at: number;
}

export interface HistorySlice {
  past: HistoryEntry[];
  future: HistoryEntry[];
  canUndo: boolean;
  canRedo: boolean;
  record(entry: HistoryEntry, coalesce?: boolean): void;
  undo(): void;
  redo(): void;
  clearHistory(): void;
}

export interface EditSlice {
  editError: string | null;
  graphLayout: EditorLayout;
  edit(ops: EditOp[]): Promise<{ path: string; line: number } | null>;
  loadGraphLayout(): Promise<void>;
  setNodePosition(label: string, x: number, y: number): void;
  resetGraphLayout(): void;
}
```

Add both to `AppState`. Remove `connectLabels` and `saveCharacter` from `ProjectSlice`. Keep `insertAtCursor`, which inserts statement text at the Monaco cursor in the code view, but change it to a plain buffer splice with no edits.ts dependency:

```ts
insertAtCursor(statement, line) {
  const { cursor, activeId, tabs, buffers } = get();
  const active = tabs.find((t) => t.id === activeId);
  const path = active?.kind === "file" ? active.path : cursor?.path;
  const buffer = path ? buffers[path] : undefined;
  if (!path || !buffer) return false;
  const lines = buffer.source.split("\n");
  const at = Math.min(line ?? (cursor?.path === path ? cursor.line : lines.length), lines.length);
  const ref = lines[at - 1] ?? "";
  const indent = (ref.match(/^\s*/)?.[0] ?? "") + (ref.trimEnd().endsWith(":") ? "    " : "");
  lines.splice(at, 0, indent + statement);
  get().setSource(path, lines.join("\n"));
  get().openFile(path, at + 1);
  return true;
},
```

`history-slice.ts`:

```ts
import type { HistoryEntry, HistorySlice, SliceCreator } from "./types";

const LIMIT = 200;
const TYPING_PAUSE_MS = 1000;

export const createHistorySlice: SliceCreator<HistorySlice> = (set, get) => {
  const restore = (entry: HistoryEntry, side: "before" | "after") => {
    set((s) => {
      const buffers = { ...s.buffers };
      for (const [path, change] of Object.entries(entry.files)) {
        const buffer = buffers[path];
        if (buffer) buffers[path] = { ...buffer, source: change[side] };
      }
      return { buffers };
    });
    get().scheduleAnalyze();
  };
  const flags = (past: HistoryEntry[], future: HistoryEntry[]) => ({ past, future, canUndo: past.length > 0, canRedo: future.length > 0 });

  return {
    past: [],
    future: [],
    canUndo: false,
    canRedo: false,
    record(entry, coalesce = false) {
      set((s) => {
        const last = s.past.at(-1);
        const paths = Object.keys(entry.files);
        const mergeable =
          coalesce &&
          last?.kind === "code" &&
          entry.at - last.at < TYPING_PAUSE_MS &&
          paths.length === 1 &&
          Object.keys(last.files).join() === paths[0];
        if (mergeable) {
          const path = paths[0]!;
          const merged = { ...last, at: entry.at, files: { [path]: { before: last.files[path]!.before, after: entry.files[path]!.after } } };
          return flags([...s.past.slice(0, -1), merged], []);
        }
        return flags([...s.past, entry].slice(-LIMIT), []);
      });
    },
    undo() {
      const entry = get().past.at(-1);
      if (!entry) return;
      restore(entry, "before");
      set((s) => flags(s.past.slice(0, -1), [...s.future, entry]));
    },
    redo() {
      const entry = get().future.at(-1);
      if (!entry) return;
      restore(entry, "after");
      set((s) => flags([...s.past, entry], s.future.slice(0, -1)));
    },
    clearHistory() {
      set(flags([], []));
    },
  };
};
```

`edit-slice.ts`:

```ts
import type { EditorLayout } from "@shared/edit-types";

import type { EditSlice, SliceCreator } from "./types";

const EMPTY_LAYOUT: EditorLayout = { graph: {} };

export const createEditSlice: SliceCreator<EditSlice> = (set, get, api) => ({
  editError: null,
  graphLayout: EMPTY_LAYOUT,

  async edit(ops) {
    const sent = Object.values(get().buffers).map(({ path, source }) => ({ path, source }));
    let applied;
    try {
      applied = await api.edit.apply(sent, ops);
    } catch (e) {
      set({ editError: e instanceof Error ? e.message : String(e) });
      return null;
    }
    const buffers = get().buffers;
    if (sent.some((f) => buffers[f.path]?.source !== f.source)) return null;
    const files: Record<string, { before: string; after: string }> = {};
    set((s) => {
      const next = { ...s.buffers };
      for (const file of applied.files) {
        const buffer = next[file.path];
        if (!buffer) continue;
        files[file.path] = { before: buffer.source, after: file.source };
        next[file.path] = { ...buffer, source: file.source };
      }
      return { buffers: next, editError: null };
    });
    if (Object.keys(files).length > 0) get().record({ files, kind: "visual", at: Date.now() });
    get().scheduleAnalyze();
    return applied.focus;
  },

  async loadGraphLayout() {
    set({ graphLayout: await api.edit.readLayout().catch(() => EMPTY_LAYOUT) });
  },

  setNodePosition(label, x, y) {
    const graphLayout = { graph: { ...get().graphLayout.graph, [label]: { x, y } } };
    set({ graphLayout });
    void api.edit.writeLayout(graphLayout);
  },

  resetGraphLayout() {
    set({ graphLayout: EMPTY_LAYOUT });
    void api.edit.writeLayout(EMPTY_LAYOUT);
  },
});
```

`project-slice.ts`:
- `setSource(path, source)` records a coalesced code entry after updating the buffer: `get().record({ files: { [path]: { before: current.source, after: source } }, kind: "code", at: Date.now() }, true)`.
- `openProject` calls `get().clearHistory()` and `await get().loadGraphLayout()` after `set(...)`.

`index.ts`: compose `createHistorySlice` and `createEditSlice` into the store.

`editor-view.tsx`:
- Monaco mirrors buffer changes with `pushEditOperations` while `mirroring` is true, which stops `setSource` from recording them. Keep that.
- In the editor creation block, route undo and redo to the store so one history serves both views:

```ts
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyZ, () => appStore().getState().undo());
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyZ, () => appStore().getState().redo());
```

Callers:
- `character-manager.tsx` calls `edit([{ op: "setCharacter", path, id, name, color: color ?? null }])` in place of `saveCharacter`. `path` is the file holding the existing declaration, or the first script file sorted by path. Show `editError` inline when it returns `null`.
- `asset-drag.ts` and the preview drop handler build the statement through `insertAtCursor` as before. Move `stagePosition` and `statementForAsset` from `edits.ts` into `asset-drag.ts`. Visual-editor drops come in Task 15.
- The `validateCharacter` and `CHARACTER_ID` exports move from `edits.ts` into `character-manager.tsx`, and their existing tests move into `character-manager.test.ts` next to it.
- `story-graph.tsx`'s `onConnect` calls `edit([{ op: "connect", from, to, choice: null }])`. Task 11 replaces this with the connect dialog.
- Delete `ProjectEdit`, `EditResult` and `applyEdit` from `shared/types.ts`, `preload`, `main/index.ts` and `main/project.ts`, then delete `shared/edits.ts` and `shared/edits.test.ts`.

- [ ] **Step 4: Run all editor checks**

Run: `bun run test && bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A apps/editor
git commit -m "Route visual edits through one action with a shared undo history"
```

---

### Task 8: Preview seek and sprite hit-testing

**Files:**
- Modify: `crates/kairos-player/src/game.rs`, `crates/kairos-player/src/player.rs`, `crates/kairos-player/src/view.rs`, `crates/kairos-player/src/web.rs`
- Test: unit tests in `game.rs` and `view.rs`

**Interfaces:**
- Produces:
  - `Game::seek_to(&mut self, start: usize, target: usize)`, which plays from `start` and stops at the first interaction at or after `target`. At menus it picks the choice whose body contains `target`, or the first choice. After 500 interactions it falls back to `start_at(target)`.
  - `Player::preview_at(&mut self, pc: usize)`, which finds the containing label with `program.label_at(pc)` and calls `game.seek_to(label_start, pc)`.
  - `pub(crate) fn sprite_rects(stage, image, size) -> Vec<(String, Rect)>` in `view.rs`, used by drawing and by `Player::sprite_at(x, y) -> Option<String>`.
  - wasm: `previewAt(path: string, line: number)` and `spriteAt(x: number, y: number): string | undefined`. Coordinates are canvas pixels.

- [ ] **Step 1: Write the failing tests** (in `game.rs` tests)

```rust
#[test]
fn seek_plays_up_to_a_line_keeping_the_stage_built_before_it() {
    let source = "label start:\n    scene bg room\n    show e happy\n    \"One\"\n    menu:\n        \"A\":\n            \"In A\"\n        \"B\":\n            show e sad\n            \"In B\"\n";
    let project = kairos_project::Project::from_sources("name = \"t\"", [(PathBuf::from("script/a.kai"), source.into())]).unwrap();
    let mut game = Game::new(project.program.clone()).unwrap();
    let program = project.program.clone();
    let target = program.pc_at(0, 10).unwrap();
    game.seek_to(program.labels["start"], target);
    assert_eq!(text(&game), "In B");
    assert_eq!(game.stage().background.as_deref(), Some(&["bg".to_string(), "room".to_string()][..]));
    assert_eq!(game.stage().sprites[0].attrs, ["sad"]);
}
```

In `view.rs` tests:

```rust
#[test]
fn sprite_rects_follow_draw_order() {
    let stage = kairos_vm::Stage {
        background: None,
        sprites: vec![
            kairos_vm::Sprite { tag: "e".into(), attrs: vec!["happy".into()], at: Some("left".into()) },
            kairos_vm::Sprite { tag: "k".into(), attrs: vec![], at: Some("right".into()) },
        ],
    };
    let rects = sprite_rects(&stage, &|_: &str| ImageInfo::Ready(400, 800), (1280.0, 720.0));
    assert_eq!(rects.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["e", "k"]);
    assert!(rects[0].1.x < rects[1].1.x);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p kairos-player --lib`
Expected: compile errors (`seek_to`, `sprite_rects`).

- [ ] **Step 3: Implement**

`game.rs`:

```rust
/// Plays from `start` until the first interaction at or after `target`, so the editor can show
/// a scene's state at any line. Menus pick the choice that leads toward `target`.
pub fn seek_to(&mut self, start: usize, target: usize) {
    self.start_at(start);
    self.settings.text_speed = Settings::INSTANT;
    for _ in 0..500 {
        let at = self.vm.last_pc();
        match &self.shown {
            Shown::Say { .. } | Shown::Pause { .. } if at < target => self.step(),
            Shown::Menu { .. } if at < target => {
                let kairos_vm::Instr::Menu { choices, .. } = &self.program.instrs[at] else { break };
                let pick = choices.iter().rposition(|c| c.target <= target).unwrap_or(0);
                self.choose(pick);
            }
            _ => return,
        }
    }
    self.start_at(target);
}
```

`seek_to` overrides `text_speed` so the shown line appears instantly. Save and restore the player's previous speed around the loop: `let speed = self.settings.text_speed;` before it, and `self.settings.text_speed = speed;` before each `return` and at the end. `choose` takes a visible index, and guarded choices may hide some. Use `rposition` over the visible targets the VM returned. If a guard hides the wanted choice, `choose` falls back to whatever index is visible, which is acceptable for a preview.

`view.rs`: extract the sprite loop's rectangle math:

```rust
pub(crate) fn sprite_rects(stage: &Stage, image: &dyn Fn(&str) -> ImageInfo, (w, h): (f32, f32)) -> Vec<(String, Rect)> {
    stage
        .sprites
        .iter()
        .map(|sprite| {
            let mut words = vec![sprite.tag.clone()];
            words.extend(sprite.attrs.iter().cloned());
            let at = sprite.at.as_deref();
            let rect = match image(&image_name(&words)) {
                ImageInfo::Ready(iw, ih) => sprite_rect(at, (iw as f32, ih as f32), (w, h)),
                _ => {
                    let (pw, ph) = (h * 0.34, h * 0.78);
                    Rect::new((w - pw) * xalign(at), h - ph, pw, ph)
                }
            };
            (sprite.tag.clone(), rect)
        })
        .collect()
}
```

Use it inside the existing drawing loop for the rectangles, keeping the per-`ImageInfo` drawing as is. The `image` field on `ViewInput` has whatever closure type it already uses; adapt the parameter type to match it, for example `&dyn Fn(&str) -> ImageInfo`.

`player.rs`:

```rust
pub fn preview_at(&mut self, pc: usize) {
    self.in_splashscreen = false;
    let program = self.game.program().clone();
    let start = program.label_at(pc).and_then(|l| program.labels.get(l).copied()).unwrap_or(pc);
    self.game.seek_to(start, pc);
}

pub fn sprite_at(&self, x: f32, y: f32) -> Option<String> {
    let images = &self.images;
    let lookup = |name: &str| images.get(name).copied().unwrap_or(ImageInfo::Loading);
    let size = self.renderer.size();
    view::sprite_rects(self.game.stage(), &lookup, (size.0 as f32, size.1 as f32))
        .into_iter()
        .rev()
        .find(|(_, r)| r.contains(x, y))
        .map(|(tag, _)| tag)
}
```

If `Game` has no `program()` accessor, add `pub fn program(&self) -> &Arc<Program> { &self.program }`. If `Renderer` has no `size()`, use the size `Player` stores from `resize`, or add a `size: (u32, u32)` field set in `resize`.

`web.rs`:

```rust
#[wasm_bindgen(js_name = previewAt)]
pub fn preview_at(&self, path: &str, line: u32) -> Result<(), JsError> {
    let mut inner = self.inner.borrow_mut();
    let pc = {
        let loaded = inner.loaded.as_ref().ok_or_else(|| error("call load() before previewAt()"))?;
        loaded.project.pc_at(Path::new(path), line as usize).ok_or_else(|| error(format!("{path}:{line}: no statement here")))?
    };
    let player = inner.player.as_mut().ok_or_else(|| error("call ready() before previewAt()"))?;
    player.preview_at(pc);
    inner.last_debug_state = None;
    Ok(())
}

#[wasm_bindgen(js_name = spriteAt)]
pub fn sprite_at(&self, x: f32, y: f32) -> Option<String> {
    self.inner.borrow().player.as_ref().and_then(|p| p.sprite_at(x, y))
}
```

- [ ] **Step 4: Run the tests and the wasm build**

Run: `cargo test -p kairos-player --lib && cargo clippy -p kairos-player --target wasm32-unknown-unknown && bash scripts/build-wasm.sh`
Expected: tests pass, clippy clean, `built .../pkg`.

- [ ] **Step 5: Commit**

```bash
git add crates/kairos-player
git commit -m "Let the preview seek to a line and report the character under a point"
```

---

### Task 9: Graph selection, positions and tidy up

**Files:**
- Modify: `apps/editor/src/renderer/src/features/graph/story-graph.tsx`, `apps/editor/src/renderer/src/features/graph/layout.ts`
- Create: `apps/editor/src/renderer/src/features/graph/graph-model.ts`
- Test: `apps/editor/src/renderer/src/features/graph/graph-model.test.ts`

**Interfaces:**
- Consumes: `graphLayout`, `setNodePosition` and `resetGraphLayout` (Task 7).
- Produces:
  - `positionsFor(graph: StoryGraph, saved: EditorLayout["graph"]): Map<string, { x: number; y: number }>`, which keeps saved positions and lays out only unsaved nodes.
  - `ghostTargets(graph: StoryGraph, edgesTo: { from: string; to: string }[]): string[]`, the missing labels that arrows point at.

The analysis graph drops edges to unknown labels, so ghosts need the raw targets. Extend `StoryGraph` in `kairos-napi` (`analysis.rs`) with `missing: Vec<GraphEdge>`: the edges whose `to` is unknown, collected before `retain`. Add a napi test for it. TS `StoryGraph` gains `missing: GraphEdge[]`.

- [ ] **Step 1: Write the failing tests**

`graph-model.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { positionsFor } from "./graph-model";

const graph = {
  nodes: [
    { label: "start", path: "a.kai", line: 1 },
    { label: "end", path: "a.kai", line: 5 },
  ],
  edges: [{ from: "start", to: "end", kind: "jump" as const, text: null }],
  missing: [],
};

describe("graph positions", () => {
  it("keeps saved positions and lays out the rest", () => {
    const p = positionsFor(graph, { start: { x: 500, y: 40 } });
    expect(p.get("start")).toEqual({ x: 500, y: 40 });
    expect(p.get("end")).toBeDefined();
    expect(p.get("end")).not.toEqual({ x: 500, y: 40 });
  });
});
```

Rust (`crates/kairos-napi/src/analysis.rs` tests):

```rust
#[test]
fn missing_targets_are_reported_separately() {
    let g = story_graph(&[file("a.kai", "label a:\n    jump nowhere\n")]);
    assert!(g.edges.is_empty());
    assert_eq!(g.missing, [edge("a", "nowhere", "jump", None)]);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `bun run --cwd apps/editor test src/renderer/src/features/graph && cargo test -p kairos-napi`
Expected: FAIL.

- [ ] **Step 3: Implement**

Rust: in `graph_parsed`, before `edges.retain(...)`, do `let missing: Vec<GraphEdge> = edges.iter().filter(|e| !known.contains(e.to.as_str())).cloned().collect();`, add `missing` to `StoryGraph`, and dedupe it the same way. Rebuild with `bun run build:napi`.

`graph-model.ts`:

```ts
import type { EditorLayout } from "@shared/edit-types";
import type { StoryGraph } from "@shared/types";

import { layoutGraph } from "./layout";

export function positionsFor(graph: StoryGraph, saved: EditorLayout["graph"]) {
  const auto = layoutGraph(graph);
  return new Map(graph.nodes.map((n) => [n.label, saved[n.label] ?? auto.get(n.label) ?? { x: 0, y: 0 }]));
}
```

`story-graph.tsx`:
- Use `positionsFor(graph, graphLayout.graph)`.
- Pass `selectionOnDrag`, `multiSelectionKeyCode="Shift"` and `onNodeDragStop={(_, node) => setNodePosition(node.id, node.position.x, node.position.y)}`. For multi-node drags, use `onSelectionDragStop` and save every node.
- Add ghost nodes for `graph.missing` targets: type `"ghost"`, a dashed border with the text "Missing scene: {label}", and an edge styled with `stroke: "var(--destructive)"`. Place each ghost 160 px below its first source.
- Add a toolbar button "Tidy up" (lucide `Wand2`) that calls `resetGraphLayout()` and then `fitView()`.

- [ ] **Step 4: Run all editor checks**

Run: `bun run test && bun run typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/editor crates/kairos-napi
git commit -m "Remember graph positions, select several scenes and show missing scenes"
```

---

### Task 10: Scene actions on the graph

**Files:**
- Create: `apps/editor/src/renderer/src/components/ui/dialog.tsx`, `apps/editor/src/renderer/src/features/graph/scene-actions.tsx`, `apps/editor/src/renderer/src/features/graph/scene-actions.test.ts`
- Modify: `story-graph.tsx`, `features/palette/command-palette.tsx`
- Add dependency: `bun add -d @radix-ui/react-context-menu --cwd apps/editor`

**Interfaces:**
- Consumes: `edit`, `api.edit.references`, `api.edit.labelFromName`, `api.edit.setStart`.
- Produces:
  - "Open" goes to the scene's line in the code view through `goToLabel`. Task 12 switches it to the scene tab.
  - `sceneActions(store)`, which returns `{ add, rename, duplicate, remove, setStart }`. Each is an async function the toolbar, the context menu, the keyboard and the palette all share.
  - `deleteSummary(refs: Reference[]): string`, for example "3 choices and 1 jump lead here".

- [ ] **Step 1: Write the failing test** (`scene-actions.test.ts`)

```ts
import { describe, expect, it } from "vitest";

import { deleteSummary } from "./scene-actions";

describe("delete confirmation", () => {
  it("counts links in plain words", () => {
    expect(deleteSummary([])).toBe("Nothing else leads here.");
    expect(
      deleteSummary([
        { path: "a", line: 1, kind: "jump", choice: "Go" },
        { path: "a", line: 2, kind: "jump", choice: "Run" },
        { path: "a", line: 3, kind: "jump", choice: null },
        { path: "a", line: 4, kind: "call", choice: null },
      ]),
    ).toBe("2 choices, 1 jump and 1 call lead here.");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `bun run --cwd apps/editor test src/renderer/src/features/graph/scene-actions.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

`components/ui/dialog.tsx`, a minimal Radix wrapper in the style of the existing `dropdown-menu.tsx`:

```tsx
import * as DialogPrimitive from "@radix-ui/react-dialog";
import type { ReactNode } from "react";

export function Dialog({ open, onOpenChange, title, children }: { open: boolean; onOpenChange(open: boolean): void; title: string; children: ReactNode }) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/40" />
        <DialogPrimitive.Content className="fixed top-1/2 left-1/2 z-50 w-[420px] -translate-x-1/2 -translate-y-1/2 space-y-3 rounded-lg border bg-popover p-4 shadow-lg">
          <DialogPrimitive.Title className="text-sm font-medium">{title}</DialogPrimitive.Title>
          {children}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
```

`scene-actions.tsx`:

```tsx
import type { Reference } from "@shared/edit-types";

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

export function deleteSummary(refs: Reference[]): string {
  if (refs.length === 0) return "Nothing else leads here.";
  const choices = refs.filter((r) => r.choice !== null).length;
  const jumps = refs.filter((r) => r.choice === null && r.kind === "jump").length;
  const calls = refs.filter((r) => r.choice === null && r.kind === "call").length;
  const parts = [
    choices && plural(choices, "choice"),
    jumps && plural(jumps, "jump"),
    calls && plural(calls, "call"),
  ].filter(Boolean) as string[];
  const list = parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}` : parts[0];
  return `${list} lead${refs.length === 1 ? "s" : ""} here.`;
}
```

The same file exports a `SceneDialogs` component and a `useSceneActions()` hook. The hook reads the store and returns:
- `add(after?: string, from?: string)`: open a name dialog, then `labelFromName(name, existing labels)`, then `edit([{ op: "addScene", path: <after's file or the first script>, name: label, after: after ?? null }, ...(from ? [{ op: "connect", from, to: label, choice: null }] : [])])`.
- `rename(label)`: open a name dialog, then `labelFromName`, then `edit([{ op: "renameScene", from: label, to }])`.
- `duplicate(label)`: `labelFromName(\`${label} copy\`, ...)`, then `duplicateScene`.
- `remove(labels: string[])`: for each label, fetch references and show one dialog listing every label with its `deleteSummary`. Its buttons are "Delete and remove links", "Delete only" and "Cancel". Then call `edit` with one `deleteScene` op per label.
- `setStart(label)`: `api.edit.setStart(label)`, then `refreshTree()`.

Each action shows `editError` with `toast.error` when `edit` returns `null`.

`story-graph.tsx`:
- **Selected-node toolbar.** Use `@xyflow/react`'s `NodeToolbar` with buttons Open, Rename, Duplicate and Delete. Open calls `goToLabel` for now.
- **Canvas context menu.** "New scene here", which calls `add` and then `setNodePosition` at the projected click point using `screenToFlowPosition`, and "Tidy up".
- **Node context menu.** Open, Rename, Duplicate, Set as start, Delete.
- **Handle dragged onto empty space.** `onConnectEnd`: when `connectionState.isValid` is false and the drop is on the pane, call `add(fromLabel's scene, fromLabel)` and place the node at the drop point.
- **Keyboard on the graph container.** Delete and Backspace call `remove(selected)`, F2 calls `rename`, Enter opens, Cmd+D calls `duplicate`. Cmd+Z and Cmd+Shift+Z call `undo` and `redo`.
- **"Add scene" button** in the graph toolbar (lucide `Plus`).

`command-palette.tsx`: add "Add scene", "Tidy up story flow", "Undo" and "Redo".

- [ ] **Step 4: Run all editor checks**

Run: `bun run test && bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/editor
git commit -m "Add, rename, duplicate and delete scenes from the story graph"
```

---

### Task 11: Arrow tools on the graph

**Files:**
- Modify: `story-graph.tsx`, `scene-actions.tsx`
- Test: `apps/editor/src/renderer/src/features/graph/scene-actions.test.ts`

**Interfaces:**
- Consumes: the `edit` ops `connect`, `removeLink`, `endScene` and `setChoiceText`.
- Produces `edgeOps(edge: GraphEdge, action: "delete" | "makeExplicit" | "endHere"): EditOp[]`, a pure function that's tested.

- [ ] **Step 1: Write the failing test**

```ts
import { edgeOps } from "./scene-actions";

it("maps arrow actions to edits", () => {
  expect(edgeOps({ from: "a", to: "b", kind: "choice", text: "Go" }, "delete")).toEqual([{ op: "removeLink", from: "a", to: "b", choice: "Go" }]);
  expect(edgeOps({ from: "a", to: "b", kind: "jump", text: null }, "delete")).toEqual([{ op: "removeLink", from: "a", to: "b", choice: null }]);
  expect(edgeOps({ from: "a", to: "b", kind: "fallthrough", text: null }, "makeExplicit")).toEqual([{ op: "connect", from: "a", to: "b", choice: null }]);
  expect(edgeOps({ from: "a", to: "b", kind: "fallthrough", text: null }, "endHere")).toEqual([{ op: "endScene", label: "a" }]);
  expect(edgeOps({ from: "a", to: "b", kind: "fallthrough", text: null }, "delete")).toEqual([{ op: "endScene", label: "a" }]);
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `bun run --cwd apps/editor test src/renderer/src/features/graph`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
export function edgeOps(edge: GraphEdge, action: "delete" | "makeExplicit" | "endHere"): EditOp[] {
  if (edge.kind === "fallthrough") {
    return action === "makeExplicit" ? [{ op: "connect", from: edge.from, to: edge.to, choice: null }] : [{ op: "endScene", label: edge.from }];
  }
  return [{ op: "removeLink", from: edge.from, to: edge.to, choice: edge.kind === "choice" ? (edge.text ?? null) : null }];
}
```

`story-graph.tsx`:
- Edges become selectable. Merged edges (several choices between the same pair) use the first edge's text; selecting one shows a toolbar of buttons rendered with `EdgeLabelRenderer` at the midpoint.
- Choice arrow: an editable text input for the label. Rename finds the option line through `api.edit.blocks(source, from)`: take the first `choice` block whose option text equals the old text and targets `to`. It then calls `edit([{ op: "setChoiceText", path, line, text }])`. Also a Delete button.
- Jump and call arrows: Delete.
- Fall-through arrows: "Make explicit" and "End scene here".
- `onConnect` opens a small dialog with "Go to" and "Add as a choice". The choice option includes a text field that defaults to the target scene name. Then call `edit([{ op: "connect", from, to, choice }])`.
- Delete and Backspace with an edge selected run `edgeOps(edge, "delete")`.

- [ ] **Step 4: Run all editor checks**

Run: `bun run test && bun run typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/editor
git commit -m "Edit, retarget and remove story arrows from the graph"
```

---

### Task 12: Scene tab with the block list

**Files:**
- Create: `apps/editor/src/renderer/src/features/scene/scene-view.tsx`, `features/scene/block-labels.ts`, `features/scene/block-labels.test.ts`, `features/scene/use-scene.ts`
- Modify: `features/workspace/workspace.tsx`, `store/tabs-slice.ts`, `store/types.ts`, `features/graph/story-graph.tsx` (Open and double-click now call `openScene`)
- Test: `block-labels.test.ts`, `store/store.test.ts`

**Interfaces:**
- Consumes: `api.edit.blocks`, the analysis diagnostics, the buffers.
- Produces:
  - `openScene(label)`, a tab with `id: "scene:<label>"`
  - `useScene(label) -> { path: string; scene: Scene | null; refresh(): void }`, which reloads whenever that file's buffer changes
  - `blockTitle(block: Block, characters: Record<string, string>): string` and `blockDiagnostics(block, diagnostics, path)`
  - `selectedBlock: { path: string; line: number } | null` and `selectBlock(sel)` in the store

- [ ] **Step 1: Write the failing tests**

`block-labels.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { blockDiagnostics, blockTitle } from "./block-labels";

const names = { e: "Eileen" };

describe("block titles", () => {
  it("uses plain words", () => {
    expect(blockTitle({ kind: "dialogue", character: "e", text: "Hi", line: 2, endLine: 2 }, names)).toBe("Eileen says");
    expect(blockTitle({ kind: "dialogue", character: null, text: "Hi", line: 2, endLine: 2 }, names)).toBe("Narrator");
    expect(blockTitle({ kind: "background", image: ["bg", "room"], transition: null, line: 2, endLine: 2 }, names)).toBe("Change background");
    expect(blockTitle({ kind: "show", image: ["e", "happy"], position: "left", transition: null, line: 2, endLine: 2 }, names)).toBe("Show Eileen");
    expect(blockTitle({ kind: "setVariable", name: "n", op: "add", value: "1", line: 2, endLine: 2 }, names)).toBe("Change n");
    expect(blockTitle({ kind: "code", text: "with fade", line: 2, endLine: 2 }, names)).toBe("Code");
  });

  it("attaches diagnostics to the block whose lines contain them", () => {
    const block = { kind: "code" as const, text: "x", line: 4, endLine: 6 };
    const diags = [
      { path: "a.kai", line: 5, message: "bad" },
      { path: "a.kai", line: 7, message: "other" },
      { path: "b.kai", line: 5, message: "elsewhere" },
    ];
    expect(blockDiagnostics(block, diags, "a.kai").map((d) => d.message)).toEqual(["bad"]);
  });
});
```

Store test:

```ts
it("opens a scene tab once per label", () => {
  store.getState().openScene("start");
  store.getState().openScene("start");
  expect(store.getState().tabs.filter((t) => t.id === "scene:start")).toHaveLength(1);
  expect(store.getState().activeId).toBe("scene:start");
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `bun run --cwd apps/editor test`
Expected: FAIL.

- [ ] **Step 3: Implement**

`store/types.ts`: `Tab` gains `| { id: \`scene:${string}\`; kind: "scene"; label: string }`. `TabsSlice` gains `openScene(label: string): void`, `selectedBlock` and `selectBlock(sel: { path: string; line: number } | null): void`.

`tabs-slice.ts`:

```ts
openScene(label) {
  const id = `scene:${label}` as const;
  set((s) => ({ tabs: s.tabs.some((t) => t.id === id) ? s.tabs : [...s.tabs, { id, kind: "scene", label }], activeId: id }));
},
selectedBlock: null,
selectBlock(selectedBlock) {
  set({ selectedBlock });
},
```

`block-labels.ts`:

```ts
import type { Block } from "@shared/edit-types";
import type { FileDiagnostic } from "@shared/types";

export function blockTitle(block: Block, names: Record<string, string>): string {
  const who = (id: string | null | undefined) => (id ? (names[id] ?? id) : "Narrator");
  switch (block.kind) {
    case "dialogue":
      return block.character ? `${who(block.character)} says` : "Narrator";
    case "background":
      return "Change background";
    case "show":
      return `Show ${who(block.image[0])}`;
    case "hide":
      return `Hide ${who(block.character)}`;
    case "audio":
      return block.channel === "music" ? "Play music" : block.channel === "voice" ? "Play voice" : "Play sound";
    case "stopAudio":
      return `Stop ${block.channel}`;
    case "pause":
      return "Pause";
    case "choice":
      return "Choice";
    case "goTo":
      return `Go to ${block.scene}`;
    case "call":
      return `Visit ${block.scene} and come back`;
    case "setVariable":
      return `Change ${block.name}`;
    case "if":
      return "Only if…";
    case "end":
      return "End scene";
    case "code":
      return "Code";
  }
}

export function blockDiagnostics(block: Block, diagnostics: FileDiagnostic[], path: string): FileDiagnostic[] {
  return diagnostics.filter((d) => d.path === path && d.line >= block.line && d.line <= block.endLine);
}
```

`use-scene.ts`:

```ts
import { useEffect, useState } from "react";

import type { Scene } from "@shared/edit-types";

import { useApp } from "@/store";

export function useScene(label: string) {
  const node = useApp((s) => s.analysis.graph.nodes.find((n) => n.label === label));
  const source = useApp((s) => (node ? s.buffers[node.path]?.source : undefined));
  const [scene, setScene] = useState<Scene | null>(null);
  useEffect(() => {
    if (source === undefined) return setScene(null);
    let live = true;
    void window.kairos.edit.blocks(source, label).then((s) => live && setScene(s));
    return () => {
      live = false;
    };
  }, [source, label]);
  return { path: node?.path ?? null, scene };
}
```

`scene-view.tsx` is read-only in this task. It renders a header (scene name, file and line, plus an "Open code" button that calls `goToLabel`) and a vertical list. Each block row shows an icon, `blockTitle`, a one-line summary (dialogue text, image name, scene name or code text) and a red count from `blockDiagnostics` with the messages in a tooltip. Choice and If rows render their options or branches indented, each with its nested blocks. Clicking a row calls `selectBlock({ path, line })`. An empty scene shows "Add the first line" as a muted call to action; it becomes a button in Task 13. Each row gets `data-testid="block"` and `aria-selected`.

`workspace.tsx`: render `<SceneView label={tab.label} />` for scene tabs, with title `tab.label` and lucide icon `Clapperboard`.

`story-graph.tsx`: double-click and the toolbar's Open call `openScene(label)`.

- [ ] **Step 4: Run all editor checks**

Run: `bun run test && bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/editor
git commit -m "Open scenes from the graph as a list of plain-language blocks"
```

---

### Task 13: Editing blocks

**Files:**
- Create: `features/scene/block-editors.tsx`, `features/scene/add-block-menu.tsx`, `features/scene/block-ops.ts`, `features/scene/block-ops.test.ts`
- Modify: `features/scene/scene-view.tsx`

**Interfaces:**
- Consumes: `edit`, `useScene`, `selectBlock`, the project's characters (`analysis.outlines`), assets (`project.assets`), sprite layers, and variables (the names in `default`/`define`, from outline additions or a regex scan of buffers for `^(default|define)\s+(\w+)`).
- Produces (pure and tested):
  - `insertOp(path, at: Place, block: BlockKind): EditOp`
  - `fieldOp(path, block: Block, patch: Partial<BlockKind>): EditOp`. For leaf blocks this is `replace`. For a choice option's text it's `setChoiceText`; for its condition, `setChoiceCondition`; for a branch condition, `setBranchCondition`.
  - `defaultBlock(kind, context): BlockKind`, which supplies new-block defaults. The first character, the first background, and so on.
  - `BLOCK_MENU: { kind: BlockKind["kind"]; label: string; keywords: string[] }[]`

- [ ] **Step 1: Write the failing tests**

```ts
import { describe, expect, it } from "vitest";

import { defaultBlock, fieldOp, insertOp } from "./block-ops";

const ctx = { characters: ["e"], backgrounds: [["bg", "room"]], music: ["theme.ogg"], scenes: ["start", "end"] };

describe("block ops", () => {
  it("builds replace ops for leaf blocks with the patched fields", () => {
    const block = { kind: "dialogue" as const, character: "e", text: "Hi", line: 3, endLine: 3 };
    expect(fieldOp("a.kai", block, { text: "Hello" })).toEqual({
      op: "replace",
      path: "a.kai",
      line: 3,
      block: { kind: "dialogue", character: "e", text: "Hello" },
    });
  });

  it("inserts after a block or at the start of a body", () => {
    expect(insertOp("a.kai", { at: "after", line: 3 }, defaultBlock("dialogue", ctx))).toEqual({
      op: "insert",
      path: "a.kai",
      place: { at: "after", line: 3 },
      block: { kind: "dialogue", character: "e", text: "" },
    });
  });

  it("gives every block kind a valid default", () => {
    expect(defaultBlock("background", ctx)).toEqual({ kind: "background", image: ["bg", "room"], transition: null });
    expect(defaultBlock("choice", ctx)).toEqual({ kind: "choice", options: [{ line: 0, text: "Option", condition: null, blocks: [] }] });
    expect(defaultBlock("goTo", ctx)).toEqual({ kind: "goTo", scene: "start" });
    expect(defaultBlock("setVariable", ctx)).toEqual({ kind: "setVariable", name: "points", op: "add", value: "1" });
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `bun run --cwd apps/editor test src/renderer/src/features/scene`
Expected: FAIL.

- [ ] **Step 3: Implement**

`block-ops.ts`:

```ts
import type { Block, BlockKind, EditOp, Place } from "@shared/edit-types";

export interface BlockContext {
  characters: string[];
  backgrounds: string[][];
  music: string[];
  scenes: string[];
}

const strip = (block: Block): BlockKind => {
  const { line: _line, endLine: _end, ...kind } = block;
  return kind as BlockKind;
};

export const insertOp = (path: string, place: Place, block: BlockKind): EditOp => ({ op: "insert", path, place, block });

export function fieldOp(path: string, block: Block, patch: Partial<BlockKind>): EditOp {
  return { op: "replace", path, line: block.line, block: { ...strip(block), ...patch } as BlockKind };
}

export function defaultBlock(kind: BlockKind["kind"], ctx: BlockContext): BlockKind {
  switch (kind) {
    case "dialogue":
      return { kind, character: ctx.characters[0] ?? null, text: "" };
    case "background":
      return { kind, image: ctx.backgrounds[0] ?? ["bg", "black"], transition: null };
    case "show":
      return { kind, image: [ctx.characters[0] ?? "someone"], position: "center", transition: null };
    case "hide":
      return { kind, character: ctx.characters[0] ?? "someone", transition: null };
    case "audio":
      return { kind, channel: "music", asset: ctx.music[0] ?? "", looped: true, fadeIn: null };
    case "stopAudio":
      return { kind, channel: "music", fadeOut: 1 };
    case "pause":
      return { kind, seconds: 1 };
    case "choice":
      return { kind, options: [{ line: 0, text: "Option", condition: null, blocks: [] }] };
    case "goTo":
    case "call":
      return { kind, scene: ctx.scenes[0] ?? "start" };
    case "setVariable":
      return { kind, name: "points", op: "add", value: "1" };
    case "if":
      return { kind, branches: [{ line: 0, condition: "points > 0", blocks: [] }] };
    case "end":
      return { kind };
    case "code":
      return { kind, text: "pass" };
  }
}

export const BLOCK_MENU: { kind: BlockKind["kind"]; label: string; keywords: string[] }[] = [
  { kind: "dialogue", label: "Character says…", keywords: ["line", "talk", "dialogue", "narration"] },
  { kind: "background", label: "Change background", keywords: ["scene", "bg", "location"] },
  { kind: "show", label: "Show character", keywords: ["sprite", "enter", "expression"] },
  { kind: "hide", label: "Hide character", keywords: ["leave", "exit"] },
  { kind: "audio", label: "Play music or sound", keywords: ["music", "sound", "voice", "audio"] },
  { kind: "stopAudio", label: "Stop audio", keywords: ["silence", "fade"] },
  { kind: "pause", label: "Pause", keywords: ["wait"] },
  { kind: "choice", label: "Choice", keywords: ["menu", "branch", "option"] },
  { kind: "goTo", label: "Go to scene", keywords: ["jump", "next"] },
  { kind: "call", label: "Visit scene and come back", keywords: ["call"] },
  { kind: "setVariable", label: "Change a variable", keywords: ["points", "affection", "flag"] },
  { kind: "if", label: "Only if…", keywords: ["condition", "if", "check"] },
  { kind: "end", label: "End scene", keywords: ["return", "stop"] },
  { kind: "code", label: "Code", keywords: ["script", "advanced"] },
];
```

`block-editors.tsx` exports `BlockEditor({ block, path, ctx })`, which switches on `block.kind`. Each field is a small input bound to local state, and every change calls a debounced `commit(patch)` that runs `edit([fieldOp(path, block, patch)])`. The debounce is 300 ms and is created once per block with `useRef`. Fields per kind:

- **dialogue:** a character `<select>` with Narrator first, and a `<textarea>` that grows. Typing `[` shows a popover listing variables; picking one inserts `[name]`.
- **background:** an image `<select>` of background assets with a thumbnail (`assetUrl`), and a transition `<select>` (none, fade, dissolve).
- **show:** character, an expression `<select>` (layer attributes from `project.spriteLayers[tag].groups`, or the image stems starting with `tag `), position (left, center, right) and transition.
- **hide:** character and transition.
- **audio:** a channel toggle, an asset `<select>`, a loop checkbox (music only) and a fade-in number.
- **stopAudio:** channel and fade-out.
- **pause:** seconds, or a "until click" checkbox that sets `seconds: null`.
- **goTo / call:** a scene `<select>`.
- **setVariable:** a variable combobox (existing names plus free text), an operation select (set, add, subtract, toggle) and a value input. The value is hidden for toggle.
- **choice:** per option, a text input that commits `setChoiceText` with `line: option.line`, an optional "only if" input that commits `setChoiceCondition`, and the nested block list (recursive). "+ Add option" commits `addChoiceOption`.
- **if:** per branch, a condition input (hidden for "otherwise") that commits `setBranchCondition`, and the nested list. "+ Add otherwise" or "+ Add else-if" commits `addBranch`.
- **code:** a monospace `<textarea>` that commits `replace` with `{ kind: "code", text }`. A `WouldBreak` rejection shows `editError` under the field and keeps the local text so the user can fix it.

`add-block-menu.tsx`: a `cmdk` popover filtered over `BLOCK_MENU` (label and keywords). Picking an item calls `edit([insertOp(path, place, defaultBlock(kind, ctx))])` and selects the returned focus line.

`scene-view.tsx` becomes editable:
- Each row expands to its editor when selected.
- A thin "+" between rows (and at the start of each body) opens the add menu with `{ at: "after", line }` or `{ at: "start", header }`.
- Each row has a menu with Duplicate (`insertOp` after itself with `strip(block)`) and Delete (`{ op: "delete", path, line }`).
- Keyboard on the list: ArrowUp and ArrowDown move the selection, Enter focuses the first field, Escape blurs. Cmd+Enter inserts a dialogue block after the selection with the same character. Delete and Backspace delete the selection when focus is on the row rather than a field.
- After any edit, `selectBlock` uses the returned focus line so the selection follows the edited block.

- [ ] **Step 4: Run all editor checks**

Run: `bun run test && bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/editor
git commit -m "Edit, add, duplicate and delete scene blocks without writing code"
```

---

### Task 14: Reordering blocks by drag

**Files:**
- Create: `features/scene/drop-target.ts`, `features/scene/drop-target.test.ts`
- Modify: `features/scene/scene-view.tsx`

**Interfaces:**
- Produces `dropPlace(rows: { line: number; endLine: number; header?: number; depth: number }[], pointerY: number, rects: DOMRect[]): Place | null`, a pure function. Pointer above a row's midpoint means "before" it, which becomes `after` the previous sibling or `start` of the parent body. Below the midpoint means `after` the row.

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";

import { dropPlace } from "./drop-target";

const rect = (top: number) => ({ top, bottom: top + 20, height: 20 }) as DOMRect;

describe("drop target", () => {
  const rows = [
    { line: 2, endLine: 2, depth: 0, parentHeader: 1 },
    { line: 3, endLine: 3, depth: 0, parentHeader: 1 },
  ];
  it("drops before the first row at the start of the body", () => {
    expect(dropPlace(rows, 2, [rect(0), rect(20)])).toEqual({ at: "start", header: 1 });
  });
  it("drops after a row when below its middle", () => {
    expect(dropPlace(rows, 15, [rect(0), rect(20)])).toEqual({ at: "after", line: 2 });
    expect(dropPlace(rows, 25, [rect(0), rect(20)])).toEqual({ at: "after", line: 2 });
    expect(dropPlace(rows, 35, [rect(0), rect(20)])).toEqual({ at: "after", line: 3 });
  });
});
```

The row type carries `parentHeader`, the header line of the body the row sits in. Update the interface line above to `{ line; endLine; depth; parentHeader }[]`.

- [ ] **Step 2: Run it to verify it fails**

Run: `bun run --cwd apps/editor test src/renderer/src/features/scene/drop-target.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
import type { Place } from "@shared/edit-types";

export interface DropRow {
  line: number;
  endLine: number;
  depth: number;
  parentHeader: number;
}

export function dropPlace(rows: DropRow[], y: number, rects: DOMRect[]): Place | null {
  for (let i = 0; i < rows.length; i++) {
    const r = rects[i]!;
    if (y > r.bottom) continue;
    const row = rows[i]!;
    if (y > r.top + r.height / 2) return { at: "after", line: row.line };
    const prev = rows.slice(0, i).findLast((p) => p.parentHeader === row.parentHeader);
    return prev ? { at: "after", line: prev.line } : { at: "start", header: row.parentHeader };
  }
  const last = rows.at(-1);
  return last ? { at: "after", line: last.line } : null;
}
```

`scene-view.tsx`:
- Each row is `draggable` with `dataTransfer.setData("application/x-kairos-block", String(line))`.
- The list container collects the flattened rows (including nested ones, with `parentHeader`) and their rects on `dragover`, shows a 2 px insertion line at the computed place, and on `drop` calls `edit([{ op: "move", path, line: dragged, place }])`. A rejection such as "moving inside itself" shows as a toast.
- Asset drops (`application/x-kairos-asset`, from the asset manager) use the same placement and insert the block that `assetToBlock` returns (Task 15).

- [ ] **Step 4: Run all editor checks**

Run: `bun run test && bun run typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/editor
git commit -m "Reorder scene blocks by dragging, including into choices"
```

---

### Task 15: Stage integration

**Files:**
- Create: `features/scene/stage-drop.ts`, `features/scene/stage-drop.test.ts`
- Modify: `features/preview/player.ts` (the `PlayerModule` type gains `previewAt` and `spriteAt`), `features/preview/preview-pane.tsx`, `features/scene/scene-view.tsx`, `store/types.ts` and `store/index.ts` (a `previewAt` request like `playRequest`)

**Interfaces:**
- Consumes: wasm `previewAt` and `spriteAt` (Task 8), `selectedBlock`, `edit`.
- Produces:
  - `assetToBlock(asset: AssetEntry, xFraction: number): BlockKind | null`
  - `showOpFor(blocks: Block[], selectedLine: number, character: string, image: string[], position: string, path: string): EditOp`, which replaces the latest Show block for that character at or before the selection, or inserts one after the selection
  - `showBlockAt(blocks: Block[], selectedLine: number, character: string): Block | null`
  - Store: `previewAt(path, line)`, which sets `playRequest` with `mode: "seek"`

- [ ] **Step 1: Write the failing tests**

```ts
import { describe, expect, it } from "vitest";

import { assetToBlock, showBlockAt, showOpFor } from "./stage-drop";

const blocks = [
  { kind: "background" as const, image: ["bg", "room"], transition: null, line: 2, endLine: 2 },
  { kind: "show" as const, image: ["e", "happy"], position: "left", transition: null, line: 3, endLine: 3 },
  { kind: "dialogue" as const, character: "e", text: "Hi", line: 4, endLine: 4 },
];

describe("stage drops", () => {
  it("turns assets into blocks", () => {
    expect(assetToBlock({ path: "assets/images/bg/bg beach.png", kind: "image", name: "bg beach", category: "background" }, 0.5)).toEqual({ kind: "background", image: ["bg", "beach"], transition: null });
    expect(assetToBlock({ path: "assets/images/sprites/e/e sad.png", kind: "image", name: "e sad", category: "sprite", characterId: "e" }, 0.9)).toEqual({ kind: "show", image: ["e", "sad"], position: "right", transition: null });
    expect(assetToBlock({ path: "assets/audio/music/theme.ogg", kind: "audio", name: "music/theme.ogg", category: "music" }, 0)).toEqual({ kind: "audio", channel: "music", asset: "music/theme.ogg", looped: true, fadeIn: null });
  });

  it("updates the character's latest show block instead of adding another", () => {
    expect(showOpFor(blocks, 4, "e", ["e", "sad"], "right", "a.kai")).toEqual({ op: "replace", path: "a.kai", line: 3, block: { kind: "show", image: ["e", "sad"], position: "right", transition: null } });
    expect(showOpFor(blocks, 4, "k", ["k"], "center", "a.kai")).toEqual({ op: "insert", path: "a.kai", place: { at: "after", line: 4 }, block: { kind: "show", image: ["k"], position: "center", transition: null } });
  });

  it("finds the show block that placed a clicked character", () => {
    expect(showBlockAt(blocks, 4, "e")?.line).toBe(3);
    expect(showBlockAt(blocks, 2, "e")).toBeNull();
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `bun run --cwd apps/editor test src/renderer/src/features/scene/stage-drop.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
import type { Block, BlockKind, EditOp } from "@shared/edit-types";
import type { AssetEntry } from "@shared/types";

const position = (x: number) => (x < 1 / 3 ? "left" : x < 2 / 3 ? "center" : "right");

export function assetToBlock(asset: AssetEntry, x: number): BlockKind | null {
  if (asset.kind === "image") {
    const image = asset.name.split(/\s+/);
    return asset.category === "background" || image[0] === "bg"
      ? { kind: "background", image, transition: null }
      : { kind: "show", image, position: position(x), transition: null };
  }
  if (asset.kind === "audio") {
    const channel = asset.category === "sound" ? "sound" : asset.category === "voice" ? "voice" : "music";
    return { kind: "audio", channel, asset: asset.name, looped: channel === "music", fadeIn: null };
  }
  return null;
}

export function showBlockAt(blocks: Block[], selectedLine: number, character: string): Block | null {
  return blocks.findLast((b) => b.line <= selectedLine && b.kind === "show" && b.image[0] === character) ?? null;
}

export function showOpFor(blocks: Block[], selectedLine: number, character: string, image: string[], pos: string, path: string): EditOp {
  const block: BlockKind = { kind: "show", image, position: pos, transition: null };
  const existing = showBlockAt(blocks, selectedLine, character);
  return existing ? { op: "replace", path, line: existing.line, block } : { op: "insert", path, place: { at: "after", line: selectedLine }, block };
}
```

`showBlockAt` only searches the top-level blocks of the scene. Pass the flattened block list when the selection is inside a choice.

Wiring:
- **Store.** `previewAt(path, line)` sets `playRequest: { path, line, nonce, mode: "seek" }`. `preview-pane.tsx` calls `player.previewAt(path, line)` when `mode === "seek"`, and `playFromLine` otherwise.
- **`scene-view.tsx`.** When `selectedBlock` changes and the selection is in this scene, call `previewAt(path, line)`, debounced to 150 ms.
- **`preview-pane.tsx`, when the active tab is a scene.**
  - Drops on the stage go through `assetToBlock`. Backgrounds and audio insert after the selection (or after the scene's last block when nothing is selected). Sprites use `showOpFor`. Then `selectBlock(focus)`.
  - A click on the canvas converts to canvas pixels: `x = e.nativeEvent.offsetX * canvas.width / canvas.clientWidth`, and likewise for y. If `player.spriteAt(x, y)` returns a tag, call `selectBlock` on `showBlockAt(...)` and stop the click from advancing the game by calling `preventDefault` and `stopPropagation` in the capture phase.
  - With a file tab active, the existing drop-to-insert-at-cursor behavior stays.

- [ ] **Step 4: Run all editor checks and the wasm build**

Run: `bash scripts/build-wasm.sh && bun run test && bun run typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/editor
git commit -m "Show each block's moment on the stage and edit characters by dropping and clicking"
```

---

### Task 16: Beginner flow end to end and docs

**Files:**
- Create: `apps/editor/e2e/visual-editor.spec.ts`
- Modify: `README.md` (the Editor section), `docs/roadmap.md` (item 3 split into parts 1–3)

**Interfaces:**
- Consumes everything above. Selectors: `data-testid="graph-node"` and `data-testid="block"`, plus role and label queries for buttons and inputs.

- [ ] **Step 1: Write the e2e test**

```ts
import { cp, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { _electron as electron, expect, test } from "@playwright/test";

const editorDir = resolve(import.meta.dirname, "..");
const example = resolve(editorDir, "../../examples/hello-kairos");

test("a beginner builds a branching scene without the code view", async () => {
  const work = await mkdtemp(join(tmpdir(), "kairos-visual-"));
  const project = join(work, "story");
  await cp(example, project, { recursive: true });
  const app = await electron.launch({ args: [editorDir, `--project=${project}`], env: { ...process.env, KAIROS_USER_DATA: join(work, "data") } });
  const page = await app.firstWindow();
  const mod = process.platform === "darwin" ? "Meta" : "Control";

  await page.getByRole("tab", { name: /Story flow/ }).click();
  await page.getByRole("button", { name: "Add scene" }).click();
  await page.getByLabel("Scene name").fill("Beach Day");
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page.getByTestId("graph-node").filter({ hasText: "beach_day" })).toBeVisible();

  await page.getByTestId("graph-node").filter({ hasText: "about" }).dblclick();
  await page.getByRole("button", { name: "Add block" }).last().click();
  await page.getByPlaceholder("Search blocks").fill("choice");
  await page.keyboard.press("Enter");
  await page.getByLabel("Option text").last().fill("Go to the beach");
  await page.getByRole("button", { name: "Add block" }).last().click();
  await page.getByPlaceholder("Search blocks").fill("go to");
  await page.keyboard.press("Enter");
  await page.getByLabel("Scene").last().selectOption("beach_day");
  await page.waitForTimeout(400);

  await page.getByRole("tab", { name: /script\.kai/ }).click();
  await expect(page.locator(".monaco-editor .view-lines")).toContainText('"Go to the beach":');
  await expect(page.locator(".monaco-editor .view-lines")).toContainText("jump beach_day");

  await page.keyboard.press(`${mod}+z`);
  await page.keyboard.press(`${mod}+z`);
  await page.keyboard.press(`${mod}+z`);
  await expect(page.locator(".monaco-editor .view-lines")).not.toContainText("jump beach_day");

  await page.keyboard.press(`${mod}+s`);
  expect(await readFile(join(project, "script/script.kai"), "utf8")).toContain("label beach_day:");
  await app.close();
});
```

The three undos revert the scene picker change, then the go-to insert, then the option text, in that order. The added scene stays because it's further back in the history. Adjust the count if the debounced option-text commit lands as two entries. Inspect `past` in a store test if it does.

- [ ] **Step 2: Run it**

Run: `bash scripts/build-wasm.sh && bun run --cwd apps/editor test:e2e`
Expected: both specs pass. If a label in the test doesn't match the UI (`Scene name`, `Option text`, `Scene`, `Search blocks`), fix the UI's `aria-label` or placeholder to the plain-language name the test uses rather than changing the test.

- [ ] **Step 3: Update the docs**

README "Editor" section, a new paragraph after the commands block:

> Projects open on the story flow. Add, rename, duplicate and delete scenes there; drag between scenes to add a jump or a choice; click an arrow to edit or remove it. Double-click a scene to edit it as blocks: dialogue, backgrounds, characters, music, choices, conditions and variables, with the preview showing the selected moment. Anything without a block shows as a Code block you can still edit. Visual edits and typing share one undo history (Cmd+Z), and both change the same script files.

`docs/roadmap.md` item 3: split it into "3a. Visual story editor (done)", "3b. Custom screens" and "3c. Beginner path: templates, guided first project, code hidden until wanted".

- [ ] **Step 4: Run every check**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && bun run test && bun run typecheck && bun run lint && bun run fmt:check`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add apps/editor/e2e README.md docs/roadmap.md
git commit -m "Cover the beginner story-building flow end to end and document it"
```
