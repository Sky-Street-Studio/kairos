use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kairos_script::Diagnostic;
use kairos_vm::{Effect, Event, Program, Vm};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default = "default_start")]
    pub start: String,
}

fn default_start() -> String {
    "start".into()
}

pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub program: Arc<Program>,
    /// Diagnostics paired with the file they came from.
    pub diagnostics: Vec<(PathBuf, Diagnostic)>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("kairos.toml: {0}")]
    Manifest(#[from] toml::de::Error),
}

impl Project {
    pub fn load(root: &Path) -> Result<Project, ProjectError> {
        let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| ProjectError::Io(p.to_path_buf(), e));
        let manifest: Manifest = toml::from_str(&read(&root.join("kairos.toml"))?)?;
        let script_dir = root.join("script");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&script_dir)
            .map_err(|e| ProjectError::Io(script_dir.clone(), e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "kai"))
            .collect();
        files.sort();

        let mut diagnostics = Vec::new();
        let mut parsed = Vec::new();
        for file in &files {
            let (stmts, diags) = kairos_script::parse(&read(file)?);
            diagnostics.extend(diags.into_iter().map(|d| (file.clone(), d)));
            parsed.push(stmts);
        }
        let (program, diags) = kairos_vm::compile(&parsed);
        diagnostics.extend(diags.into_iter().map(|(i, d)| (files[i].clone(), d)));
        Ok(Project { root: root.to_path_buf(), manifest, program: Arc::new(program), diagnostics })
    }
}

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
