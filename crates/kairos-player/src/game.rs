//! The player's input state machine on top of the VM. No graphics; the host draws from
//! [`Game::shown`] and [`Game::stage`] and acts on [`Signal`]s.

use std::collections::BTreeMap;
use std::sync::Arc;

use kairos_vm::{Effect, Event, LoadError, Persistent, Program, RuntimeError, Stage, Vm};

#[derive(Debug, Clone, PartialEq)]
pub enum Shown {
    Nothing,
    Say { who: Option<String>, color: Option<String>, text: String, revealed: f32, len: usize, waited: f32 },
    Menu { caption: Option<String>, choices: Vec<String> },
    Pause { remaining: Option<f32> },
    OpenLoad,
    OpenSettings,
    Quit,
    End,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Auto,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Game,
    Title,
    Menu,
    Save,
    Load,
    Backlog,
    Settings,
}

/// Things the host must do: start a transition, drive audio, or redraw and resync audio from
/// scratch because the stage was replaced (start, rollback, load).
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    Transition(String),
    Audio(Effect),
    Resync,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub who: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Characters per second; [`Settings::INSTANT`] or more shows lines at once.
    pub text_speed: f32,
    /// Seconds auto mode waits after a line is fully shown, plus a little per character.
    pub auto_delay: f32,
    pub music: f32,
    pub sound: f32,
    pub voice: f32,
    /// Skip runs through lines the player has never read instead of stopping at them.
    pub skip_unseen: bool,
}

impl Settings {
    pub const INSTANT: f32 = 200.0;
}

impl Default for Settings {
    fn default() -> Self {
        Settings { text_speed: 45.0, auto_delay: 1.2, music: 0.8, sound: 0.8, voice: 0.8, skip_unseen: false }
    }
}

const SKIP_INTERVAL: f32 = 0.06;

pub struct Game {
    program: Arc<Program>,
    vm: Vm,
    pub shown: Shown,
    pub mode: Mode,
    pub screen: Screen,
    pub settings: Settings,
    language: String,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    backlog: Vec<Line>,
    /// Backlog length before each shown interaction, parallel to the VM's rollback history.
    marks: Vec<usize>,
    signals: Vec<Signal>,
    skip_timer: f32,
}

impl Game {
    pub fn new(program: Arc<Program>) -> Result<Game, RuntimeError> {
        Ok(Game {
            vm: Vm::new(program.clone())?,
            program,
            shown: Shown::Nothing,
            mode: Mode::Normal,
            screen: Screen::Game,
            settings: Settings::default(),
            language: String::new(),
            translations: BTreeMap::new(),
            backlog: Vec::new(),
            marks: Vec::new(),
            signals: Vec::new(),
            skip_timer: 0.0,
        })
    }

    pub fn start(&mut self, label: &str) {
        match self.program.labels.get(label) {
            Some(&pc) => self.start_at(pc),
            None => self.shown = Shown::Error(format!("the start label `{label}` does not exist")),
        }
    }

    pub fn has_label(&self, name: &str) -> bool {
        self.program.labels.contains_key(name)
    }

    pub fn show_title(&mut self) {
        self.shown = Shown::Nothing;
        self.reset();
        self.screen = Screen::Title;
    }

    /// Starts with fresh variables at an instruction, for "play from this line".
    pub fn start_at(&mut self, pc: usize) {
        if let Err(e) = self.start_at_with_overrides(pc, &BTreeMap::new()) {
            self.shown = Shown::Error(e.to_string());
        }
    }

    pub fn start_at_with_overrides(
        &mut self,
        pc: usize,
        overrides: &BTreeMap<String, kairos_vm::Value>,
    ) -> Result<(), RuntimeError> {
        match Vm::new(self.program.clone()) {
            Ok(mut vm) => {
                vm.set_persistent(self.vm.persistent().clone());
                vm.seed_random(crate::saves::timestamp() as u64);
                vm.set_translations(self.translations.get(&self.language).cloned().unwrap_or_default());
                for (name, value) in overrides {
                    vm.seed_variable(name, value.clone());
                }
                self.vm = vm;
            }
            Err(e) => {
                return Err(e);
            }
        }
        self.vm.start_at(pc);
        self.reset();
        self.signals.push(Signal::Resync);
        self.step();
        Ok(())
    }

    /// Plays from `start` until the first interaction at or after `target`, so the editor can show
    /// a scene's state at any line. Menus pick the choice that leads toward `target`.
    pub fn seek_to(&mut self, start: usize, target: usize) {
        let speed = self.settings.text_speed;
        self.start_at(start);
        self.settings.text_speed = Settings::INSTANT;
        for _ in 0..500 {
            let at = self.vm.last_pc();
            match &self.shown {
                Shown::Say { .. } | Shown::Pause { .. } if at < target => self.step(),
                Shown::Menu { .. } if at < target => {
                    let kairos_vm::Instr::Menu { choices, .. } = &self.program.instrs[at] else { break };
                    let pick = choices.iter().rposition(|c| c.target <= target).unwrap_or(0);
                    self.choose(pick);
                }
                _ => {
                    self.settings.text_speed = speed;
                    return;
                }
            }
        }
        self.start_at(target);
        self.settings.text_speed = speed;
    }

    pub fn program(&self) -> &Arc<Program> {
        &self.program
    }

    pub fn stage(&self) -> &Stage {
        &self.vm.state().stage
    }

    pub fn audio_state(&self) -> &std::collections::BTreeMap<String, kairos_vm::Track> {
        &self.vm.state().audio
    }

    pub fn backlog(&self) -> &[Line] {
        &self.backlog
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn language_label(&self) -> &str {
        if self.language.is_empty() { "Source" } else { &self.language }
    }

    pub fn languages(&self) -> Vec<String> {
        std::iter::once(String::new()).chain(self.translations.keys().cloned()).collect()
    }

    pub fn set_localizations(&mut self, translations: BTreeMap<String, BTreeMap<String, String>>, language: &str) {
        self.translations = translations;
        self.set_language(language);
    }

    pub fn set_language(&mut self, language: &str) -> bool {
        if !language.is_empty() && !self.translations.contains_key(language) {
            return false;
        }
        self.language = language.to_string();
        self.vm.set_translations(self.translations.get(language).cloned().unwrap_or_default());
        true
    }

    pub fn cycle_language(&mut self, direction: i8) {
        let languages = self.languages();
        let index = languages.iter().position(|language| language == &self.language).unwrap_or(0);
        let next = (index as isize + direction.signum() as isize).rem_euclid(languages.len() as isize) as usize;
        self.set_language(&languages[next]);
    }

    pub fn can_rollback(&self) -> bool {
        self.vm.can_rollback()
    }

    pub fn debug_variables(&self) -> std::collections::BTreeMap<String, kairos_vm::Value> {
        self.vm.variables()
    }

    pub fn debug_pc(&self) -> usize {
        self.vm.last_pc()
    }

    pub fn debug_call_stack(&self) -> &[usize] {
        self.vm.call_stack()
    }

    pub fn debug_history_len(&self) -> usize {
        self.vm.history_len()
    }

    pub fn debug_set_variable(&mut self, name: &str, value: kairos_vm::Value) -> Result<(), RuntimeError> {
        self.vm.set_variable(name, value)
    }

    pub fn debug_rollback_to(&mut self, position: usize) -> bool {
        if !self.vm.rollback_to(position) {
            return false;
        }
        self.backlog.truncate(*self.marks.get(position).unwrap_or(&0));
        self.marks.truncate(position);
        self.mode = Mode::Normal;
        self.signals.push(Signal::Resync);
        self.step();
        if let Shown::Say { revealed, len, .. } = &mut self.shown {
            *revealed = *len as f32;
        }
        true
    }

    pub fn take_signals(&mut self) -> Vec<Signal> {
        std::mem::take(&mut self.signals)
    }

    /// Click, space or enter: finishes the typewriter first, then moves on.
    pub fn proceed(&mut self) {
        if self.mode == Mode::Skip {
            self.mode = Mode::Normal;
        }
        match &mut self.shown {
            Shown::Say { revealed, len, .. } if (*revealed as usize) < *len => *revealed = *len as f32,
            Shown::Say { .. } | Shown::Pause { .. } | Shown::OpenLoad | Shown::OpenSettings | Shown::Quit => {
                self.step()
            }
            _ => {}
        }
    }

    pub fn choose(&mut self, index: usize) {
        let Shown::Menu { choices, .. } = &self.shown else { return };
        let Some(text) = choices.get(index).cloned() else { return };
        if let Err(e) = self.vm.choose(index) {
            self.shown = Shown::Error(e.to_string());
            return;
        }
        self.backlog.push(Line { who: None, text: format!("> {text}") });
        self.step();
    }

    /// Steps back one interaction and shows it again, fully revealed.
    pub fn rollback(&mut self) -> bool {
        if !self.vm.rollback() {
            return false;
        }
        self.mode = Mode::Normal;
        self.marks.pop();
        if let Some(mark) = self.marks.pop() {
            self.backlog.truncate(mark);
        }
        self.signals.push(Signal::Resync);
        self.step();
        if let Shown::Say { revealed, len, .. } = &mut self.shown {
            *revealed = *len as f32;
        }
        true
    }

    pub fn save(&self) -> Option<Vec<u8>> {
        if matches!(self.vm.program().label_at(self.vm.last_pc()), Some("splashscreen" | "main_menu")) {
            return None;
        }
        self.vm.save()
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<(), LoadError> {
        let mut vm = Vm::load(self.program.clone(), bytes)?;
        vm.set_persistent(self.vm.persistent().clone());
        self.vm = vm;
        self.vm.set_translations(self.translations.get(&self.language).cloned().unwrap_or_default());
        self.reset();
        self.signals.push(Signal::Resync);
        self.step();
        Ok(())
    }

    /// Whether the last [`Game::load`] resumed near the saved line because a patch changed it.
    pub fn load_relocated(&self) -> bool {
        self.vm.relocated()
    }

    pub fn set_persistent(&mut self, persistent: Persistent) {
        self.vm.set_persistent(persistent);
    }

    /// Persistent data to write out, if it changed since the last call.
    pub fn take_persistent(&mut self) -> Option<Vec<u8>> {
        self.vm.take_persistent_changed().then(|| self.vm.persistent().to_bytes())
    }

    pub fn toggle(&mut self, mode: Mode) {
        self.mode = if self.mode == mode { Mode::Normal } else { mode };
        self.skip_timer = 0.0;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.screen != Screen::Game {
            return;
        }
        if self.mode == Mode::Skip {
            self.skip_timer += dt;
            if matches!(self.shown, Shown::Say { .. }) && !self.settings.skip_unseen && !self.vm.line_seen() {
                self.mode = Mode::Normal;
                return;
            }
            match self.shown {
                Shown::Say { .. } | Shown::Pause { .. } if self.skip_timer >= SKIP_INTERVAL => {
                    self.skip_timer = 0.0;
                    self.step();
                }
                Shown::Say { .. } | Shown::Pause { .. } => {}
                _ => self.mode = Mode::Normal,
            }
            return;
        }
        let speed = self.settings.text_speed;
        let auto = self.mode == Mode::Auto;
        let delay = self.settings.auto_delay;
        let next = match &mut self.shown {
            Shown::Say { revealed, len, waited, .. } => {
                if speed >= Settings::INSTANT {
                    *revealed = *len as f32;
                } else if (*revealed as usize) < *len {
                    *revealed = (*revealed + speed * dt).min(*len as f32);
                } else {
                    *waited += dt;
                }
                auto && (*revealed as usize) >= *len && *waited >= delay + *len as f32 * 0.02
            }
            Shown::Pause { remaining: Some(r) } => {
                *r -= dt;
                *r <= 0.0
            }
            Shown::Pause { remaining: None } => auto,
            _ => false,
        };
        if next {
            self.step();
        }
    }

    fn reset(&mut self) {
        self.backlog.clear();
        self.marks.clear();
        self.mode = Mode::Normal;
        self.screen = Screen::Game;
    }

    fn step(&mut self) {
        let event = self.vm.advance();
        let mut transition = None;
        for effect in self.vm.take_effects() {
            match effect {
                Effect::Scene { with, .. } | Effect::Show { with, .. } | Effect::Hide { with, .. } => {
                    transition = with.or(transition);
                }
                Effect::Transition(t) => transition = Some(t),
                audio @ (Effect::Play { .. } | Effect::Stop { .. }) => self.signals.push(Signal::Audio(audio)),
            }
        }
        if let Some(t) = transition.filter(|_| self.mode != Mode::Skip) {
            self.signals.push(Signal::Transition(t));
        }
        let event = match event {
            Ok(e) => e,
            Err(e) => {
                self.shown = Shown::Error(e.to_string());
                return;
            }
        };
        if !matches!(event, Event::End) {
            self.marks.push(self.backlog.len());
        }
        self.shown = match event {
            Event::Say { who, color, text } => {
                self.backlog.push(Line { who: who.clone(), text: text.clone() });
                let len = text.chars().count();
                let revealed = if self.settings.text_speed >= Settings::INSTANT { len as f32 } else { 0.0 };
                Shown::Say { who, color, text, revealed, len, waited: 0.0 }
            }
            Event::Menu { caption, choices } => Shown::Menu { caption, choices },
            Event::Pause(d) => Shown::Pause { remaining: d.map(|d| d as f32) },
            Event::OpenLoad => Shown::OpenLoad,
            Event::OpenSettings => Shown::OpenSettings,
            Event::Quit => Shown::Quit,
            Event::End => Shown::End,
        };
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const SCRIPT: &str = r##"
character e "Eileen" color="#c8ffc8"
default n = 0

label start:
    scene bg room with fade
    play music "theme.ogg" loop
    show e happy at center
    e "Hello there."
    "Second line."
    menu:
        "Left":
            $ n += 1
            e "You went left."
        "Right":
            e "You went right."
    pause 0.5
    e "Done."
    return
"##;

    fn game() -> Game {
        let project =
            kairos_project::Project::from_sources("name = \"t\"", [(PathBuf::from("script/a.kai"), SCRIPT.into())])
                .unwrap();
        assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
        let mut game = Game::new(project.program).unwrap();
        game.start("start");
        game
    }

    fn text(game: &Game) -> &str {
        match &game.shown {
            Shown::Say { text, .. } => text,
            other => panic!("expected a line, got {other:?}"),
        }
    }

    fn revealed(game: &Game) -> usize {
        match &game.shown {
            Shown::Say { revealed, .. } => *revealed as usize,
            other => panic!("expected a line, got {other:?}"),
        }
    }

    fn to_menu(game: &mut Game) {
        game.proceed();
        game.proceed();
        game.proceed();
        game.proceed();
        assert!(matches!(game.shown, Shown::Menu { .. }), "{:?}", game.shown);
    }

    #[test]
    fn debug_overrides_apply_before_start_and_history_rolls_back_to_an_exact_position() {
        let project = kairos_project::Project::from_sources(
            "name = \"t\"",
            [(
                PathBuf::from("script/a.kai"),
                r#"default n = 0
label start:
    "Value [n]"
    "Next"
"#
                .into(),
            )],
        )
        .unwrap();
        let mut game = Game::new(project.program).unwrap();
        let overrides = BTreeMap::from([("n".into(), kairos_vm::Value::Int(3))]);
        game.start_at_with_overrides(0, &overrides).unwrap();
        assert_eq!(text(&game), "Value 3");
        game.proceed();
        game.proceed();
        assert_eq!(text(&game), "Next");
        assert!(game.debug_rollback_to(0));
        assert_eq!(text(&game), "Value 3");
        assert_eq!(game.debug_history_len(), 1);
        assert!(game.debug_set_variable("missing", kairos_vm::Value::None).is_err());
    }

    #[test]
    fn start_reports_the_opening_transition_and_music() {
        let mut game = game();
        let signals = game.take_signals();
        assert_eq!(signals[0], Signal::Resync);
        assert!(signals.contains(&Signal::Transition("fade".into())));
        assert!(
            signals
                .iter()
                .any(|s| matches!(s, Signal::Audio(Effect::Play { asset, looped: true, .. }) if asset == "theme.ogg"))
        );
        assert_eq!(game.stage().background.as_deref(), Some(&["bg".to_string(), "room".to_string()][..]));
    }

    #[test]
    fn the_typewriter_reveals_over_time() {
        let mut game = game();
        assert_eq!(revealed(&game), 0);
        game.tick(0.1);
        assert_eq!(revealed(&game), 4);
        game.tick(10.0);
        assert_eq!(revealed(&game), "Hello there.".len());
    }

    #[test]
    fn proceeding_finishes_the_typewriter_before_advancing() {
        let mut game = game();
        game.proceed();
        assert_eq!(text(&game), "Hello there.");
        assert_eq!(revealed(&game), "Hello there.".len());
        game.proceed();
        assert_eq!(text(&game), "Second line.");
        assert_eq!(revealed(&game), 0);
    }

    #[test]
    fn instant_text_speed_skips_the_typewriter() {
        let mut game = game();
        game.settings.text_speed = Settings::INSTANT;
        game.proceed();
        game.proceed();
        assert_eq!(text(&game), "Second line.");
        assert_eq!(revealed(&game), "Second line.".len());
    }

    #[test]
    fn rollback_shows_the_previous_line_fully_and_trims_the_backlog() {
        let mut game = game();
        assert!(!game.rollback());
        game.proceed();
        game.proceed();
        assert_eq!(game.backlog().len(), 2);
        game.take_signals();
        assert!(game.rollback());
        assert_eq!(text(&game), "Hello there.");
        assert_eq!(revealed(&game), "Hello there.".len());
        assert_eq!(game.backlog().len(), 1);
        assert_eq!(game.take_signals(), [Signal::Resync]);
    }

    #[test]
    fn choices_jump_to_their_branch_and_enter_the_backlog() {
        let mut game = game();
        to_menu(&mut game);
        assert_eq!(game.shown, Shown::Menu { caption: None, choices: vec!["Left".into(), "Right".into()] });
        game.proceed();
        assert!(matches!(game.shown, Shown::Menu { .. }), "proceed must not leave a menu");
        game.choose(1);
        assert_eq!(text(&game), "You went right.");
        let lines: Vec<&str> = game.backlog().iter().map(|l| l.text.as_str()).collect();
        assert_eq!(lines, ["Hello there.", "Second line.", "> Right", "You went right."]);
    }

    #[test]
    fn rolling_back_out_of_a_branch_returns_to_the_menu() {
        let mut game = game();
        to_menu(&mut game);
        game.choose(0);
        assert!(game.rollback());
        assert!(matches!(game.shown, Shown::Menu { .. }));
        assert_eq!(game.backlog().len(), 2);
        game.choose(1);
        assert_eq!(text(&game), "You went right.");
    }

    #[test]
    fn timed_pauses_end_by_themselves() {
        let mut game = game();
        to_menu(&mut game);
        game.choose(0);
        game.proceed();
        game.proceed();
        assert_eq!(game.shown, Shown::Pause { remaining: Some(0.5) });
        game.tick(0.3);
        assert!(matches!(game.shown, Shown::Pause { .. }));
        game.tick(0.3);
        assert_eq!(text(&game), "Done.");
    }

    #[test]
    fn auto_mode_advances_after_the_line_is_shown_and_read() {
        let mut game = game();
        game.toggle(Mode::Auto);
        game.tick(1.0);
        assert_eq!(text(&game), "Hello there.");
        game.tick(5.0);
        assert_eq!(text(&game), "Second line.");
    }

    #[test]
    fn skip_mode_runs_to_the_next_menu_and_stops() {
        let mut game = game();
        game.settings.skip_unseen = true;
        game.toggle(Mode::Skip);
        for _ in 0..10 {
            game.tick(0.1);
        }
        assert!(matches!(game.shown, Shown::Menu { .. }));
        assert_eq!(game.mode, Mode::Normal);
    }

    #[test]
    fn skip_stops_at_unread_lines_until_a_playthrough_has_read_them() {
        let mut first = game();
        assert!(first.take_persistent().is_none());
        first.toggle(Mode::Skip);
        first.tick(0.1);
        assert_eq!(text(&first), "Hello there.");
        assert_eq!(first.mode, Mode::Normal);

        to_menu(&mut first);
        let saved = first.take_persistent().expect("reading lines changes persistent data");
        let mut second = Game::new(first.program.clone()).unwrap();
        second.set_persistent(Persistent::from_bytes(&saved).unwrap());
        second.start("start");
        second.toggle(Mode::Skip);
        for _ in 0..10 {
            second.tick(0.1);
        }
        assert!(matches!(second.shown, Shown::Menu { .. }), "{:?}", second.shown);
    }

    #[test]
    fn open_menus_pause_the_game() {
        let mut game = game();
        game.toggle(Mode::Auto);
        game.screen = Screen::Settings;
        game.tick(10.0);
        assert_eq!(revealed(&game), 0);
    }

    #[test]
    fn save_and_load_restore_the_line_and_variables() {
        let mut game = game();
        to_menu(&mut game);
        game.choose(0);
        let bytes = game.save().unwrap();
        let mut other = Game::new(game.program.clone()).unwrap();
        other.load(&bytes).unwrap();
        assert_eq!(text(&other), "You went left.");
        assert_eq!(other.stage(), game.stage());
        assert_eq!(other.take_signals(), [Signal::Resync]);
        other.proceed();
        other.proceed();
        other.tick(1.0);
        assert_eq!(text(&other), "Done.");
    }

    #[test]
    fn the_end_is_reported_and_proceeding_past_it_does_nothing() {
        let mut game = game();
        game.settings.text_speed = Settings::INSTANT;
        to_menu(&mut game);
        game.choose(0);
        game.proceed();
        game.tick(1.0);
        game.proceed();
        assert_eq!(game.shown, Shown::End);
        game.proceed();
        assert_eq!(game.shown, Shown::End);
    }

    #[test]
    fn open_statements_pause_for_host_screens_then_continue() {
        let source = "label start:\n    open settings\n    open load\n    quit\n    \"Done\"\n";
        let project =
            kairos_project::Project::from_sources("name = \"t\"", [(PathBuf::from("script/a.kai"), source.into())])
                .unwrap();
        let mut game = Game::new(project.program).unwrap();
        game.start("start");
        assert_eq!(game.shown, Shown::OpenSettings);
        game.proceed();
        assert_eq!(game.shown, Shown::OpenLoad);
        game.proceed();
        assert_eq!(game.shown, Shown::Quit);
        game.proceed();
        assert_eq!(text(&game), "Done");
    }

    #[test]
    fn saving_stays_disabled_in_splashscreen_and_main_menu_until_jump_to_start() {
        let source = "label splashscreen:\n    \"Splash\"\n    return\nlabel main_menu:\n    menu:\n        \"Start\":\n            jump start\nlabel start:\n    \"Game\"\n";
        let project =
            kairos_project::Project::from_sources("name = \"t\"", [(PathBuf::from("script/a.kai"), source.into())])
                .unwrap();
        let mut game = Game::new(project.program).unwrap();
        game.start("splashscreen");
        assert!(game.save().is_none());
        game.proceed();
        game.proceed();
        game.start("main_menu");
        assert!(game.save().is_none());
        game.choose(0);
        assert_eq!(text(&game), "Game");
        assert!(game.save().is_some());
    }

    #[test]
    fn saving_remains_disabled_while_a_main_menu_screen_is_open() {
        let source = "label main_menu:\n    open settings\nlabel start:\n    \"Game\"\n";
        let project =
            kairos_project::Project::from_sources("name = \"t\"", [(PathBuf::from("script/a.kai"), source.into())])
                .unwrap();
        let mut game = Game::new(project.program).unwrap();
        game.start("main_menu");
        assert_eq!(game.shown, Shown::OpenSettings);
        assert!(game.save().is_none());
        game.proceed();
        assert_eq!(text(&game), "Game");
        assert!(game.save().is_some());
    }

    #[test]
    fn language_selection_changes_runtime_text_and_can_be_restored_from_preferences() {
        let project =
            kairos_project::Project::from_sources("name = \"t\"", [(PathBuf::from("script/a.kai"), SCRIPT.into())])
                .unwrap();
        let mut translations = BTreeMap::new();
        translations.insert(
            "ja".to_string(),
            project
                .translation_entries()
                .into_iter()
                .map(|(key, source)| {
                    let value = if source == "Hello there." { "こんにちは" } else { "" };
                    (key, value.to_string())
                })
                .collect(),
        );
        let mut game = Game::new(project.program).unwrap();
        game.set_localizations(translations, "ja");
        game.start("start");
        assert_eq!(game.language_label(), "ja");
        assert_eq!(text(&game), "こんにちは");
        game.cycle_language(1);
        assert_eq!(game.language(), "");
        game.set_localizations(BTreeMap::from([("ja".to_string(), BTreeMap::new())]), "ja");
        assert_eq!(game.language_label(), "ja");
    }

    #[test]
    fn a_missing_start_label_is_shown_as_an_error() {
        let mut game = game();
        game.start("nowhere");
        assert!(matches!(&game.shown, Shown::Error(m) if m.contains("nowhere")));
    }

    #[test]
    fn seek_plays_up_to_a_line_keeping_the_stage_built_before_it() {
        let source = "label start:\n    scene bg room\n    show e happy\n    \"One\"\n    menu:\n        \"A\":\n            \"In A\"\n        \"B\":\n            show e sad\n            \"In B\"\n";
        let project =
            kairos_project::Project::from_sources("name = \"t\"", [(PathBuf::from("script/a.kai"), source.into())])
                .unwrap();
        let mut game = Game::new(project.program.clone()).unwrap();
        let program = project.program.clone();
        let target = program.pc_at(0, 10).unwrap();
        game.seek_to(program.labels["start"], target);
        assert_eq!(text(&game), "In B");
        assert_eq!(game.stage().background.as_deref(), Some(&["bg".to_string(), "room".to_string()][..]));
        assert_eq!(game.stage().sprites[0].attrs, ["sad"]);
    }
}
