use std::collections::BTreeMap;
use std::fmt::Write as _;

pub use kairos_project::{Manifest, Project, ProjectError};
use kairos_vm::{Effect, Event, Vm};

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Runtime(#[from] kairos_vm::RuntimeError),
    #[error("menu reached at line {0} but no input is left")]
    OutOfInput(usize),
    #[error("input `{0}` is not a choice number")]
    BadInput(String),
}

/// Plays the project without graphics, answering menus from `inputs` (1-based choice numbers),
/// and returns a plain-text transcript.
pub fn update_translation_file(project: &Project, lang: &str) -> Result<(), Box<dyn std::error::Error>> {
    if lang.is_empty() || !lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "language must use letters, numbers, hyphens or underscores",
        )
        .into());
    }
    let dir = project.root.join("tl");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{lang}.toml"));
    let mut active = BTreeMap::new();
    let mut stale = BTreeMap::new();
    if path.exists() {
        let previous: toml::Value = toml::from_str(&std::fs::read_to_string(&path)?)?;
        if let Some(table) = previous.as_table() {
            for (key, value) in table {
                if key == "stale" {
                    if let Some(entries) = value.as_table() {
                        stale.extend(
                            entries.iter().filter_map(|(key, value)| {
                                value.as_str().map(|value| (key.clone(), value.to_string()))
                            }),
                        );
                    }
                } else if let Some(value) = value.as_str() {
                    active.insert(key.clone(), value.to_string());
                }
            }
        }
    }
    let entries = project.translation_entries();
    let live: BTreeMap<_, _> = entries.iter().cloned().collect();
    let mut next = BTreeMap::new();
    for (key, _) in &entries {
        let value = active.remove(key).or_else(|| stale.remove(key)).unwrap_or_default();
        next.insert(key.clone(), value);
    }
    stale.extend(active);
    let mut output = String::new();
    for (key, value) in &next {
        let source = live.get(key).expect("translation key came from source");
        writeln!(output, "# source: {}", source.replace(['\n', '\r'], " "))?;
        writeln!(output, "{} = {}", toml::Value::String(key.clone()), toml::Value::String(value.clone()))?;
    }
    if !stale.is_empty() {
        output.push_str("\n[stale]\n");
        for (key, value) in stale {
            writeln!(output, "{} = {}", toml::Value::String(key), toml::Value::String(value))?;
        }
    }
    std::fs::write(path, output)?;
    Ok(())
}

pub fn run_headless(project: &Project, inputs: &str) -> Result<String, RunError> {
    let mut answers = inputs.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'));
    let mut vm = Vm::new(project.program.clone())?;
    vm.start(&project.manifest.start)?;
    let mut out = String::new();
    loop {
        let event = vm.advance()?;
        for effect in vm.take_effects() {
            writeln!(out, "{}", describe_effect(&effect)).unwrap();
        }
        match event {
            Event::Say { who: Some(who), text, .. } => writeln!(out, "{who}: {text}").unwrap(),
            Event::Say { who: None, text, .. } => writeln!(out, "{text}").unwrap(),
            Event::Pause(Some(d)) => writeln!(out, "(pause {d}s)").unwrap(),
            Event::Pause(None) => writeln!(out, "(pause)").unwrap(),
            Event::OpenLoad | Event::OpenSettings => {}
            Event::Quit => {
                writeln!(out, "(quit)").unwrap();
                return Ok(out);
            }
            Event::Menu { caption, choices } => {
                if let Some(c) = caption {
                    writeln!(out, "{c}").unwrap();
                }
                for (i, c) in choices.iter().enumerate() {
                    writeln!(out, "  {}) {c}", i + 1).unwrap();
                }
                let answer = answers.next().ok_or(RunError::OutOfInput(vm.current_line()))?;
                let n: usize = answer.parse().map_err(|_| RunError::BadInput(answer.to_string()))?;
                let index = n.checked_sub(1).ok_or_else(|| RunError::BadInput(answer.to_string()))?;
                vm.choose(index)?;
                writeln!(out, "> {}", choices[index]).unwrap();
            }
            Event::End => {
                writeln!(out, "(end)").unwrap();
                return Ok(out);
            }
        }
    }
}

fn describe_effect(e: &Effect) -> String {
    let with = |w: &Option<String>| w.as_ref().map(|w| format!(" with {w}")).unwrap_or_default();
    match e {
        Effect::Scene { image: Some(i), with: w } => format!("[scene {}{}]", i.join(" "), with(w)),
        Effect::Scene { image: None, with: w } => format!("[scene{}]", with(w)),
        Effect::Show { sprite, with: w } => {
            let at = sprite.at.as_ref().map(|a| format!(" at {a}")).unwrap_or_default();
            let attrs: String = sprite.attrs.iter().map(|a| format!(" {a}")).collect();
            format!("[show {}{attrs}{at}{}]", sprite.tag, with(w))
        }
        Effect::Hide { tag, with: w } => format!("[hide {tag}{}]", with(w)),
        Effect::ShowScreen(id) => format!("[show screen {id}]"),
        Effect::HideScreen(id) => format!("[hide screen {id}]"),
        Effect::Transition(t) => format!("[with {t}]"),
        Effect::Play { channel, asset, looped, queue, .. } => {
            let verb = if *queue { "queue" } else { "play" };
            format!("[{verb} {channel} {asset}{}]", if *looped { " loop" } else { "" })
        }
        Effect::Stop { channel, .. } => format!("[stop {channel}]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn translation_generation_preserves_current_values_and_stales_removed_ids() {
        let root = std::env::temp_dir().join(format!("kairos-translate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let source = r#"label start:
    "Hello"
    "New line"
"#;
        let project =
            Project::from_sources("name = \"test\"", [(PathBuf::from("script/a.kai"), source.into())]).unwrap();
        let mut project = project;
        project.root = root.clone();
        let path = root.join("tl/ja.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let live_id = project.translation_entries()[0].0.clone();
        std::fs::write(&path, format!("{} = \"Bonjour\"\n\"legacy-id\" = \"Retired\"\n", toml::Value::String(live_id)))
            .unwrap();
        update_translation_file(&project, "ja").unwrap();
        let generated = std::fs::read_to_string(&path).unwrap();
        assert!(generated.contains("# source: Hello"));
        assert!(generated.contains("Bonjour"));
        assert!(generated.contains("= \"\""));
        assert!(generated.contains("[stale]"));
        assert!(generated.contains("legacy-id"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn translation_generation_rejects_path_components_in_language() {
        let project = Project::from_sources("name = \"test\"", []).unwrap();
        assert!(update_translation_file(&project, "../outside").is_err());
    }
}
