use std::sync::Arc;

use kairos_vm::{Event, LoadError, Persistent, Program, Value, Vm};
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
fn splashscreen_falling_off_its_label_returns_before_the_next_label() {
    let p = program("label splashscreen:\n    \"Splash\"\nlabel start:\n    \"Game\"\n");
    let mut vm = Vm::new(p.clone()).unwrap();
    vm.start("splashscreen").unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Splash");
    assert_eq!(vm.advance().unwrap(), Event::End);
    vm.start("start").unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Game");
}

#[test]
fn title_flow_statements_return_host_actions_and_advance() {
    let p = program("label start:\n    open load\n    open settings\n    quit\n");
    let mut vm = started(&p);
    assert_eq!(vm.advance().unwrap(), Event::OpenLoad);
    assert_eq!(vm.advance().unwrap(), Event::OpenSettings);
    assert_eq!(vm.advance().unwrap(), Event::Quit);
    assert_eq!(vm.advance().unwrap(), Event::End);
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
    assert!(!vm.relocated());
    assert_eq!(say(vm.advance().unwrap()), "Two.");
    assert_eq!(say(vm.advance().unwrap()), "Three.");

    let v3 = program("label start:\n    \"One.\"\n");
    assert!(matches!(Vm::load(v3, &bytes), Err(LoadError::MissingStatement(_))));
}

#[test]
fn a_reworded_saved_line_resumes_at_its_replacement_without_rerunning_earlier_lines() {
    let v1 = program(
        "default n = 0\nlabel start:\n    \"One.\"\n    $ n += 1\n    \"Two.\"\n    call side\n    \"Four.\"\nlabel side:\n    \"Three.\"\n    return\n",
    );
    let mut vm = started(&v1);
    vm.advance().unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Two.");
    let at_two = vm.save().unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Three.");
    let in_call = vm.save().unwrap();

    let v2 = program(
        "default n = 0\nlabel start:\n    \"One.\"\n    $ n += 1\n    \"Two, reworded.\"\n    call  side\n    \"Four.\"\nlabel side:\n    \"Three, reworded.\"\n    return\n",
    );
    let mut vm = Vm::load(v2.clone(), &at_two).unwrap();
    assert!(vm.relocated());
    assert_eq!(say(vm.advance().unwrap()), "Two, reworded.");
    assert_eq!(vm.variables()["n"], Value::Int(1));

    let mut vm = Vm::load(v2, &in_call).unwrap();
    assert_eq!(say(vm.advance().unwrap()), "Three, reworded.");
    assert_eq!(say(vm.advance().unwrap()), "Four.");
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

#[test]
fn persistent_values_survive_rollback_saves_and_new_games() {
    let p = program(
        "default persistent.endings = 0\nlabel start:\n    \"One\"\n    $ persistent.endings += 1\n    $ persistent.true_end = True\n    \"Two\"\n",
    );
    let mut vm = started(&p);
    assert_eq!(vm.variables()["persistent.endings"], Value::Int(0));
    say(vm.advance().unwrap());
    let save = vm.save().unwrap();
    say(vm.advance().unwrap());
    assert!(vm.take_persistent_changed());
    assert!(vm.rollback());
    say(vm.advance().unwrap());
    assert_eq!(vm.persistent().values["persistent.endings"], Value::Int(1));

    let mut loaded = Vm::load(p.clone(), &save).unwrap();
    loaded.set_persistent(vm.persistent().clone());
    assert_eq!(loaded.variables()["persistent.endings"], Value::Int(1));

    let mut fresh = Vm::new(p.clone()).unwrap();
    fresh.set_persistent(Persistent::from_bytes(&vm.persistent().to_bytes()).unwrap());
    assert_eq!(fresh.variables()["persistent.true_end"], Value::Bool(true));
}

#[test]
fn undeclared_persistent_names_read_as_none() {
    let p = program("label start:\n    if persistent.unlocked:\n        \"Secret\"\n    \"Normal\"\n");
    let mut vm = started(&p);
    assert_eq!(say(vm.advance().unwrap()), "Normal");
    assert!(!vm.take_persistent_changed());
}

#[test]
fn a_line_counts_as_seen_once_the_player_moves_past_it() {
    let p = program("label start:\n    \"One\"\n    \"Two\"\n    jump start\n");
    let mut vm = started(&p);
    say(vm.advance().unwrap());
    assert!(!vm.line_seen());
    say(vm.advance().unwrap());
    assert!(!vm.line_seen());
    assert!(vm.take_persistent_changed());
    say(vm.advance().unwrap());
    assert!(vm.line_seen(), "One was read on the first pass");
    say(vm.advance().unwrap());
    assert!(vm.line_seen(), "Two was read on the first pass");

    let mut next_playthrough = started(&p);
    next_playthrough.set_persistent(vm.persistent().clone());
    say(next_playthrough.advance().unwrap());
    assert!(next_playthrough.line_seen());
}

#[test]
fn persistent_files_reject_other_data() {
    assert!(Persistent::from_bytes(b"KSAV").is_err());
    assert!(Persistent::from_bytes(&[]).is_err());
}
