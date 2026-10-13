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
            match t.get(..n.indent).filter(|p| p.trim().is_empty()) {
                Some(_) => &t[n.indent..],
                None => t.trim_start(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    BlockKind::Code { text: text.trim_end().into() }
}

fn header(lines: &Lines, line: usize) -> &str {
    strip_comment(lines.text(line)).trim()
}

/// The text after a leading string literal, e.g. ` if key:` in `"Go" if key:`.
pub(crate) fn after_string(s: &str) -> &str {
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
        StmtKind::Scene { image: Some(image), with } => {
            BlockKind::Background { image: image.clone(), transition: with.clone() }
        }
        StmtKind::Show { image, at, with } => {
            BlockKind::Show { image: image.clone(), position: at.clone(), transition: with.clone() }
        }
        StmtKind::Hide { tag, with } => BlockKind::Hide { character: tag.clone(), transition: with.clone() },
        StmtKind::Play { channel, asset, looped, queue: false, fadein }
            if ["music", "sound", "voice"].contains(&channel.as_str()) =>
        {
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
                .map(|b| IfBranch {
                    line: b.header,
                    condition: condition_of_branch(lines, b.header),
                    blocks: blocks(&b.nodes, lines),
                })
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
        BlockKind::Pause { seconds } => {
            one(seconds.map(|s| format!("pause {}", number(s))).unwrap_or_else(|| "pause".into()))
        }
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
        BlockKind::Code { text } => {
            text.lines().map(|l| if l.is_empty() { String::new() } else { format!("{pad}{l}") }).collect()
        }
    }
}

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
                BlockKind::Show {
                    image: vec!["e".into(), "happy".into()],
                    position: Some("left".into()),
                    transition: None
                },
                BlockKind::Dialogue { character: Some("e".into()), text: "Hi".into() },
                BlockKind::Dialogue { character: None, text: "Narration".into() },
                BlockKind::Audio {
                    channel: "music".into(),
                    asset: "theme.ogg".into(),
                    looped: true,
                    fade_in: Some(1.5)
                },
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
        assert_eq!(
            (options[0].line, options[0].text.as_str(), options[0].condition.as_deref()),
            (3, "Go", Some("key and n > 1"))
        );
        assert_eq!(options[1].blocks[0].kind, BlockKind::Dialogue { character: None, text: "Ok".into() });
        let BlockKind::If { branches } = &blocks[1].kind else { panic!() };
        assert_eq!(
            branches.iter().map(|b| (b.line, b.condition.clone())).collect::<Vec<_>>(),
            [(7, Some("n > 2".into())), (9, None)]
        );
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
        assert_eq!(
            r(BlockKind::SetVariable { name: "seen".into(), op: SetOp::Toggle, value: String::new() }),
            "    $ seen = not seen"
        );
        assert_eq!(
            r(BlockKind::Choice {
                options: vec![ChoiceOption {
                    line: 0,
                    text: "Go".into(),
                    condition: Some("key".into()),
                    blocks: vec![]
                }]
            }),
            "    menu:\n        \"Go\" if key:\n            pass"
        );
        assert_eq!(r(BlockKind::Code { text: "if a:\n    pass".into() }), "    if a:\n        pass");
        assert_eq!(
            r(BlockKind::Audio { channel: "music".into(), asset: "t.ogg".into(), looped: true, fade_in: None }),
            "    play music \"t.ogg\" loop"
        );
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
    fn non_ascii_comment_inside_a_statement_does_not_split_a_char() {
        let src = "label start:\n    menu:\n# 日本語\n        \"Caption\"\n        \"Only\":\n            pass\n";
        let blocks = kinds(src);
        let [BlockKind::Code { text }] = blocks.as_slice() else { panic!() };
        assert!(text.contains("# 日本語"), "{text}");
    }

    #[test]
    fn blocks_serialize_with_a_kind_tag() {
        let block = Block { line: 2, end_line: 2, kind: BlockKind::GoTo { scene: "x".into() } };
        assert_eq!(serde_json::to_string(&block).unwrap(), r#"{"line":2,"endLine":2,"kind":"goTo","scene":"x"}"#);
    }
}
