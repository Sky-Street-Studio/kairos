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
    let out = run(
        src,
        vec![
            Op::Insert { path: P.into(), place: Place::After { line: 3 }, block: say("1.5") },
            Op::Insert { path: P.into(), place: Place::Start { header: 2 }, block: say("Zero") },
        ],
    );
    assert_eq!(out, "# intro\nlabel start:\n    \"Zero\"\n    \"One\"  # keep\n    \"1.5\"\n\n    \"Two\"\n");
}

#[test]
fn insert_keeps_crlf() {
    let out = run(
        "label start:\r\n    \"One\"\r\n",
        vec![Op::Insert { path: P.into(), place: Place::After { line: 2 }, block: say("Two") }],
    );
    assert_eq!(out, "label start:\r\n    \"One\"\r\n    \"Two\"\r\n");
}

#[test]
fn insert_matches_sibling_indent() {
    let out = run(
        "label start:\n  \"One\"\n",
        vec![Op::Insert { path: P.into(), place: Place::After { line: 2 }, block: say("Two") }],
    );
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
    assert_eq!(
        run(src, vec![Op::Delete { path: P.into(), line: 5 }]),
        "label start:\n    \"A\"\n    menu:\n        \"Go\":\n            pass\n"
    );
    assert_eq!(
        run(src, vec![Op::Move { path: P.into(), line: 2, place: Place::Start { header: 4 } }]),
        "label start:\n    menu:\n        \"Go\":\n            \"A\"\n            \"B\"\n"
    );
    let err =
        apply(&file(src), &[Op::Move { path: P.into(), line: 3, place: Place::Start { header: 4 } }]).unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)));
}

#[test]
fn choice_and_branch_ops_edit_headers_only() {
    let src = "label start:\n    menu:\n        \"Go\" if a:  # c\n            jump x\n    if n > 1:\n        pass\nlabel x:\n    return\n";
    let out = run(
        src,
        vec![
            Op::SetChoiceText { path: P.into(), line: 3, text: "Leave \"now\"".into() },
            Op::SetChoiceCondition { path: P.into(), line: 3, condition: None },
            Op::AddChoiceOption { path: P.into(), line: 2, text: "Stay".into() },
            Op::SetBranchCondition { path: P.into(), line: 7, condition: "n > 2".into() },
            Op::AddBranch { path: P.into(), line: 7, condition: None },
        ],
    );
    assert_eq!(
        out,
        "label start:\n    menu:\n        \"Leave \\\"now\\\"\":  # c\n            jump x\n        \"Stay\":\n            pass\n    if n > 2:\n        pass\n    else:\n        pass\nlabel x:\n    return\n"
    );
}

#[test]
fn edits_that_break_parsing_are_rejected_but_existing_errors_are_tolerated() {
    let broken = BlockKind::Code { text: "menu:".into() };
    let err = apply(
        &file("label start:\n    \"A\"\n"),
        &[Op::Insert { path: P.into(), place: Place::After { line: 2 }, block: broken }],
    )
    .unwrap_err();
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
            if let Ok(applied) = apply(&file(&source), &[op])
                && let Some(f) = applied.files.into_iter().next()
            {
                source = f.source;
            }
            let (_, after) = kairos_script::parse(&source);
            proptest::prop_assert!(after.len() <= before.len(), "{after:?}\n{source}");
        }
    }
}
