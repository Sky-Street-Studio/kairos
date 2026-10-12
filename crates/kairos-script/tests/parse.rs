use kairos_script::parse;

#[test]
fn parses_a_scene_with_menu_and_conditions() {
    let src = r##"
character e "Eileen" color="#c8ffc8"
default score = 0

label start:
    scene bg room with fade
    show e happy at center
    e "Hi, [name]."  # comment with "quotes"
    menu:
        "Ask" if score >= 1 and not done:
            $ score += 2 * (1 + 1)
        "Leave":
            return
    if score == 0:
        "Nothing."
    elif score > 3:
        pause 1.5
    else:
        play music "a.ogg" loop fadein 2
"##;
    let (stmts, diags) = parse(src);
    assert!(diags.is_empty(), "{diags:?}");
    insta::assert_debug_snapshot!(stmts);
}

#[test]
fn reports_errors_and_keeps_parsing() {
    let src = r#"
label start:
    e "unterminated
    frobnicate now
    menu:
    "Still parsed."
"#;
    let (stmts, diags) = parse(src);
    let lines: Vec<(usize, &str)> = diags.iter().map(|d| (d.line, d.message.as_str())).collect();
    assert_eq!(
        lines,
        vec![(3, "unterminated string"), (4, "unknown statement `frobnicate`"), (5, "expected an indented block"),]
    );
    let kairos_script::StmtKind::Label { body, .. } = &stmts[0].kind else { panic!() };
    assert_eq!(body.last().unwrap().source, "\"Still parsed.\"");
}

#[test]
fn parses_title_flow_statements() {
    let (stmts, diags) = parse("label main_menu:\n    open load\n    open settings\n    quit\n");
    assert!(diags.is_empty(), "{diags:?}");
    let kairos_script::StmtKind::Label { body, .. } = &stmts[0].kind else { panic!() };
    assert_eq!(
        body.iter().map(|stmt| &stmt.kind).collect::<Vec<_>>(),
        [&kairos_script::StmtKind::OpenLoad, &kairos_script::StmtKind::OpenSettings, &kairos_script::StmtKind::Quit,]
    );
}

#[test]
fn not_binds_looser_than_comparison() {
    let (stmts, _) = parse("default x = not 1 == 2");
    let kairos_script::StmtKind::Default { value, .. } = &stmts[0].kind else { panic!() };
    assert_eq!(format!("{value:?}"), "Unary(Not, Binary(Eq, Lit(Int(1)), Lit(Int(2))))");
}
