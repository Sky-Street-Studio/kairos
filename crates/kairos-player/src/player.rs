//! Ties the game, renderer, audio and assets together. Platform shells feed it input and frames.

use std::collections::HashMap;

use kairos_audio::{Audio, Bus, Sound};
use kairos_project::{
    AudioLoop, AudioMetadata, Project, parse_audio_metadata,
    screens::{self, ActionType, BindingPart, ScreenAction},
};
use kairos_render::{Fonts, Renderer, TransitionKind};
use kairos_vm::{Effect, Persistent, PersistentError, Value};

use crate::assets::{Assets, Loaded, Request};
use crate::game::{Game, Screen, Shown, Signal};
use crate::saves::SaveStore;
use crate::theme::{Theme, Title};
use crate::view::{self, Action, Hotspot, ImageInfo, SLOTS, ViewInput, image_name};

const PERSISTENT_INTERVAL: f32 = 2.0;

fn apply_set_value(current: &mut Value, value: Value, path: &str) -> Result<(), String> {
    if std::mem::discriminant(current) != std::mem::discriminant(&value) {
        return Err(format!("set value type does not match target binding `{path}`"));
    }
    *current = value;
    Ok(())
}

fn toml_value(value: toml::Value) -> Option<Value> {
    Some(match value {
        toml::Value::Boolean(value) => Value::Bool(value),
        toml::Value::Integer(value) => Value::Int(value),
        toml::Value::Float(value) => Value::Float(value),
        toml::Value::String(value) => Value::Str(value),
        toml::Value::Array(values) => Value::List(values.into_iter().map(toml_value).collect::<Option<_>>()?),
        toml::Value::Table(values) => {
            Value::Dict(values.into_iter().map(|(key, value)| Some((key, toml_value(value)?))).collect::<Option<_>>()?)
        }
        toml::Value::Datetime(_) => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Advance,
    Escape,
    Back,
    Forward,
    FocusNext,
    FocusPrevious,
    Digit(u8),
    Auto,
    Skip,
}

pub struct Player {
    game: Game,
    screens: std::collections::BTreeMap<String, kairos_project::screens::ScreenDefinition>,
    start_label: String,
    project_name: String,
    title: Title,
    title_text: String,
    in_splashscreen: bool,
    theme: Theme,
    fonts: Fonts,
    dialogue_family: Option<String>,
    interface_family: Option<String>,
    renderer: Renderer,
    audio: Audio,
    audio_started: bool,
    duck: f32,
    duck_fade: f64,
    audio_metadata: AudioMetadata,
    audio_metadata_ready: bool,
    pending_audio: Vec<Loaded>,
    assets: Assets,
    images: HashMap<String, ImageInfo>,
    saves: SaveStore,
    slots: Vec<Option<String>>,
    mouse: (f32, f32),
    hotspots: Vec<Hotspot>,
    back_to: Screen,
    backlog_scroll: usize,
    notice: Option<(String, f32)>,
    focused_custom: Option<String>,
    /// Persistent data waiting to be written; reading a line changes it, so writes are batched.
    persistent_pending: Option<Vec<u8>>,
    persistent_timer: f32,
    /// Off when the stored persistent data came from a newer engine, so it is never overwritten.
    persistent_writable: bool,
    can_quit: bool,
    quit: bool,
}

impl Player {
    /// `audio` may be silent until the first user gesture on the web; see [`Player::start_audio`].
    pub fn new(
        renderer: Renderer,
        project: &Project,
        theme: Theme,
        assets: Assets,
        saves: SaveStore,
        can_quit: bool,
    ) -> Result<Player, String> {
        let saved_language = saves.language().unwrap_or_default();
        let mut game = Game::new(project.program.clone()).map_err(|e| e.to_string())?;
        game.set_localizations(project.translations.clone(), &saved_language);
        let mut persistent_writable = true;
        match saves.persistent().map(|bytes| Persistent::from_bytes(&bytes)) {
            Some(Ok(persistent)) => game.set_persistent(persistent),
            Some(Err(e)) => {
                log(&format!("persistent data ignored: {e}"));
                persistent_writable = !matches!(e, PersistentError::UnsupportedVersion(_));
            }
            None => {}
        }
        let mut assets = assets;
        let audio_metadata_ready = !cfg!(target_arch = "wasm32");
        if !audio_metadata_ready {
            assets.request(Request::AudioMetadata);
        }
        let title = Title::parse(&load_title(project)?)?;
        let project_name = project.manifest.name.clone();
        let title_text = title.title.clone().unwrap_or_else(|| project_name.clone());
        let mut player = Player {
            game,
            screens: project.screens.clone(),
            start_label: project.manifest.start.clone(),
            project_name,
            title,
            title_text,
            in_splashscreen: false,
            theme,
            fonts: Fonts::new(),
            dialogue_family: None,
            interface_family: None,
            renderer,
            audio: Audio::silent(),
            audio_started: false,
            duck: project.manifest.audio.duck,
            duck_fade: project.manifest.audio.duck_fade,
            audio_metadata: project.audio_metadata.clone(),
            audio_metadata_ready,
            pending_audio: Vec::new(),
            assets,
            images: HashMap::new(),
            saves,
            slots: Vec::new(),
            mouse: (-1.0, -1.0),
            hotspots: Vec::new(),
            back_to: Screen::Game,
            backlog_scroll: 0,
            notice: None,
            focused_custom: None,
            persistent_pending: None,
            persistent_timer: 0.0,
            persistent_writable,
            can_quit,
            quit: false,
        };
        for (dialogue, path) in [(true, &player.theme.fonts.dialogue), (false, &player.theme.fonts.interface)] {
            if let Some(path) = path {
                player.assets.request(Request::Font { dialogue, path: path.clone() });
            }
        }
        Ok(player)
    }

    pub fn start(&mut self) {
        if self.game.has_label("splashscreen") {
            self.in_splashscreen = true;
            self.game.start("splashscreen");
        } else {
            self.show_title();
        }
    }

    pub fn set_title(&mut self, text: &str) -> Result<(), String> {
        self.title = Title::parse(text)?;
        self.title_text = self.title.title.clone().unwrap_or_else(|| self.project_name.clone());
        Ok(())
    }

    fn show_title(&mut self) {
        self.in_splashscreen = false;
        if self.game.has_label("main_menu") {
            self.game.start("main_menu");
        } else {
            self.slots = (0..SLOTS).map(|i| self.saves.preview(i)).collect();
            self.game.show_title();
            if self.audio_started {
                self.play_title_music();
            }
        }
    }

    fn start_game(&mut self) {
        self.in_splashscreen = false;
        let label = self.start_label.clone();
        self.game.start(&label);
    }

    fn play_title_music(&mut self) {
        if let Some(asset) = self.title.music.clone() {
            self.assets.request(Request::TitleMusic(asset));
        }
    }

    pub fn start_at(&mut self, pc: usize) {
        self.in_splashscreen = false;
        self.game.start_at(pc);
    }

    pub fn preview_at(&mut self, pc: usize) {
        self.in_splashscreen = false;
        let program = self.game.program().clone();
        let start = program.label_at(pc).and_then(|l| program.labels.get(l).copied()).unwrap_or(pc);
        self.game.seek_to(start, pc);
    }

    pub fn sprite_at(&self, x: f32, y: f32) -> Option<String> {
        let images = &self.images;
        let lookup = |name: &str| images.get(name).copied().unwrap_or(ImageInfo::Loading);
        let (x, y) = self.renderer.letterbox().to_virtual(x, y);
        view::sprite_hit_rects(self.game.screen, self.game.stage(), &lookup, self.renderer.virtual_size())
            .into_iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(tag, _)| tag)
    }

    pub fn debug_start_at(
        &mut self,
        pc: usize,
        overrides: &std::collections::BTreeMap<String, kairos_vm::Value>,
    ) -> Result<(), kairos_vm::RuntimeError> {
        self.in_splashscreen = false;
        self.game.start_at_with_overrides(pc, overrides)
    }

    pub fn debug_variables(&self) -> std::collections::BTreeMap<String, kairos_vm::Value> {
        self.game.debug_variables()
    }

    pub fn debug_pc(&self) -> usize {
        self.game.debug_pc()
    }

    pub fn debug_call_stack(&self) -> &[usize] {
        self.game.debug_call_stack()
    }

    pub fn debug_history_len(&self) -> usize {
        self.game.debug_history_len()
    }

    pub fn debug_set_variable(&mut self, name: &str, value: kairos_vm::Value) -> Result<(), kairos_vm::RuntimeError> {
        self.game.debug_set_variable(name, value)
    }

    pub fn debug_rollback_to(&mut self, position: usize) -> bool {
        self.game.debug_rollback_to(position)
    }

    /// Opens the audio device. Browsers only allow this after a user gesture.
    pub fn start_audio(&mut self) {
        if self.audio_started {
            return;
        }
        self.audio_started = true;
        self.audio = Audio::new();
        self.apply_volumes();
        self.sync_audio();
        if self.game.screen == Screen::Title {
            self.play_title_music();
        }
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Writes persistent data now instead of waiting for the next batch, e.g. before the
    /// window closes.
    pub fn flush_persistent(&mut self) {
        if let Some(bytes) = self.game.take_persistent() {
            self.persistent_pending = Some(bytes);
        }
        self.persistent_timer = 0.0;
        let Some(bytes) = self.persistent_pending.take() else { return };
        if !self.persistent_writable {
            return;
        }
        if let Err(e) = self.saves.write_persistent(&bytes) {
            self.notify(format!("Could not save progress: {e}"));
        }
    }

    fn batch_persistent(&mut self, dt: f32) {
        if let Some(bytes) = self.game.take_persistent() {
            self.persistent_pending = Some(bytes);
        }
        self.persistent_timer += dt;
        if self.persistent_pending.is_some() && self.persistent_timer >= PERSISTENT_INTERVAL {
            self.flush_persistent();
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize((width, height));
    }

    pub fn device(&self) -> &wgpu::Device {
        self.renderer.device()
    }

    pub fn frame(&mut self, dt: f32, target: &wgpu::TextureView) {
        let dt = dt.clamp(0.0, 0.25);
        self.receive_assets();
        self.game.tick(dt);
        if self.in_splashscreen && matches!(self.game.shown, Shown::End) {
            self.show_title();
        }
        match self.game.shown {
            Shown::OpenLoad => self.act(Action::Open(Screen::Load)),
            Shown::OpenSettings => self.act(Action::Open(Screen::Settings)),
            Shown::Quit if self.can_quit => self.quit = true,
            Shown::Quit => self.game.proceed(),
            _ => {}
        }
        self.batch_persistent(dt);
        if self.quit {
            self.flush_persistent();
        }
        self.renderer.tick(dt);
        self.audio.update();
        self.handle_signals();
        self.request_stage_images();
        if let Some((_, left)) = &mut self.notice {
            *left -= dt;
            if *left <= 0.0 {
                self.notice = None;
            }
        }

        let images = &self.images;
        let lookup = |name: &str| images.get(name).copied().unwrap_or(ImageInfo::Loading);
        let variables = self.game.debug_variables();
        let input = ViewInput {
            game: &self.game,
            theme: &self.theme,
            title: &self.title,
            title_text: &self.title_text,
            size: self.renderer.virtual_size(),
            mouse: self.mouse,
            image: &lookup,
            slots: &self.slots,
            backlog_scroll: self.backlog_scroll,
            dialogue_family: self.dialogue_family.as_deref(),
            interface_family: self.interface_family.as_deref(),
            can_quit: self.can_quit,
            notice: self.notice.as_ref().map(|n| n.0.as_str()),
            screens: &self.screens,
            screen_stack: self.game.custom_screens(),
            variables: &variables,
            focused_custom: self.focused_custom.as_deref(),
        };
        let (frame, hotspots) = view::build(&input, &mut self.fonts);
        self.hotspots = hotspots;
        self.renderer.render(&mut self.fonts, &frame, target);
    }

    pub fn pointer_moved(&mut self, x: f32, y: f32) {
        self.mouse = self.renderer.letterbox().to_virtual(x, y);
        if let Some(Action::Custom { screen, node, selection, repeat_path }) =
            view::hit(&self.hotspots, self.mouse.0, self.mouse.1)
        {
            self.focused_custom = Some(view::custom_focus_id(&screen, &node, selection, &repeat_path));
        }
    }

    pub fn pointer_down(&mut self, button: Button) {
        self.start_audio();
        match button {
            Button::Secondary if self.custom_modal() || self.custom_close_configured() => self.close_custom_screen(),
            Button::Secondary if self.game.screen == Screen::Game => self.act(Action::Open(Screen::Menu)),
            Button::Secondary => self.act(Action::Back),
            Button::Primary => match view::hit(&self.hotspots, self.mouse.0, self.mouse.1) {
                Some(action) => self.act(action),
                None if !self.custom_modal() && self.game.screen == Screen::Game => self.game.proceed(),
                None => {}
            },
        }
    }

    /// Positive `lines` scrolls up: rollback in the game, older entries in the history.
    pub fn wheel(&mut self, lines: f32) {
        match self.game.screen {
            Screen::Game if lines > 0.0 => {
                self.game.rollback();
            }
            Screen::Backlog => {
                let max = self.game.backlog().len().saturating_sub(1);
                let next = self.backlog_scroll as i64 + lines.signum() as i64;
                self.backlog_scroll = next.clamp(0, max as i64) as usize;
            }
            _ => {}
        }
    }

    pub fn key(&mut self, key: Key) {
        self.start_audio();
        let screen = self.game.screen;
        if key == Key::Escape && self.custom_close_configured() {
            self.close_custom_screen();
            return;
        }
        let has_custom_controls = self.hotspots.iter().any(|hotspot| matches!(&hotspot.action, Action::Custom { .. }));
        if self.custom_modal() || has_custom_controls {
            match key {
                Key::Escape if !self.game.custom_screens().is_empty() => self.close_custom_screen(),
                Key::Advance if has_custom_controls => self.activate_focused_custom(),
                Key::Forward | Key::FocusNext if has_custom_controls => self.focus_custom(1),
                Key::Back | Key::FocusPrevious if has_custom_controls => self.focus_custom(-1),
                _ => {}
            }
            return;
        }
        match key {
            Key::Advance if screen == Screen::Game => self.game.proceed(),
            Key::Escape if screen == Screen::Game => self.act(Action::Open(Screen::Menu)),
            Key::Escape => self.act(Action::Back),
            Key::Back => self.wheel(1.0),
            Key::Forward => self.wheel(-1.0),
            Key::Digit(n) if screen == Screen::Game && n > 0 => {
                if matches!(self.game.shown, Shown::Menu { .. }) {
                    self.game.choose(n as usize - 1);
                }
            }
            Key::Auto if screen == Screen::Game => self.act(Action::Toggle(crate::game::Mode::Auto)),
            Key::Skip if screen == Screen::Game => self.act(Action::Toggle(crate::game::Mode::Skip)),
            _ => {}
        }
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Choose(i) => self.game.choose(i),
            Action::Back => {
                if self.game.screen == Screen::Title {
                    return;
                }
                self.game.screen = if self.game.screen == Screen::Menu { Screen::Game } else { self.back_to };
                self.back_to = Screen::Game;
                if matches!(self.game.shown, Shown::OpenLoad | Shown::OpenSettings) {
                    self.game.proceed();
                }
            }
            Action::Open(screen) => {
                self.back_to = match self.game.screen {
                    Screen::Menu => Screen::Menu,
                    Screen::Title => Screen::Title,
                    _ => Screen::Game,
                };
                self.game.screen = screen;
                self.backlog_scroll = 0;
                self.slots = (0..SLOTS).map(|i| self.saves.preview(i)).collect();
            }
            Action::Save(slot) => {
                let Some(bytes) = self.game.save() else { return };
                let preview = match &self.game.shown {
                    Shown::Say { who: Some(who), text, .. } => format!("{who}: {text}"),
                    Shown::Say { text, .. } => text.clone(),
                    Shown::Menu { caption, choices } => caption.clone().unwrap_or_else(|| choices.join(" / ")),
                    _ => String::new(),
                };
                match self.saves.write(slot, &bytes, &preview) {
                    Ok(()) => {
                        self.notify(format!("Saved to slot {}", slot + 1));
                        self.flush_persistent();
                    }
                    Err(e) => self.notify(format!("Could not save: {e}")),
                }
                self.slots = (0..SLOTS).map(|i| self.saves.preview(i)).collect();
            }
            Action::Load(slot) => {
                let Some(bytes) = self.saves.read(slot) else { return };
                match self.game.load(&bytes) {
                    Ok(()) => self.notify_loaded(slot),
                    Err(e) => self.notify(format!("Could not load: {e}")),
                }
            }
            Action::Start => self.start_game(),
            Action::Continue => {
                if let Some(slot) = self.saves.newest()
                    && let Some(bytes) = self.saves.read(slot)
                {
                    if let Err(e) = self.game.load(&bytes) {
                        self.notify(format!("Could not load: {e}"));
                    } else {
                        self.game.screen = Screen::Game;
                        self.notify_loaded(slot);
                    }
                }
            }
            Action::MainMenu => self.show_title(),
            Action::Toggle(mode) => {
                self.game.toggle(mode);
                self.game.screen = Screen::Game;
            }
            Action::Rollback => {
                self.game.rollback();
            }
            Action::Adjust(setting, direction) => {
                view::adjust(&mut self.game.settings, setting, direction);
                self.apply_volumes();
            }
            Action::Language(direction) => {
                self.game.cycle_language(direction);
                if let Err(error) = self.saves.set_language(self.game.language()) {
                    self.notify(format!("Could not save language: {error}"));
                }
            }
            Action::Restart => self.start_game(),
            Action::Quit => self.quit = true,
            Action::Custom { screen, node, selection, .. } => self.activate_custom(&screen, &node, selection),
        }
    }

    fn custom_close_configured(&self) -> bool {
        self.game
            .custom_screens()
            .last()
            .and_then(|id| self.screens.get(id))
            .is_some_and(|screen| screen.close_action.is_some())
    }

    fn custom_modal(&self) -> bool {
        self.game.custom_screens().iter().any(|id| self.screens.get(id).is_some_and(|screen| screen.modal))
    }

    fn close_custom_screen(&mut self) {
        let Some(id) = self.game.custom_screens().last().cloned() else { return };
        let Some(action) = self.screens.get(&id).and_then(|screen| screen.close_action.clone()) else { return };
        self.run_screen_action(action);
    }

    fn focus_custom(&mut self, direction: isize) {
        let controls: Vec<_> = self
            .hotspots
            .iter()
            .filter_map(|hotspot| match &hotspot.action {
                Action::Custom { screen, node, selection, repeat_path } => {
                    Some((screen.clone(), node.clone(), *selection, repeat_path.clone()))
                }
                _ => None,
            })
            .collect();
        if controls.is_empty() {
            return;
        }
        let current = self.focused_custom.as_ref().and_then(|id| {
            controls.iter().position(|(screen, node, selection, repeat_path)| {
                id == &view::custom_focus_id(screen, node, *selection, repeat_path)
            })
        });
        let next = current.map_or(if direction < 0 { controls.len() - 1 } else { 0 }, |at| {
            (at as isize + direction).rem_euclid(controls.len() as isize) as usize
        });
        let (screen, node, selection, repeat_path) = &controls[next];
        self.focused_custom = Some(view::custom_focus_id(screen, node, *selection, repeat_path));
    }

    fn activate_focused_custom(&mut self) {
        let focused = self.focused_custom.clone().or_else(|| {
            self.hotspots.iter().find_map(|hotspot| match &hotspot.action {
                Action::Custom { screen, node, selection, repeat_path } => {
                    Some(view::custom_focus_id(screen, node, *selection, repeat_path))
                }
                _ => None,
            })
        });
        if let Some(focused) = focused
            && let Some(action) = self.hotspots.iter().find_map(|hotspot| match &hotspot.action {
                Action::Custom { screen, node, selection, repeat_path }
                    if focused == view::custom_focus_id(screen, node, *selection, repeat_path) =>
                {
                    Some(Action::Custom {
                        screen: screen.clone(),
                        node: node.clone(),
                        selection: *selection,
                        repeat_path: repeat_path.clone(),
                    })
                }
                _ => None,
            })
        {
            self.act(action);
        }
    }

    fn activate_custom(&mut self, screen_id: &str, node_id: &str, selection: Option<usize>) {
        let Some(node) =
            self.screens.get(screen_id).and_then(|screen| screen.nodes.iter().find(|node| node.id == node_id)).cloned()
        else {
            return;
        };
        if let Some(index) = selection {
            let Some(value) = node.binding.as_deref().and_then(|binding| self.binding_value(binding)) else { return };
            let selected = match value {
                Value::List(items) => items.get(index).cloned(),
                Value::Dict(items) => items.keys().nth(index).cloned().map(Value::Str),
                _ => None,
            };
            let Some(selected) = selected else { return };
            let Some(target) = node.target.as_deref() else { return };
            if let Err(error) = self.set_binding(target, selected) {
                self.notify(error);
                return;
            }
        } else if node.kind == kairos_project::screens::NodeType::Toggle {
            let Some(binding) = node.binding.as_deref() else { return };
            let value = match self.binding_value(binding) {
                Some(Value::Bool(value)) => Value::Bool(!value),
                _ => return,
            };
            if let Err(error) = self.set_binding(binding, value) {
                self.notify(error);
                return;
            }
        } else if node.kind == kairos_project::screens::NodeType::Slider {
            let Some(binding) = node.binding.as_deref() else { return };
            let Some(old) = self.binding_value(binding) else { return };
            let Some(rect) = self.hotspots.iter().find(|hotspot| matches!(&hotspot.action, Action::Custom { screen, node: id, selection: None, .. } if screen == screen_id && id == node_id)).map(|hotspot| hotspot.rect) else { return };
            let ratio = ((self.mouse.0 - rect.x) / rect.w.max(1.0)).clamp(0.0, 1.0) as f64;
            let min = node.min.unwrap_or(0.0);
            let value = min + ratio * (node.max.unwrap_or(1.0) - min);
            let next = match old {
                Value::Int(_) => Value::Int(value.round() as i64),
                Value::Float(_) => Value::Float(value),
                _ => return,
            };
            if let Err(error) = self.set_binding(binding, next) {
                self.notify(error);
                return;
            }
        }
        if let Some(action) = node.action {
            self.run_screen_action(action);
        }
    }

    fn run_screen_action(&mut self, action: ScreenAction) {
        match action.kind {
            ActionType::Set => {
                let Some(target) = action.target.as_deref() else { return };
                let value = match (action.value, action.binding.as_deref()) {
                    (Some(value), _) => match toml_value(value) {
                        Some(value) => value,
                        None => {
                            self.notify("set action values cannot contain TOML datetimes".into());
                            return;
                        }
                    },
                    (_, Some(binding)) => match self.binding_value(binding) {
                        Some(value) => value,
                        None => return,
                    },
                    _ => return,
                };
                if let Err(error) = self.set_action_binding(target, value) {
                    self.notify(error);
                }
            }
            ActionType::Jump => {
                if let Some(label) = action.target.as_deref()
                    && let Err(error) = self.game.jump_label(label)
                {
                    self.notify(error.to_string());
                }
            }
            ActionType::Call => {
                if let Some(label) = action.target.as_deref()
                    && let Err(error) = self.game.call_label(label)
                {
                    self.notify(error.to_string());
                }
            }
            ActionType::Show => {
                if let Some(id) = action.target.as_deref() {
                    if self.screens.contains_key(id) {
                        self.game.show_custom_screen(id);
                    } else {
                        self.notify(format!("unknown screen `{id}`"));
                    }
                }
            }
            ActionType::Hide => {
                if let Some(id) = action.target.as_deref() {
                    self.game.hide_custom_screen(id);
                }
            }
            ActionType::Return => self.game.return_custom_screen(),
        }
    }

    fn binding_value(&self, path: &str) -> Option<Value> {
        let parts = screens::parse_binding(path).ok()?;
        let (name, rest) = screens::binding_root(&parts)?;
        let variables = self.game.debug_variables();
        let value = variables.get(&name)?;
        screens::read_binding(value, rest).cloned()
    }

    fn set_action_binding(&mut self, path: &str, value: Value) -> Result<(), String> {
        let mut current =
            self.binding_value(path).ok_or_else(|| format!("binding `{path}` does not address an existing value"))?;
        apply_set_value(&mut current, value, path)?;
        self.set_binding(path, current)
    }

    fn set_binding(&mut self, path: &str, value: Value) -> Result<(), String> {
        if self.binding_value(path).is_none() {
            return Err(format!("binding `{path}` does not address an existing value"));
        }
        let parts = screens::parse_binding(path).map_err(|_| format!("invalid binding `{path}`"))?;
        let Some((name, rest)) = screens::binding_root(&parts) else {
            return Err(format!("invalid binding `{path}`"));
        };
        let indices: Vec<_> = rest
            .iter()
            .map(|part| match part {
                BindingPart::Key(key) => Value::Str(key.clone()),
                BindingPart::Index(index) => Value::Int(*index as i64),
            })
            .collect();
        self.game.set_variable_path(&name, &indices, value).map_err(|error| error.to_string())
    }

    fn notify(&mut self, message: String) {
        self.notice = Some((message, 2.5));
    }

    fn notify_loaded(&mut self, slot: usize) {
        if self.game.load_relocated() {
            self.notice = Some((
                format!("Loaded slot {}. The story changed since this save, so it resumed nearby.", slot + 1),
                5.0,
            ));
        } else {
            self.notify(format!("Loaded slot {}", slot + 1));
        }
    }

    fn apply_volumes(&mut self) {
        let s = self.game.settings;
        self.audio.set_volume(Bus::Music, s.music);
        self.audio.set_volume(Bus::Sound, s.sound);
        self.audio.set_volume(Bus::Voice, s.voice);
        self.audio.set_ducking(self.duck, self.duck_fade);
    }

    fn handle_signals(&mut self) {
        for signal in self.game.take_signals() {
            match signal {
                Signal::Transition(name) => match TransitionKind::named(&name) {
                    Some((kind, seconds)) => self.renderer.begin_transition(kind, seconds),
                    None => self.renderer.finish_transition(),
                },
                Signal::Resync => {
                    self.renderer.finish_transition();
                    self.sync_audio();
                }
                Signal::Audio(Effect::Play { channel, asset, looped, queue, fadein }) => {
                    if self.audio.is_enabled() {
                        self.assets.request(Request::Audio { channel, asset, looped, queue, fadein });
                    }
                }
                Signal::Audio(Effect::Stop { channel, fadeout }) => self.audio.stop(&channel, fadeout),
                Signal::Audio(_) => {}
            }
        }
    }

    /// Makes the mixer match the VM's audio state after a jump in time. One-shot sounds are not
    /// replayed; only looping tracks are restored.
    fn sync_audio(&mut self) {
        let wanted = self.game.audio_state().clone();
        for channel in self.audio.channels() {
            if !wanted.get(&channel).is_some_and(|t| t.looped) {
                self.audio.stop(&channel, None);
            }
        }
        if !self.audio.is_enabled() {
            return;
        }
        for (channel, track) in wanted {
            if track.looped && self.audio.playing(&channel) != Some(track.asset.as_str()) {
                let asset = track.asset;
                self.assets.request(Request::Audio { channel, asset, looped: true, queue: false, fadein: None });
            }
        }
    }

    fn request_stage_images(&mut self) {
        let stage = self.game.stage();
        let mut names: Vec<String> = stage.background.iter().map(|w| image_name(w)).collect();
        if self.game.screen == Screen::Title {
            names.extend(self.title.background.iter().cloned());
            names.extend(self.title.logo.iter().cloned());
        }
        for sprite in &stage.sprites {
            let mut words = vec![sprite.tag.clone()];
            words.extend(sprite.attrs.iter().cloned());
            names.push(image_name(&words));
        }
        for id in self.game.custom_screens() {
            if let Some(screen) = self.screens.get(id) {
                for node in &screen.nodes {
                    if let Some(image) = &node.image {
                        names.push(image.clone());
                    } else if node.kind == kairos_project::screens::NodeType::Image
                        && let Some(image) = node.binding.as_deref().and_then(|binding| self.binding_value(binding))
                        && let Value::Str(image) = image
                    {
                        names.push(image);
                    }
                }
            }
        }
        for name in names {
            if !self.images.contains_key(&name) {
                self.images.insert(name.clone(), ImageInfo::Loading);
                self.assets.request(Request::Image(name));
            }
        }
    }

    fn receive_assets(&mut self) {
        for loaded in self.assets.poll() {
            if matches!(&loaded.request, Request::AudioMetadata) {
                if let Some(bytes) = loaded.bytes {
                    match std::str::from_utf8(&bytes)
                        .map_err(|e| e.to_string())
                        .and_then(|text| parse_audio_metadata(text).map_err(|e| e.to_string()))
                    {
                        Ok(metadata) => self.audio_metadata = metadata,
                        Err(e) => log(&format!("audio/audio.toml: {e}")),
                    }
                }
                self.audio_metadata_ready = true;
            } else if matches!(&loaded.request, Request::Audio { .. } | Request::TitleMusic(_))
                && !self.audio_metadata_ready
            {
                self.pending_audio.push(loaded);
            } else {
                self.receive_asset(loaded);
            }
        }
        if self.audio_metadata_ready {
            for loaded in std::mem::take(&mut self.pending_audio) {
                self.receive_audio(loaded);
            }
        }
    }

    fn receive_asset(&mut self, loaded: Loaded) {
        if matches!(&loaded.request, Request::Audio { .. } | Request::TitleMusic(_)) {
            self.receive_audio(loaded);
            return;
        }
        match (loaded.request, loaded.bytes) {
            (Request::Image(name), Some(bytes)) => {
                let info = match decode_image(&bytes, self.renderer.max_texture_size()) {
                    Ok(image) => {
                        let size = image.dimensions();
                        self.renderer.upload_image(&name, size, image.as_raw());
                        ImageInfo::Ready(size.0, size.1)
                    }
                    Err(e) => {
                        log(&format!("image `{name}`: {e}"));
                        ImageInfo::Missing
                    }
                };
                self.images.insert(name, info);
            }
            (Request::Image(name), None) => {
                log(&format!("image `{name}` not found under assets/images"));
                self.images.insert(name, ImageInfo::Missing);
            }
            (Request::Font { dialogue, path }, Some(bytes)) => match self.fonts.load(bytes) {
                Some(family) if dialogue => self.dialogue_family = Some(family),
                Some(family) => self.interface_family = Some(family),
                None => log(&format!("font `{path}` could not be read")),
            },
            (Request::Font { path, .. }, None) => log(&format!("font `{path}` not found under assets")),
            _ => {}
        }
    }

    fn receive_audio(&mut self, loaded: Loaded) {
        let resolved_path = loaded.resolved_path;
        let request = match loaded.request {
            Request::TitleMusic(_) if self.game.screen != Screen::Title => return,
            Request::TitleMusic(asset) => {
                Request::Audio { channel: "music".into(), asset, looped: true, queue: false, fadein: None }
            }
            request => request,
        };
        match (request, loaded.bytes) {
            (Request::Audio { channel, asset, looped, queue, fadein }, Some(bytes)) => {
                let loop_region = audio_loop(&self.audio_metadata, resolved_path.as_deref());
                match Sound::decode(bytes) {
                    Ok(mut sound) => {
                        if let Some(region) = loop_region {
                            sound = sound.with_loop_region(region.loop_start, region.loop_end);
                        }
                        if queue {
                            self.audio.queue(&channel, &asset, sound, looped);
                        } else {
                            self.audio.play(&channel, &asset, sound, looped, fadein);
                        }
                    }
                    Err(e) => log(&format!("audio `{asset}`: {e}")),
                }
            }
            (Request::Audio { asset, .. }, None) => log(&format!("audio `{asset}` not found under assets/audio")),
            _ => {}
        }
    }
}

fn audio_loop(metadata: &AudioMetadata, resolved_path: Option<&str>) -> Option<AudioLoop> {
    resolved_path.and_then(|path| path.strip_prefix("audio/")).and_then(|path| metadata.get(path)).copied()
}

fn decode_image(bytes: &[u8], max: u32) -> Result<image::RgbaImage, image::ImageError> {
    let image = image::load_from_memory(bytes)?;
    let image = if image.width() > max || image.height() > max {
        image.resize(max, max, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    Ok(image.into_rgba8())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_title(project: &Project) -> Result<String, String> {
    match std::fs::read_to_string(project.root.join("ui/title.toml")) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("ui/title.toml: {e}")),
    }
}

#[cfg(target_arch = "wasm32")]
fn load_title(_: &Project) -> Result<String, String> {
    Ok(String::new())
}

pub(crate) fn log(message: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::warn_1(&message.into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_actions_reject_type_mismatches_without_changing_the_target() {
        let mut target = Value::Bool(false);
        let before = target.clone();

        assert!(apply_set_value(&mut target, Value::Str("yes".into()), "enabled").is_err());
        assert_eq!(target, before);
    }

    #[test]
    fn set_actions_reject_toml_datetimes() {
        let parsed: toml::Value = toml::from_str("value = 2024-01-02").unwrap();
        assert!(toml_value(parsed["value"].clone()).is_none());
    }

    #[test]
    fn loop_metadata_uses_the_resolved_audio_path_relative_to_audio() {
        let metadata =
            [("music/theme.ogg".to_string(), AudioLoop { loop_start: 2.0, loop_end: Some(8.0) })].into_iter().collect();
        assert_eq!(
            audio_loop(&metadata, Some("audio/music/theme.ogg")),
            Some(AudioLoop { loop_start: 2.0, loop_end: Some(8.0) })
        );
        assert_eq!(audio_loop(&metadata, Some("audio/theme.ogg")), None);
        assert_eq!(audio_loop(&metadata, None), None);
    }
}
