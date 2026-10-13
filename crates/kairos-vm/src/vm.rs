use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::Arc;

use kairos_script::{AssignOp, Speaker, Value};
use serde::{Deserialize, Serialize};

use crate::eval::{self, Scope};
use crate::persistent::{Persistent, is_persistent};
use crate::program::{Instr, Program};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    pub tag: String,
    pub attrs: Vec<String>,
    pub at: Option<String>,
    #[serde(default)]
    pub transform: SpriteTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpriteTransform {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub rotation: f32,
    pub alpha: f32,
}

impl Default for SpriteTransform {
    fn default() -> Self {
        SpriteTransform { x: 0.0, y: 0.0, scale: 1.0, rotation: 0.0, alpha: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageTween {
    pub tag: Option<String>,
    pub from: SpriteTransform,
    pub to: SpriteTransform,
    pub elapsed: f32,
    pub duration: f32,
    pub easing: kairos_script::Easing,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    pub background: Option<Vec<String>>,
    pub sprites: Vec<Sprite>,
    #[serde(default)]
    pub camera: Camera,
    #[serde(default)]
    pub tweens: Vec<StageTween>,
    #[serde(default)]
    pub shake: Option<(f32, f32, f32)>,
    #[serde(default)]
    pub flash: Option<(String, f32, f32)>,
}

#[derive(Debug, Clone, Copy)]
struct StageTarget {
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
    rotation: Option<f64>,
    alpha: Option<f64>,
    over: Option<f64>,
    easing: kairos_script::Easing,
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
    pub nvl_mode: bool,
    pub nvl_page: Vec<(Option<String>, String)>,
    /// Jump targets of the visible choices while a menu is waiting for input.
    pub menu: Option<Vec<usize>>,
    /// Random generator state. A `Cell` because expressions draw numbers while only reading
    /// the rest of the state.
    pub rng: Cell<u64>,
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
    ShowScreen(String),
    HideScreen(String),
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
    debug_overrides: BTreeMap<String, Value>,
    /// Snapshots taken at each interaction, newest last; the top is the current interaction.
    pub(crate) history: Vec<State>,
    /// Set by rollback: the restored state is already the top snapshot, so the next
    /// checkpoint must not push it twice.
    restored: bool,
    last_pc: usize,
    effects: Vec<Effect>,
    translations: BTreeMap<String, String>,
    persistent: Persistent,
    persistent_changed: bool,
    /// The dialogue line on screen; it counts as seen once the player advances past it.
    displayed: Option<usize>,
    line_seen: bool,
    pub(crate) relocated: bool,
}

struct Env<'a> {
    store: &'a BTreeMap<String, Value>,
    constants: &'a BTreeMap<String, Value>,
    debug_overrides: &'a BTreeMap<String, Value>,
    persistent: &'a BTreeMap<String, Value>,
    rng: &'a Cell<u64>,
}

static NONE: Value = Value::None;

fn valid_transform(transform: SpriteTransform) -> bool {
    transform.x.is_finite()
        && transform.y.is_finite()
        && transform.scale.is_finite()
        && transform.scale > 0.0
        && transform.rotation.is_finite()
        && transform.alpha.is_finite()
        && (0.0..=1.0).contains(&transform.alpha)
}

fn interpolate_transform(from: SpriteTransform, to: SpriteTransform, t: f32) -> SpriteTransform {
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    SpriteTransform {
        x: lerp(from.x, to.x),
        y: lerp(from.y, to.y),
        scale: lerp(from.scale, to.scale),
        rotation: lerp(from.rotation, to.rotation),
        alpha: lerp(from.alpha, to.alpha),
    }
}

impl Scope for Env<'_> {
    fn lookup(&self, name: &str) -> Option<&Value> {
        if is_persistent(name) {
            return Some(self.persistent.get(name).unwrap_or(&NONE));
        }
        self.store.get(name).or_else(|| self.debug_overrides.get(name)).or_else(|| self.constants.get(name))
    }

    /// splitmix64: small, fast and good enough for story randomness.
    fn random(&self) -> u64 {
        let state = self.rng.get().wrapping_add(0x9e37_79b9_7f4a_7c15);
        self.rng.set(state);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

impl Vm {
    pub fn new(program: Arc<Program>) -> Result<Self, RuntimeError> {
        let mut vm = Vm {
            program,
            state: State::default(),
            constants: BTreeMap::new(),
            debug_overrides: BTreeMap::new(),
            history: Vec::new(),
            restored: false,
            last_pc: 0,
            effects: Vec::new(),
            translations: BTreeMap::new(),
            persistent: Persistent::default(),
            persistent_changed: false,
            displayed: None,
            line_seen: false,
            relocated: false,
        };
        let program = vm.program.clone();
        for (name, expr) in &program.defines {
            let v = vm.eval(expr, 0)?;
            vm.constants.insert(name.clone(), v);
        }
        for (name, expr) in &program.defaults {
            let v = vm.eval(expr, 0)?;
            let store = if is_persistent(name) { &mut vm.persistent.values } else { &mut vm.state.store };
            store.insert(name.clone(), v);
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

    fn stage_sprite(&mut self, tag: &str, target: StageTarget) -> Result<(), RuntimeError> {
        let Some(sprite) = self.state.stage.sprites.iter().find(|sprite| sprite.tag == tag) else {
            return Err(self.error(format!("cannot transform hidden sprite `{tag}`")));
        };
        let from = sprite.transform;
        let mut to = from;
        if let Some(value) = target.x {
            to.x = value as f32;
        }
        if let Some(value) = target.y {
            to.y = value as f32;
        }
        if let Some(value) = target.scale {
            to.scale = value as f32;
        }
        if let Some(value) = target.rotation {
            to.rotation = value as f32;
        }
        if let Some(value) = target.alpha {
            to.alpha = value as f32;
        }
        if !valid_transform(to) {
            return Err(self.error("staging scale must be positive and alpha must be between 0 and 1"));
        }
        let duration = target.over.unwrap_or_default() as f32;
        self.state.stage.tweens.retain(|tween| tween.tag.as_deref() != Some(tag));
        if duration <= 0.0 {
            self.state.stage.sprites.iter_mut().find(|sprite| sprite.tag == tag).unwrap().transform = to;
        } else {
            self.state.stage.tweens.push(StageTween {
                tag: Some(tag.into()),
                from,
                to,
                elapsed: 0.0,
                duration,
                easing: target.easing,
            });
        }
        Ok(())
    }

    fn stage_camera(
        &mut self,
        x: Option<f64>,
        y: Option<f64>,
        zoom: Option<f64>,
        over: Option<f64>,
        easing: kairos_script::Easing,
    ) -> Result<(), RuntimeError> {
        let camera = self.state.stage.camera;
        let from = SpriteTransform { x: camera.x, y: camera.y, scale: camera.zoom, ..SpriteTransform::default() };
        let mut to = from;
        if let Some(value) = x {
            to.x = value as f32;
        }
        if let Some(value) = y {
            to.y = value as f32;
        }
        if let Some(value) = zoom {
            to.scale = value as f32;
        }
        if !to.scale.is_finite() || to.scale <= 0.0 {
            return Err(self.error("camera zoom must be positive"));
        }
        let duration = over.unwrap_or_default() as f32;
        self.state.stage.tweens.retain(|tween| tween.tag.is_some());
        if duration <= 0.0 {
            self.state.stage.camera = Camera { x: to.x, y: to.y, zoom: to.scale };
        } else {
            self.state.stage.tweens.push(StageTween { tag: None, from, to, elapsed: 0.0, duration, easing });
        }
        Ok(())
    }

    pub fn tick_stage(&mut self, dt: f32, snap: bool) {
        let dt = dt.max(0.0);
        for tween in &mut self.state.stage.tweens {
            tween.elapsed = if snap { tween.duration } else { (tween.elapsed + dt).min(tween.duration) };
            let t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);
            let t = match tween.easing {
                kairos_script::Easing::At => {
                    if t >= 1.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                kairos_script::Easing::Linear => t,
                kairos_script::Easing::Ease => t * t * (3.0 - 2.0 * t),
            };
            let value = interpolate_transform(tween.from, tween.to, t);
            if let Some(tag) = &tween.tag {
                if let Some(sprite) = self.state.stage.sprites.iter_mut().find(|sprite| &sprite.tag == tag) {
                    sprite.transform = value;
                }
            } else {
                self.state.stage.camera = Camera { x: value.x, y: value.y, zoom: value.scale };
            }
        }
        self.state.stage.tweens.retain(|tween| tween.elapsed < tween.duration);
        if let Some((duration, elapsed, _)) = &mut self.state.stage.shake {
            *elapsed = if snap { *duration } else { (*elapsed + dt).min(*duration) };
            if *elapsed >= *duration {
                self.state.stage.shake = None;
            }
        }
        if let Some((_, duration, elapsed)) = &mut self.state.stage.flash {
            *elapsed = if snap { *duration } else { (*elapsed + dt).min(*duration) };
            if *elapsed >= *duration {
                self.state.stage.flash = None;
            }
        }
        if let Some(current) = self.history.last_mut() {
            *current = self.state.clone();
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn variables(&self) -> BTreeMap<String, Value> {
        self.constants
            .iter()
            .chain(&self.debug_overrides)
            .chain(&self.state.store)
            .chain(&self.persistent.values)
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()
    }

    pub fn call_stack(&self) -> &[usize] {
        &self.state.stack
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn set_variable_path(&mut self, name: &str, indices: &[Value], value: Value) -> Result<(), RuntimeError> {
        let target = self.variable_mut(name, false)?;
        eval::assign_at(target, indices, AssignOp::Set, value).map_err(|message| self.error(message))
    }

    pub fn jump_to_label(&mut self, name: &str) -> Result<(), RuntimeError> {
        self.state.pc = self.label(name, self.state.pc)?;
        self.state.menu = None;
        Ok(())
    }

    pub fn call_label(&mut self, name: &str) -> Result<(), RuntimeError> {
        let target = self.label(name, self.state.pc)?;
        self.state.stack.push(self.state.pc.saturating_sub(1));
        self.state.pc = target;
        self.state.menu = None;
        Ok(())
    }

    pub fn set_variable(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        if is_persistent(name) {
            self.set_persistent_value(name, value);
            return Ok(());
        }
        if self.state.store.contains_key(name) {
            self.state.store.insert(name.to_string(), value);
            return Ok(());
        }
        if self.constants.contains_key(name) {
            self.debug_overrides.insert(name.to_string(), value);
            return Ok(());
        }
        Err(self.error(format!("unknown variable `{name}`")))
    }

    /// Like [`Vm::set_variable`], but also creates store entries, because play-from-here starts a
    /// fresh VM where variables first assigned with `$` do not exist yet.
    pub fn seed_variable(&mut self, name: &str, value: Value) {
        if is_persistent(name) {
            self.set_persistent_value(name, value);
        } else if self.constants.contains_key(name) && !self.state.store.contains_key(name) {
            self.debug_overrides.insert(name.to_string(), value);
        } else {
            self.state.store.insert(name.to_string(), value);
        }
    }

    /// Replaces the `default persistent.*` values with what an earlier session stored.
    pub fn set_persistent(&mut self, persistent: Persistent) {
        self.persistent.values.extend(persistent.values);
        self.persistent.seen = persistent.seen;
    }

    pub fn persistent(&self) -> &Persistent {
        &self.persistent
    }

    /// Whether persistent data changed since the last call, so the host knows to write it out.
    pub fn take_persistent_changed(&mut self) -> bool {
        std::mem::take(&mut self.persistent_changed)
    }

    /// Whether the player had already read the dialogue line most recently returned by
    /// [`Vm::advance`], in this playthrough or an earlier one.
    pub fn line_seen(&self) -> bool {
        self.line_seen
    }

    fn set_persistent_value(&mut self, name: &str, value: Value) {
        self.persistent.values.insert(name.to_string(), value);
        self.persistent_changed = true;
    }

    /// Seeds `random()`, `randint()` and `choice()`. Rollback and loads replay the same draws.
    pub fn seed_random(&mut self, seed: u64) {
        self.state.rng.set(seed);
    }

    pub fn rollback_to(&mut self, position: usize) -> bool {
        if position + 1 >= self.history.len() {
            return false;
        }
        self.history.truncate(position + 1);
        self.state = self.history[position].clone();
        self.restored = true;
        self.effects.clear();
        true
    }

    pub fn set_translations(&mut self, translations: BTreeMap<String, String>) {
        self.translations = translations;
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
        if let Some(pc) = self.displayed.take() {
            self.persistent_changed |= self.persistent.seen.insert(program.ids[pc].clone());
        }
        for _ in 0..STEP_LIMIT {
            let pc = self.state.pc;
            self.last_pc = pc;
            match &program.instrs[pc] {
                Instr::Say { speaker, text } => {
                    // Before interpolating, so rollback replays the same random draws.
                    self.checkpoint();
                    let id = &program.ids[pc];
                    let text = self.interpolate(text, id)?;
                    let (who, color) = self.speaker(speaker.as_ref(), id)?;
                    if matches!(speaker, Some(Speaker::Character(character)) if program.characters.get(character).is_some_and(|definition| definition.nvl))
                    {
                        self.state.nvl_mode = true;
                    }
                    if self.state.nvl_mode {
                        self.state.nvl_page.push((who.clone(), text.clone()));
                    }
                    self.line_seen = self.persistent.seen.contains(id);
                    self.displayed = Some(pc);
                    self.state.pc += 1;
                    return Ok(Event::Say { who, color, text });
                }
                Instr::Pause(d) => {
                    self.checkpoint();
                    self.state.pc += 1;
                    return Ok(Event::Pause(*d));
                }
                Instr::Nvl(action) => {
                    match action {
                        kairos_script::NvlAction::On => self.state.nvl_mode = true,
                        kairos_script::NvlAction::Off => self.state.nvl_mode = false,
                        kairos_script::NvlAction::Clear => self.state.nvl_page.clear(),
                    }
                    self.state.pc += 1;
                }
                Instr::StageSprite { tag, x, y, scale, rotation, alpha, over, easing } => {
                    self.stage_sprite(
                        tag,
                        StageTarget {
                            x: *x,
                            y: *y,
                            scale: *scale,
                            rotation: *rotation,
                            alpha: *alpha,
                            over: *over,
                            easing: *easing,
                        },
                    )?;
                    self.state.pc += 1;
                }
                Instr::Camera { x, y, zoom, over, easing } => {
                    self.stage_camera(*x, *y, *zoom, *over, *easing)?;
                    self.state.pc += 1;
                }
                Instr::ScreenShake { duration, amount } => {
                    self.state.stage.shake = (*duration > 0.0).then_some((*duration as f32, 0.0, *amount as f32));
                    self.state.pc += 1;
                }
                Instr::ScreenFlash { color, duration } => {
                    self.state.stage.flash = (*duration > 0.0).then_some((color.clone(), *duration as f32, 0.0));
                    self.state.pc += 1;
                }
                Instr::Menu { caption, choices } => {
                    self.checkpoint();
                    let id = &program.ids[pc];
                    let caption =
                        caption.as_ref().map(|(_, t)| self.interpolate(t, &format!("{id}#caption"))).transpose()?;
                    let mut labels = Vec::new();
                    let mut targets = Vec::new();
                    for (index, choice) in choices.iter().enumerate() {
                        let visible = match &choice.guard {
                            Some(g) => eval::truthy(&self.eval(g, pc)?),
                            None => true,
                        };
                        if visible {
                            labels.push(self.interpolate(&choice.text, &format!("{id}#choice:{index}"))?);
                            targets.push(choice.target);
                        }
                    }
                    if targets.is_empty() {
                        return Err(self.error("no menu choice is available"));
                    }
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
                Instr::Assign { name, index, op, value } => {
                    let rhs = self.eval(value, pc)?;
                    let keys = index.iter().map(|e| self.eval(e, pc)).collect::<Result<Vec<_>, _>>()?;
                    let target = self.variable_mut(name, keys.is_empty() && *op == AssignOp::Set)?;
                    eval::assign_at(target, &keys, *op, rhs).map_err(|m| self.error(m))?;
                    self.state.pc += 1;
                }
                Instr::Mutate { name, method, args } => {
                    let args = args.iter().map(|e| self.eval(e, pc)).collect::<Result<Vec<_>, _>>()?;
                    let target = self.variable_mut(name, false)?;
                    eval::mutate(target, method, args).map_err(|m| self.error(m))?;
                    self.state.pc += 1;
                }
                Instr::Scene { image, with } => {
                    self.state.stage = Stage { background: image.clone(), ..Stage::default() };
                    self.effects.push(Effect::Scene { image: image.clone(), with: with.clone() });
                    self.state.pc += 1;
                }
                Instr::Show { image, at, with } => {
                    let sprite = Sprite {
                        tag: image[0].clone(),
                        attrs: image[1..].to_vec(),
                        at: at.clone(),
                        transform: SpriteTransform::default(),
                    };
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
                Instr::ShowScreen(id) => {
                    self.effects.push(Effect::ShowScreen(id.clone()));
                    self.state.pc += 1;
                }
                Instr::HideScreen(id) => {
                    self.effects.push(Effect::HideScreen(id.clone()));
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

    /// The variable a statement writes to. `create` allows a plain `$ x = ...` to introduce a
    /// new variable; everything else must change one that exists.
    fn variable_mut(&mut self, name: &str, create: bool) -> Result<&mut Value, RuntimeError> {
        if is_persistent(name) {
            self.persistent_changed = true;
            return Ok(self.persistent.values.entry(name.to_string()).or_insert(Value::None));
        }
        if self.constants.contains_key(name) && !self.state.store.contains_key(name) {
            return Err(self.error(format!("`{name}` is a constant (define); use `default` for variables")));
        }
        if create {
            return Ok(self.state.store.entry(name.to_string()).or_insert(Value::None));
        }
        let missing = self.error(format!("`{name}` is not defined"));
        self.state.store.get_mut(name).ok_or(missing)
    }

    fn speaker(
        &self,
        speaker: Option<&Speaker>,
        statement_id: &str,
    ) -> Result<(Option<String>, Option<String>), RuntimeError> {
        match speaker {
            None => Ok((None, None)),
            Some(Speaker::Name(n)) => Ok((Some(n.clone()), None)),
            Some(Speaker::Character(id)) => match self.program.characters.get(id) {
                Some(c) => Ok((Some(self.interpolate(&c.name, &format!("{statement_id}#name"))?), c.color.clone())),
                None => Ok((Some(id.clone()), None)),
            },
        }
    }

    fn label(&self, name: &str, pc: usize) -> Result<usize, RuntimeError> {
        self.program.labels.get(name).copied().ok_or_else(|| self.error_at(pc, format!("unknown label `{name}`")))
    }

    fn env(&self) -> Env<'_> {
        Env {
            store: &self.state.store,
            constants: &self.constants,
            debug_overrides: &self.debug_overrides,
            persistent: &self.persistent.values,
            rng: &self.state.rng,
        }
    }

    fn eval(&self, expr: &kairos_script::Expr, pc: usize) -> Result<Value, RuntimeError> {
        eval::eval(expr, &self.env()).map_err(|m| self.error_at(pc, m))
    }

    fn interpolate(&self, text: &str, key: &str) -> Result<String, RuntimeError> {
        let source = self.translations.get(key).filter(|s| !s.is_empty()).map(String::as_str).unwrap_or(text);
        eval::interpolate(source, &self.env()).map_err(|m| self.error(m))
    }

    fn error(&self, message: impl Into<String>) -> RuntimeError {
        self.error_at(self.state.pc, message)
    }

    fn error_at(&self, pc: usize, message: impl Into<String>) -> RuntimeError {
        RuntimeError { line: self.program.lines.get(pc).copied().unwrap_or(0), message: message.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translations_use_statement_and_choice_ids_before_interpolation() {
        let (statements, diagnostics) = kairos_script::parse(
            r#"character e "Eileen"
default name = "Kai"
label start:
    e "Hello [name]"
    menu:
        "Continue [name]":
            return
"#,
        );
        assert!(diagnostics.is_empty());
        let (program, diagnostics) = crate::compile(&[statements]);
        assert!(diagnostics.is_empty());
        let say_id = program.ids[0].clone();
        let menu_id = program.ids[1].clone();
        let mut vm = Vm::new(Arc::new(program)).unwrap();
        vm.set_translations(BTreeMap::from([
            (say_id.clone(), "Bonjour [name]".into()),
            (format!("{say_id}#name"), "Émilie [name]".into()),
            (format!("{menu_id}#choice:0"), "Continuer [name]".into()),
        ]));
        vm.start("start").unwrap();
        assert_eq!(
            vm.advance().unwrap(),
            Event::Say { who: Some("Émilie Kai".into()), color: None, text: "Bonjour Kai".into() }
        );
        assert_eq!(vm.advance().unwrap(), Event::Menu { caption: None, choices: vec!["Continuer Kai".into()] });
    }

    #[test]
    fn debugger_accessors_edit_variables_and_restore_history_positions() {
        let (statements, diagnostics) =
            kairos_script::parse("define fixed = 1\ndefault count = 0\nlabel start:\n    \"First\"\n    \"Second\"\n");
        assert!(diagnostics.is_empty());
        let (program, diagnostics) = crate::compile(&[statements]);
        assert!(diagnostics.is_empty());
        let mut vm = Vm::new(Arc::new(program)).unwrap();
        vm.start("start").unwrap();
        assert_eq!(vm.variables()["count"], Value::Int(0));
        assert_eq!(vm.variables()["fixed"], Value::Int(1));
        vm.set_variable("count", Value::Int(4)).unwrap();
        vm.set_variable("fixed", Value::Int(2)).unwrap();
        assert_eq!(vm.variables()["fixed"], Value::Int(2));
        assert!(vm.set_variable("missing", Value::None).is_err());
        vm.advance().unwrap();
        vm.advance().unwrap();
        assert_eq!(vm.history_len(), 2);
        assert!(vm.rollback_to(0));
        assert_eq!(vm.current_line(), 4);
        assert_eq!(vm.history_len(), 1);
        assert!(!vm.rollback_to(1));
    }

    #[test]
    fn seeding_creates_assignment_variables_that_set_variable_rejects() {
        let (statements, _) = kairos_script::parse("define fixed = 1\nlabel start:\n    \"[visited]\"\n");
        let (program, _) = crate::compile(&[statements]);
        let mut vm = Vm::new(Arc::new(program)).unwrap();
        assert!(vm.set_variable("visited", Value::Bool(true)).is_err());
        vm.seed_variable("visited", Value::Bool(true));
        vm.seed_variable("fixed", Value::Int(2));
        assert_eq!(vm.variables()["visited"], Value::Bool(true));
        assert_eq!(vm.variables()["fixed"], Value::Int(2));
    }

    #[test]
    fn empty_translations_fall_back_to_source() {
        let (statements, _) = kairos_script::parse("label start:\n    \"Source\"\n");
        let (program, _) = crate::compile(&[statements]);
        let id = program.ids[0].clone();
        let mut vm = Vm::new(Arc::new(program)).unwrap();
        vm.set_translations(BTreeMap::from([(id, String::new())]));
        vm.start("start").unwrap();
        assert_eq!(vm.advance().unwrap(), Event::Say { who: None, color: None, text: "Source".into() });
    }
}
