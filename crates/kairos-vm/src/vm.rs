use std::collections::BTreeMap;
use std::sync::Arc;

use kairos_script::{Speaker, Value};
use serde::{Deserialize, Serialize};

use crate::eval::{self, Scope};
use crate::program::{Instr, Program};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    pub tag: String,
    pub attrs: Vec<String>,
    pub at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    pub background: Option<Vec<String>>,
    pub sprites: Vec<Sprite>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub asset: String,
    pub looped: bool,
}

/// Everything rollback and saves need. Constants (`define`) are not part of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub pc: usize,
    pub stack: Vec<usize>,
    pub store: BTreeMap<String, Value>,
    pub stage: Stage,
    pub audio: BTreeMap<String, Track>,
    /// Jump targets of the visible choices while a menu is waiting for input.
    pub menu: Option<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Say { who: Option<String>, color: Option<String>, text: String },
    Menu { caption: Option<String>, choices: Vec<String> },
    Pause(Option<f64>),
    OpenLoad,
    OpenSettings,
    Quit,
    End,
}

/// Presentation changes for a renderer or transcript. Rollback and load do not replay them;
/// a renderer redraws from [`State::stage`] instead.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Scene { image: Option<Vec<String>>, with: Option<String> },
    Show { sprite: Sprite, with: Option<String> },
    Hide { tag: String, with: Option<String> },
    Transition(String),
    Play { channel: String, asset: String, looped: bool, queue: bool, fadein: Option<f64> },
    Stop { channel: String, fadeout: Option<f64> },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct RuntimeError {
    pub line: usize,
    pub message: String,
}

const STEP_LIMIT: usize = 1_000_000;
const HISTORY_LIMIT: usize = 1_000;

pub struct Vm {
    pub(crate) program: Arc<Program>,
    pub(crate) state: State,
    constants: BTreeMap<String, Value>,
    /// Snapshots taken at each interaction, newest last; the top is the current interaction.
    pub(crate) history: Vec<State>,
    /// Set by rollback: the restored state is already the top snapshot, so the next
    /// checkpoint must not push it twice.
    restored: bool,
    last_pc: usize,
    effects: Vec<Effect>,
}

struct Env<'a> {
    store: &'a BTreeMap<String, Value>,
    constants: &'a BTreeMap<String, Value>,
}

impl Scope for Env<'_> {
    fn lookup(&self, name: &str) -> Option<&Value> {
        self.store.get(name).or_else(|| self.constants.get(name))
    }
}

impl Vm {
    pub fn new(program: Arc<Program>) -> Result<Self, RuntimeError> {
        let mut vm = Vm {
            program,
            state: State::default(),
            constants: BTreeMap::new(),
            history: Vec::new(),
            restored: false,
            last_pc: 0,
            effects: Vec::new(),
        };
        let program = vm.program.clone();
        for (name, expr) in &program.defines {
            let v = vm.eval(expr, 0)?;
            vm.constants.insert(name.clone(), v);
        }
        for (name, expr) in &program.defaults {
            let v = vm.eval(expr, 0)?;
            vm.state.store.insert(name.clone(), v);
        }
        Ok(vm)
    }

    pub fn start(&mut self, label: &str) -> Result<(), RuntimeError> {
        let pc = self.label(label, 0)?;
        self.start_at(pc);
        Ok(())
    }

    /// Starts at an instruction index; used by the editor's "play from this line".
    pub fn start_at(&mut self, pc: usize) {
        self.state.pc = pc;
        self.state.stack.clear();
        self.state.menu = None;
        self.history.clear();
        self.restored = false;
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn current_line(&self) -> usize {
        self.program.lines.get(self.state.pc).copied().unwrap_or(0)
    }

    pub fn last_pc(&self) -> usize {
        self.last_pc
    }

    /// Runs until the next interaction (dialogue, menu or pause) and returns it.
    pub fn advance(&mut self) -> Result<Event, RuntimeError> {
        if self.state.menu.is_some() {
            return Err(self.error("waiting for a menu choice"));
        }
        let program = self.program.clone();
        for _ in 0..STEP_LIMIT {
            let pc = self.state.pc;
            self.last_pc = pc;
            match &program.instrs[pc] {
                Instr::Say { speaker, text } => {
                    let text = self.interpolate(text)?;
                    let (who, color) = self.speaker(speaker.as_ref());
                    self.checkpoint();
                    self.state.pc += 1;
                    return Ok(Event::Say { who, color, text });
                }
                Instr::Pause(d) => {
                    self.checkpoint();
                    self.state.pc += 1;
                    return Ok(Event::Pause(*d));
                }
                Instr::Menu { caption, choices } => {
                    let caption = caption.as_ref().map(|(_, t)| self.interpolate(t)).transpose()?;
                    let mut labels = Vec::new();
                    let mut targets = Vec::new();
                    for choice in choices {
                        let visible = match &choice.guard {
                            Some(g) => eval::truthy(&self.eval(g, pc)?),
                            None => true,
                        };
                        if visible {
                            labels.push(self.interpolate(&choice.text)?);
                            targets.push(choice.target);
                        }
                    }
                    if targets.is_empty() {
                        return Err(self.error("no menu choice is available"));
                    }
                    self.checkpoint();
                    self.state.menu = Some(targets);
                    return Ok(Event::Menu { caption, choices: labels });
                }
                Instr::Goto(t) => self.state.pc = *t,
                Instr::GotoUnless(cond, t) => {
                    let pass = eval::truthy(&self.eval(cond, pc)?);
                    self.state.pc = if pass { pc + 1 } else { *t };
                }
                Instr::Jump(l) => self.state.pc = self.label(l, pc)?,
                Instr::Call(l) => {
                    let target = self.label(l, pc)?;
                    self.state.stack.push(pc);
                    self.state.pc = target;
                }
                Instr::Return => match self.state.stack.pop() {
                    Some(call) => self.state.pc = call + 1,
                    None => return Ok(Event::End),
                },
                Instr::OpenLoad | Instr::OpenSettings | Instr::Quit => {
                    self.checkpoint();
                    self.state.pc += 1;
                    return Ok(match &program.instrs[pc] {
                        Instr::OpenLoad => Event::OpenLoad,
                        Instr::OpenSettings => Event::OpenSettings,
                        Instr::Quit => Event::Quit,
                        _ => unreachable!(),
                    });
                }
                Instr::Assign { name, op, value } => {
                    let rhs = self.eval(value, pc)?;
                    if self.constants.contains_key(name) && !self.state.store.contains_key(name) {
                        return Err(self.error(format!("`{name}` is a constant (define); use `default` for variables")));
                    }
                    let v = eval::assign(*op, self.state.store.get(name), rhs).map_err(|m| self.error(m))?;
                    self.state.store.insert(name.clone(), v);
                    self.state.pc += 1;
                }
                Instr::Scene { image, with } => {
                    self.state.stage = Stage { background: image.clone(), sprites: Vec::new() };
                    self.effects.push(Effect::Scene { image: image.clone(), with: with.clone() });
                    self.state.pc += 1;
                }
                Instr::Show { image, at, with } => {
                    let sprite = Sprite { tag: image[0].clone(), attrs: image[1..].to_vec(), at: at.clone() };
                    let sprites = &mut self.state.stage.sprites;
                    match sprites.iter_mut().find(|s| s.tag == sprite.tag) {
                        Some(existing) => {
                            let at = sprite.at.clone().or(existing.at.take());
                            *existing = Sprite { at, ..sprite.clone() };
                        }
                        None => sprites.push(sprite.clone()),
                    }
                    self.effects.push(Effect::Show { sprite, with: with.clone() });
                    self.state.pc += 1;
                }
                Instr::Hide { tag, with } => {
                    self.state.stage.sprites.retain(|s| &s.tag != tag);
                    self.effects.push(Effect::Hide { tag: tag.clone(), with: with.clone() });
                    self.state.pc += 1;
                }
                Instr::With(t) => {
                    self.effects.push(Effect::Transition(t.clone()));
                    self.state.pc += 1;
                }
                Instr::Play { channel, asset, looped, queue, fadein } => {
                    if !*queue || !self.state.audio.contains_key(channel) {
                        self.state.audio.insert(channel.clone(), Track { asset: asset.clone(), looped: *looped });
                    }
                    self.effects.push(Effect::Play {
                        channel: channel.clone(),
                        asset: asset.clone(),
                        looped: *looped,
                        queue: *queue,
                        fadein: *fadein,
                    });
                    self.state.pc += 1;
                }
                Instr::Stop { channel, fadeout } => {
                    self.state.audio.remove(channel);
                    self.effects.push(Effect::Stop { channel: channel.clone(), fadeout: *fadeout });
                    self.state.pc += 1;
                }
            }
        }
        Err(self.error("script ran too long without an interaction; is there an infinite loop?"))
    }

    /// Picks a visible menu choice by its index in the last [`Event::Menu`].
    pub fn choose(&mut self, index: usize) -> Result<(), RuntimeError> {
        let targets = self.state.menu.as_ref().ok_or_else(|| self.error("no menu is waiting for a choice"))?;
        let target = *targets.get(index).ok_or_else(|| self.error(format!("choice {} does not exist", index + 1)))?;
        self.state.menu = None;
        self.state.pc = target;
        Ok(())
    }

    /// Steps back to the previous interaction. The next [`Vm::advance`] shows it again.
    pub fn rollback(&mut self) -> bool {
        if self.history.len() < 2 {
            return false;
        }
        self.history.pop();
        self.state = self.history.last().expect("checked length").clone();
        self.restored = true;
        self.effects.clear();
        true
    }

    pub fn can_rollback(&self) -> bool {
        self.history.len() >= 2
    }

    fn checkpoint(&mut self) {
        if std::mem::take(&mut self.restored) {
            return;
        }
        if self.history.len() == HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(self.state.clone());
    }

    fn speaker(&self, speaker: Option<&Speaker>) -> (Option<String>, Option<String>) {
        match speaker {
            None => (None, None),
            Some(Speaker::Name(n)) => (Some(n.clone()), None),
            Some(Speaker::Character(id)) => match self.program.characters.get(id) {
                Some(c) => (Some(c.name.clone()), c.color.clone()),
                None => (Some(id.clone()), None),
            },
        }
    }

    fn label(&self, name: &str, pc: usize) -> Result<usize, RuntimeError> {
        self.program.labels.get(name).copied().ok_or_else(|| self.error_at(pc, format!("unknown label `{name}`")))
    }

    fn env(&self) -> Env<'_> {
        Env { store: &self.state.store, constants: &self.constants }
    }

    fn eval(&self, expr: &kairos_script::Expr, pc: usize) -> Result<Value, RuntimeError> {
        eval::eval(expr, &self.env()).map_err(|m| self.error_at(pc, m))
    }

    fn interpolate(&self, text: &str) -> Result<String, RuntimeError> {
        eval::interpolate(text, &self.env()).map_err(|m| self.error(m))
    }

    fn error(&self, message: impl Into<String>) -> RuntimeError {
        self.error_at(self.state.pc, message)
    }

    fn error_at(&self, pc: usize, message: impl Into<String>) -> RuntimeError {
        RuntimeError { line: self.program.lines.get(pc).copied().unwrap_or(0), message: message.into() }
    }
}
