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
fn parses_custom_screen_statements_without_confusing_sprite_tags() {
    let (stmts, diagnostics) = parse("label start:\n    show screen phone\n    hide screen phone\n");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let kairos_script::StmtKind::Label { body, .. } = &stmts[0].kind else { panic!() };
    assert_eq!(body[0].kind, kairos_script::StmtKind::ShowScreen("phone".into()));
    assert_eq!(body[1].kind, kairos_script::StmtKind::HideScreen("phone".into()));
}

#[test]
fn deeply_nested_expression_reports_a_diagnostic() {
    let expression = format!("default x = {}1{}", "(".repeat(20_000), ")".repeat(20_000));
    let (_, diagnostics) = parse(&expression);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("nesting")), "{diagnostics:?}");
}

#[test]
fn long_index_chain_reports_a_diagnostic() {
    let expression = format!("default x = x{}", "[0]".repeat(100_000));
    let (_, diagnostics) = parse(&expression);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("expression")), "{diagnostics:?}");
}

#[test]
fn long_binary_expression_reports_a_diagnostic() {
    let expression = format!("$ x = {}", vec!["1"; 300].join("+"));
    let (_, diagnostics) = parse(&expression);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("expression")), "{diagnostics:?}");
}

#[test]
fn deeply_nested_blocks_report_a_diagnostic() {
    let mut source = String::from("label start:\n");
    for depth in 0..110 {
        source.push_str(&format!("{}if True:\n", "    ".repeat(depth + 1)));
    }
    source.push_str(&format!("{}pass\n", "    ".repeat(111)));
    let (_, diagnostics) = parse(&source);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("block nesting")), "{diagnostics:?}");
}

#[test]
fn rejects_non_ascii_indentation_whitespace() {
    let (_, diagnostics) = parse("label start:\n    \"A\"\n  　\"Hi\"\n");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("U+3000") && diagnostic.message.contains("spaces")),
        "{diagnostics:?}"
    );
}

#[test]
fn not_binds_looser_than_comparison() {
    let (stmts, _) = parse("default x = not 1 == 2");
    let kairos_script::StmtKind::Default { value, .. } = &stmts[0].kind else { panic!() };
    assert_eq!(format!("{value:?}"), "Unary(Not, Binary(Eq, Lit(Int(1)), Lit(Int(2))))");
}
