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
fn custom_screen_statements_emit_presentation_effects_and_continue() {
    let p = program(
        r#"label start:
    show screen phone
    hide screen phone
    "Done."
"#,
    );
    let mut vm = started(&p);
    assert_eq!(say(vm.advance().unwrap()), "Done.");
    assert_eq!(
        vm.take_effects(),
        vec![kairos_vm::Effect::ShowScreen("phone".into()), kairos_vm::Effect::HideScreen("phone".into()),]
    );
}

#[test]
fn an_nvl_character_enters_nvl_mode() {
    let p = program("character e \"Eileen\" nvl\nlabel start:\n    e \"First\"\n");
    let mut vm = started(&p);
    assert_eq!(say(vm.advance().unwrap()), "First");
    assert!(vm.state().nvl_mode);
    assert_eq!(vm.state().nvl_page, [(Some("Eileen".into()), "First".into())]);
}

#[test]
fn nvl_page_accumulates_and_survives_rollback_and_save_load() {
    let p =
        program("label start:\n    nvl on\n    \"First\"\n    \"Second\"\n    nvl clear\n    nvl off\n    \"Third\"\n");
    let mut vm = started(&p);
    assert_eq!(say(vm.advance().unwrap()), "First");
    assert!(vm.state().nvl_mode);
    assert_eq!(vm.state().nvl_page, [(None, "First".into())]);
    assert_eq!(say(vm.advance().unwrap()), "Second");
    assert_eq!(vm.state().nvl_page, [(None, "First".into()), (None, "Second".into())]);
    let bytes = vm.save().unwrap();
    let mut loaded = Vm::load(p.clone(), &bytes).unwrap();
    assert_eq!(say(loaded.advance().unwrap()), "Second");
    assert_eq!(loaded.state().nvl_page.len(), 2);
    assert!(vm.rollback());
    assert_eq!(say(vm.advance().unwrap()), "First");
    assert_eq!(vm.state().nvl_page.len(), 1);
    assert_eq!(say(vm.advance().unwrap()), "Second");
    assert_eq!(vm.advance().unwrap(), Event::Say { who: None, color: None, text: "Third".into() });
    assert!(vm.state().nvl_page.is_empty());
    assert!(!vm.state().nvl_mode);
}

#[test]
fn stage_tweens_camera_and_effects_advance_and_survive_saves() {
    let p = program(
        "label start:\n    show e happy\n    stage e x=100 y=20 scale=2 rotation=30 alpha=0.5 over=2 ease\n    camera x=50 y=10 zoom=1.2 over=2 linear\n    shake 2 4\n    flash \"#ffffff\" 2\n    \"Line\"\n",
    );
    let mut vm = started(&p);
    assert_eq!(say(vm.advance().unwrap()), "Line");
    vm.tick_stage(1.0, false);
    let sprite = &vm.state().stage.sprites[0];
    assert_eq!(sprite.transform.x, 50.0);
    assert_eq!(sprite.transform.scale, 1.5);
    assert_eq!(sprite.transform.alpha, 0.75);
    assert_eq!(vm.state().stage.camera.x, 25.0);
    assert_eq!(vm.state().stage.camera.zoom, 1.1);
    assert!(vm.state().stage.shake.is_some());
    assert!(vm.state().stage.flash.is_some());
    let bytes = vm.save().unwrap();
    let mut loaded = Vm::load(p, &bytes).unwrap();
    assert_eq!(loaded.state().stage.sprites[0].transform.x, 50.0);
    loaded.tick_stage(1.0, false);
    assert_eq!(loaded.state().stage.sprites[0].transform.x, 100.0);
    assert_eq!(loaded.state().stage.camera.zoom, 1.2);
    assert!(loaded.state().stage.tweens.is_empty());
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

fn lines(src: &str) -> Vec<String> {
    let p = program(src);
    let mut vm = started(&p);
    let mut out = Vec::new();
    loop {
        match vm.advance().unwrap() {
            Event::Say { text, .. } => out.push(text),
            Event::End => return out,
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn lists_and_dicts_read_like_python() {
    let out = lines(
        r#"
default inventory = ["key", "map"]
default affection = {"kai": 3, "eileen": 5}
label start:
    "[inventory[0]] [inventory[-1]] [len(inventory)]"
    "[affection['eileen']] [len(affection)]"
    if "map" in inventory and "rope" not in inventory:
        "Has the map."
    "[inventory] [affection]"
    "[([1, 2] + [3])] [len([]) == 0] [max(affection['kai'], 4)]"
    "[affection.get('nobody', 0)] [affection.keys()]"
"#,
    );
    assert_eq!(
        out,
        [
            "key map 2",
            "5 2",
            "Has the map.",
            "['key', 'map'] {'eileen': 5, 'kai': 3}",
            "[1, 2, 3] True 4",
            "0 ['eileen', 'kai']",
        ]
    );
}

#[test]
fn bad_indexes_name_the_problem() {
    let p = program("default xs = [1]\nlabel start:\n    \"[xs[3]]\"\n");
    let err = started(&p).advance().unwrap_err();
    assert!(err.message.contains("index 3 is out of range"), "{}", err.message);
    let p = program("default d = {}\nlabel start:\n    \"[d['k']]\"\n");
    let err = started(&p).advance().unwrap_err();
    assert!(err.message.contains("no key 'k'"), "{}", err.message);
}

#[test]
fn lists_and_dicts_change_in_place_and_roll_back() {
    let p = program(
        r#"
default inventory = []
default stats = {"kai": {"affection": 0}}
default persistent.gallery = []
label start:
    $ inventory.append("key")
    $ inventory.extend(["map", "rope"])
    $ inventory.insert(0, "coin")
    $ inventory.remove("map")
    $ stats["kai"]["affection"] += 2
    $ stats["eileen"] = {"affection": 1}
    $ persistent.gallery.append("cg_beach")
    "[inventory] [stats['kai']['affection']] [len(stats)]"
    $ inventory.pop()
    $ inventory[0] = "gem"
    $ stats.pop("eileen")
    "[inventory] [stats.keys()] [persistent.gallery]"
"#,
    );
    let mut vm = started(&p);
    assert_eq!(say(vm.advance().unwrap()), "['coin', 'key', 'rope'] 2 2");
    assert!(vm.take_persistent_changed());
    assert_eq!(say(vm.advance().unwrap()), "['gem', 'key'] ['kai'] ['cg_beach']");
    assert!(vm.rollback());
    assert_eq!(say(vm.advance().unwrap()), "['coin', 'key', 'rope'] 2 2");
}

#[test]
fn mutating_a_constant_or_using_a_mutating_method_as_a_value_fails() {
    let p = program("define ROUTES = [\"a\"]\nlabel start:\n    $ ROUTES.append(\"b\")\n    \"x\"\n");
    let err = started(&p).advance().unwrap_err();
    assert!(err.message.contains("constant"), "{}", err.message);
    let p = program("default xs = []\nlabel start:\n    \"[xs.append(1)]\"\n");
    let err = started(&p).advance().unwrap_err();
    assert!(err.message.contains("use it as its own statement"), "{}", err.message);
}

#[test]
fn nested_list_growth_stops_at_the_runtime_size_limit() {
    let p = program("default l = [1]\nlabel start:\n    $ l = [l, l]\n    jump start\n");
    let mut vm = started(&p);
    let error = loop {
        match vm.advance() {
            Ok(Event::End) => panic!("the loop should not end"),
            Ok(_) => {}
            Err(error) => break error,
        }
    };
    assert!(error.message.contains("value size limit"), "{}", error.message);
}

#[test]
fn indexed_list_growth_stops_at_the_runtime_size_limit_without_changing_state() {
    let p = program("default l = [1]\nlabel start:\n    $ l[0] = l\n    \"next\"\n    jump start\n");
    let mut vm = started(&p);
    vm.set_variable("l", Value::List(vec![Value::Int(1); 600_000])).unwrap();
    loop {
        let before = vm.variables()["l"].clone();
        match vm.advance() {
            Ok(Event::End) => panic!("the loop should not end"),
            Ok(_) => {}
            Err(error) => {
                assert!(error.message.contains("value size limit exceeded"), "{}", error.message);
                assert_eq!(vm.variables()["l"], before);
                break;
            }
        }
    }
}

#[test]
fn rejected_indexed_mutation_does_not_change_state() {
    let p = program("default l = [1]\nlabel start:\n    $ l.append(l)\n    \"next\"\n    jump start\n");
    let mut vm = started(&p);
    loop {
        let before = vm.variables()["l"].clone();
        match vm.advance() {
            Ok(Event::End) => panic!("the loop should not end"),
            Ok(_) => {}
            Err(error) => {
                assert!(error.message.contains("value size limit exceeded"), "{}", error.message);
                assert_eq!(vm.variables()["l"], before);
                break;
            }
        }
    }
}

#[test]
fn seed_variable_returns_the_value_size_error() {
    let p = program("default n = 0\nlabel start:\n    return\n");
    let mut vm = started(&p);
    let oversized = Value::List(vec![Value::Int(1); 1_000_001]);

    let result = vm.seed_variable("n", oversized);

    assert!(result.is_err());
    assert_eq!(vm.variables()["n"], Value::Int(0));
}

#[test]
fn indexed_dictionary_growth_stops_at_the_runtime_size_limit() {
    let p = program("default d = {\"a\": 1}\nlabel start:\n    $ d[\"a\"] = d\n    $ d[\"b\"] = d\n    jump start\n");
    let mut vm = started(&p);
    let error = loop {
        match vm.advance() {
            Ok(Event::End) => panic!("the loop should not end"),
            Ok(_) => {}
            Err(error) => break error,
        }
    };
    assert!(error.message.contains("value size limit exceeded"), "{}", error.message);
}

#[test]
fn nested_dictionary_growth_stops_at_the_runtime_size_limit() {
    let p = program("default d = {\"a\": 1}\nlabel start:\n    $ d = {\"left\": d, \"right\": d}\n    jump start\n");
    let mut vm = started(&p);
    let error = loop {
        match vm.advance() {
            Ok(Event::End) => panic!("the loop should not end"),
            Ok(_) => {}
            Err(error) => break error,
        }
    };
    assert!(error.message.contains("value size limit"), "{}", error.message);
}

#[test]
fn string_concatenation_stops_at_the_runtime_size_limit() {
    let p = program("default s = \"x\"\nlabel start:\n    $ s = s + s\n    \"next\"\n    jump start\n");
    let mut vm = started(&p);
    let error = loop {
        match vm.advance() {
            Ok(Event::End) => panic!("the loop should not end"),
            Ok(_) => {}
            Err(error) => break error,
        }
    };
    assert!(error.message.contains("value size limit"), "{}", error.message);
}

#[test]
fn list_growth_stops_at_the_runtime_size_limit() {
    let doubled = "default xs = range(100000)\nlabel start:\n    $ xs = xs + xs\n    \"next\"\n    jump start\n";
    let p = program(doubled);
    let mut vm = started(&p);
    for _ in 0..3 {
        assert!(matches!(vm.advance().unwrap(), Event::Say { .. }));
    }
    let error = vm.advance().unwrap_err();
    assert!(error.message.contains("value size limit"), "{}", error.message);

    let extended = "default xs = range(100000)\nlabel start:\n    $ xs.extend(xs)\n    \"next\"\n    jump start\n";
    let p = program(extended);
    let mut vm = started(&p);
    for _ in 0..3 {
        assert!(matches!(vm.advance().unwrap(), Event::Say { .. }));
    }
    let error = vm.advance().unwrap_err();
    assert!(error.message.contains("value size limit"), "{}", error.message);

    let appended = "default xs = range(100000)\nlabel start:\n    $ xs = xs + xs\n    \"next\"\n    if len(xs) < 800000:\n        jump start\n    $ xs.extend(range(100000))\n    $ xs.extend(range(100000))\n    $ xs.append(1)\n    \"done\"\n";
    let p = program(appended);
    let mut vm = started(&p);
    for _ in 0..3 {
        assert!(matches!(vm.advance().unwrap(), Event::Say { .. }));
    }
    let error = vm.advance().unwrap_err();
    assert!(error.message.contains("value size limit"), "{}", error.message);
}

#[test]
fn random_draws_replay_after_rollback_and_load() {
    let p = program(
        "default roll = 0\nlabel start:\n    $ roll = randint(1, 1000000)\n    \"[roll] [randint(1, 1000000)]\"\n    \"[choice(['a', 'b', 'c'])] [random() < 1.0] [randint(3, 3)]\"\n",
    );
    let seeded = |seed| {
        let mut vm = Vm::new(p.clone()).unwrap();
        vm.seed_random(seed);
        vm.start("start").unwrap();
        vm
    };
    let mut vm = seeded(7);
    let first = say(vm.advance().unwrap());
    let save = vm.save().unwrap();
    let second = say(vm.advance().unwrap());
    assert!(second.ends_with(" True 3"), "{second}");

    assert!(vm.rollback());
    assert_eq!(say(vm.advance().unwrap()), first);
    assert_eq!(say(vm.advance().unwrap()), second);

    let mut loaded = Vm::load(p.clone(), &save).unwrap();
    assert_eq!(say(loaded.advance().unwrap()), first);
    assert_eq!(say(loaded.advance().unwrap()), second);

    assert_eq!(say(seeded(7).advance().unwrap()), first);
    assert_ne!(say(seeded(8).advance().unwrap()), first);
}

#[test]
fn unknown_functions_and_methods_are_compile_errors() {
    let (stmts, _) = kairos_script::parse(
        "default xs = []\nlabel start:\n    $ n = lenn(xs)\n    $ xs.apend(1)\n    if xs.sizes() > 0:\n        \"[randomint(1, 2)]\"\n    $ xs.append(len(xs))\n",
    );
    let (_, diags) = kairos_vm::compile(&[stmts]);
    let messages: Vec<_> = diags.iter().map(|(_, d)| (d.line, d.message.clone())).collect();
    assert_eq!(
        messages,
        [
            (3, "there is no function `lenn`".to_string()),
            (4, "there is no method `apend`".to_string()),
            (5, "there is no method `sizes`".to_string()),
            (6, "there is no function `randomint`".to_string()),
        ]
    );
}
