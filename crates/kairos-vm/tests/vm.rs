use std::sync::Arc;

use kairos_vm::{Event, LoadError, Program, Vm};
use proptest::prelude::*;

fn program(src: &str) -> Arc<Program> {
    let (stmts, diags) = kairos_script::parse(src);
    assert!(diags.is_empty(), "{diags:?}");
    let (program, diags) = kairos_vm::compile(&[stmts]);
    assert!(diags.is_empty(), "{diags:?}");
    Arc::new(program)
}

fn started(p: &Arc<Program>) -> Vm {
    let mut vm = Vm::new(p.clone()).unwrap();
    vm.start("start").unwrap();
    vm
}

fn say(e: Event) -> String {
    match e {
        Event::Say { text, .. } => text,
        other => panic!("expected dialogue, got {other:?}"),
    }
}

const LOOP: &str = r#"
default n = 0
label start:
    show e happy
    "Line [n]."
    $ n += 1
    if n % 3 == 0:
        menu:
            "Up":
                $ n += 10
            "Down":
                $ n -= 1
    if n < 60:
        jump start
    "Done at [n]."
"#;

proptest! {
    #[test]
    fn rolling_back_restores_every_earlier_interaction(steps in 2usize..40, back in 1usize..20) {
        let p = program(LOOP);
        let mut vm = started(&p);
        let mut states = Vec::new();
        for i in 0..steps {
            match vm.advance().unwrap() {
                Event::Menu { .. } => {
                    states.push(vm.state().clone());
                    vm.choose(i % 2).unwrap();
                }
                Event::End => break,
                _ => states.push(vm.state().clone()),
            }
        }
        let back = back.min(states.len() - 1);
        for _ in 0..back {
            prop_assert!(vm.rollback());
        }
        vm.advance().unwrap();
        prop_assert_eq!(vm.state(), &states[states.len() - 1 - back]);
    }
}

#[test]
fn rollback_stops_at_the_first_interaction() {
    let p = program(LOOP);
    let mut vm = started(&p);
    vm.advance().unwrap();
    assert!(!vm.rollback());
}

#[test]
fn save_and_load_resume_the_same_line_with_the_same_variables() {
    let p = program(LOOP);
    let mut vm = started(&p);
    for _ in 0..3 {
        vm.advance().unwrap();
    }
    let saved_state = vm.state().clone();
    let bytes = vm.save().unwrap();
    let next = vm.advance().unwrap();

    let mut loaded = Vm::load(p.clone(), &bytes).unwrap();
    let replayed = loaded.advance().unwrap();
    assert_eq!(loaded.state(), &saved_state);
    assert_eq!(loaded.advance().unwrap(), next);
    assert_ne!(replayed, next);
}

#[test]
fn saves_survive_unrelated_edits_but_not_deleted_lines() {
    let v1 = program(
        r#"
label start:
    "One."
    call side
    "Three."
label side:
    "Two."
    return
"#,
    );
    let mut vm = started(&v1);
    vm.advance().unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Two.");
    let bytes = vm.save().unwrap();

    let v2 = program(
        r#"
label start:
    "Zero, newly added."
    "One."
    call side
    "Three."
label side:
    "Before two, also new."
    "Two."
    return
"#,
    );
    let mut vm = Vm::load(v2, &bytes).unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Two.");
    assert_eq!(say(vm.advance().unwrap()), "Three.");

    let v3 = program("label start:\n    \"One.\"\n    call side\nlabel side:\n    \"Renamed two.\"\n    return\n");
    assert!(matches!(Vm::load(v3, &bytes), Err(LoadError::MissingStatement(_))));
}

#[test]
fn guarded_choices_are_hidden_and_indices_follow_visible_choices() {
    let p = program(
        r#"
default key = False
label start:
    menu:
        "Open the door" if key:
            "Opened."
        "Look around":
            "Looked."
"#,
    );
    let mut vm = started(&p);
    let Event::Menu { choices, .. } = vm.advance().unwrap() else { panic!() };
    assert_eq!(choices, ["Look around"]);
    vm.choose(0).unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Looked.");
}

#[test]
fn define_is_constant_and_runtime_errors_name_the_line() {
    let p = program("define speed = 1\nlabel start:\n    $ speed = 2\n");
    let err = started(&p).advance().unwrap_err();
    assert_eq!(err.line, 3);
    assert!(err.message.contains("constant"), "{}", err.message);
}

#[test]
fn unknown_labels_and_characters_are_compile_errors() {
    let (stmts, _) = kairos_script::parse("label start:\n    zed \"Hi\"\n    jump nowhere\n");
    let (_, diags) = kairos_vm::compile(&[stmts]);
    let messages: Vec<_> = diags.iter().map(|(_, d)| (d.line, d.message.clone())).collect();
    assert_eq!(messages, [(2, "unknown character `zed`".to_string()), (3, "unknown label `nowhere`".to_string())]);
}
