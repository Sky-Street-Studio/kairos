//! Ties the game, renderer, audio and assets together. Platform shells feed it input and frames.

use std::collections::HashMap;

use kairos_audio::{Audio, Bus, Sound};
use kairos_project::{AudioLoop, AudioMetadata, Project, parse_audio_metadata};
use kairos_render::{Fonts, Renderer, TransitionKind};
use kairos_vm::Effect;

use crate::assets::{Assets, Loaded, Request};
use crate::game::{Game, Screen, Shown, Signal};
use crate::saves::SaveStore;
use crate::theme::{Theme, Title};
use crate::view::{self, Action, Hotspot, ImageInfo, SLOTS, ViewInput, image_name};

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
    Digit(u8),
    Auto,
    Skip,
}

pub struct Player {
    game: Game,
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
        };
        let (frame, hotspots) = view::build(&input, &mut self.fonts);
        self.hotspots = hotspots;
        self.renderer.render(&mut self.fonts, &frame, target);
    }

    pub fn pointer_moved(&mut self, x: f32, y: f32) {
        self.mouse = self.renderer.letterbox().to_virtual(x, y);
    }

    pub fn pointer_down(&mut self, button: Button) {
        self.start_audio();
        match button {
            Button::Secondary if self.game.screen == Screen::Game => self.act(Action::Open(Screen::Menu)),
            Button::Secondary => self.act(Action::Back),
            Button::Primary => match view::hit(&self.hotspots, self.mouse.0, self.mouse.1) {
                Some(action) => self.act(action),
                None if self.game.screen == Screen::Game => self.game.proceed(),
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
                    Ok(()) => self.notify(format!("Saved to slot {}", slot + 1)),
                    Err(e) => self.notify(format!("Could not save: {e}")),
                }
                self.slots = (0..SLOTS).map(|i| self.saves.preview(i)).collect();
            }
            Action::Load(slot) => {
                let Some(bytes) = self.saves.read(slot) else { return };
                match self.game.load(&bytes) {
                    Ok(()) => self.notify(format!("Loaded slot {}", slot + 1)),
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
                        self.notify(format!("Loaded slot {}", slot + 1));
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
        }
    }

    fn notify(&mut self, message: String) {
        self.notice = Some((message, 2.5));
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
