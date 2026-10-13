use kairos_script::edit::{BlockKind, EditError, Op, Place, RefKind, SourceFile, apply, references, scene_blocks};

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

#[test]
fn move_to_a_later_position() {
    let src = "label s:\n    \"A\"\n    \"B\"\n    \"C\"\n    \"D\"\n";
    let out = run(src, vec![Op::Move { path: P.into(), line: 2, place: Place::After { line: 5 } }]);
    assert_eq!(out, "label s:\n    \"B\"\n    \"C\"\n    \"D\"\n    \"A\"\n");
}

#[test]
fn header_ops_need_the_right_owner() {
    let src = "label start:\n    if a:\n        pass\n    else:\n        pass\n    menu:\n        \"Go\":\n            pass\n";
    let err = |op: Op| apply(&file(src), &[op]).unwrap_err();
    assert!(matches!(
        err(Op::SetBranchCondition { path: P.into(), line: 1, condition: "x".into() }),
        EditError::NotFound(_)
    ));
    assert!(matches!(err(Op::SetChoiceText { path: P.into(), line: 1, text: "x".into() }), EditError::NotFound(_)));
    assert!(matches!(err(Op::SetChoiceText { path: P.into(), line: 2, text: "x".into() }), EditError::NotFound(_)));
    assert!(matches!(err(Op::SetChoiceCondition { path: P.into(), line: 4, condition: None }), EditError::NotFound(_)));
    assert!(matches!(
        err(Op::SetBranchCondition { path: P.into(), line: 7, condition: "x".into() }),
        EditError::NotFound(_)
    ));
    assert!(matches!(
        err(Op::SetBranchCondition { path: P.into(), line: 4, condition: "x".into() }),
        EditError::Invalid(_)
    ));
    assert!(matches!(err(Op::AddBranch { path: P.into(), line: 2, condition: None }), EditError::Invalid(_)));
}

fn files(a: &str, b: &str) -> Vec<SourceFile> {
    vec![
        SourceFile { path: "script/a.kai".into(), source: a.into() },
        SourceFile { path: "script/b.kai".into(), source: b.into() },
    ]
}

fn run_all(fs: &[SourceFile], ops: Vec<Op>) -> Vec<String> {
    let applied = apply(fs, &ops).unwrap();
    fs.iter()
        .map(|f| applied.files.iter().find(|c| c.path == f.path).map_or(f.source.clone(), |c| c.source.clone()))
        .collect()
}

#[test]
fn add_scene_after_a_label_or_at_the_end() {
    let fs = files("label start:\n    jump day\nlabel day:\n    return\n", "");
    let out = run_all(
        &fs,
        vec![Op::AddScene { path: "script/a.kai".into(), name: "beach".into(), after: Some("start".into()) }],
    );
    assert_eq!(out[0], "label start:\n    jump day\n\nlabel beach:\n    \"\"\n    return\nlabel day:\n    return\n");
    let out = run_all(&fs, vec![Op::AddScene { path: "script/b.kai".into(), name: "end".into(), after: None }]);
    assert_eq!(out[1], "label end:\n    \"\"\n    return\n");
    assert!(matches!(
        apply(&fs, &[Op::AddScene { path: "script/a.kai".into(), name: "day".into(), after: None }]),
        Err(EditError::Invalid(_))
    ));
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
    let fs = files(
        "label a:\n    jump x\n    call x\n",
        "label b:\n    menu:\n        \"Go\":\n            jump x\nlabel x:\n    return\n",
    );
    let refs: Vec<_> = references(&fs, "x").into_iter().map(|r| (r.path, r.line, r.kind, r.choice)).collect();
    assert_eq!(
        refs,
        [
            ("script/a.kai".into(), 2, RefKind::Jump, None),
            ("script/a.kai".into(), 3, RefKind::Call, None),
            ("script/b.kai".into(), 4, RefKind::Jump, Some("Go".into())),
        ]
    );
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

    let fs = files("label a:\n    jump x\n", "label x:\n    return\n");
    let out = run_all(&fs, vec![Op::DeleteScene { label: "x".into(), remove_references: true }]);
    assert_eq!(out[0], "label a:\n    pass\n");
}

#[test]
fn connect_adds_jumps_and_choices_and_remove_link_undoes_them() {
    let fs = files("label a:\n    \"Hi\"\nlabel b:\n    return\nlabel c:\n    return\n", "");
    let out = run_all(
        &fs,
        vec![
            Op::Connect { from: "a".into(), to: "b".into(), choice: Some("To B".into()) },
            Op::Connect { from: "a".into(), to: "c".into(), choice: Some("To C".into()) },
        ],
    );
    assert_eq!(
        out[0],
        "label a:\n    \"Hi\"\n    menu:\n        \"To B\":\n            jump b\n        \"To C\":\n            jump c\nlabel b:\n    return\nlabel c:\n    return\n"
    );
    let fs2 = files(&out[0], "");
    let out = run_all(&fs2, vec![Op::RemoveLink { from: "a".into(), to: "b".into(), choice: Some("To B".into()) }]);
    assert!(!out[0].contains("To B") && out[0].contains("To C"));
    let out = run_all(
        &fs,
        vec![Op::Connect { from: "a".into(), to: "b".into(), choice: None }, Op::EndScene { label: "b".into() }],
    );
    assert!(out[0].starts_with("label a:\n    \"Hi\"\n    jump b\n"));
}

#[test]
fn duplicate_and_set_character() {
    let fs = files("character e \"Eileen\"\nlabel a:\n    e \"Hi\"\n", "");
    let out = run_all(
        &fs,
        vec![
            Op::DuplicateScene { label: "a".into(), name: "a_copy".into() },
            Op::SetCharacter {
                path: "script/a.kai".into(),
                id: "e".into(),
                name: "Eve".into(),
                color: Some("#ffcc00".into()),
            },
            Op::SetCharacter { path: "script/a.kai".into(), id: "k".into(), name: "Kai".into(), color: None },
        ],
    );
    assert_eq!(
        out[0],
        "character e \"Eve\" color=\"#ffcc00\"\ncharacter k \"Kai\"\nlabel a:\n    e \"Hi\"\n\nlabel a_copy:\n    e \"Hi\"\n"
    );
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
