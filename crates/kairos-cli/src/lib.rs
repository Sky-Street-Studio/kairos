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
        Effect::Transition(t) => format!("[with {t}]"),
        Effect::Play { channel, asset, looped, queue, .. } => {
            let verb = if *queue { "queue" } else { "play" };
            format!("[{verb} {channel} {asset}{}]", if *looped { " loop" } else { "" })
        }
        Effect::Stop { channel, .. } => format!("[stop {channel}]"),
    }
}
