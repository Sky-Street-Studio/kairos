//! Builds a render frame and its clickable regions from the game state. Pure: the same input
//! always gives the same frame, so rollback and load only need a redraw.

use kairos_render::layout::{cover_rect, sprite_rect, xalign};
use kairos_render::{Color, Draw, Fonts, Frame, Layer, Rect, TextAlign, TextSpec};

use crate::game::{Game, Mode, Screen, Settings, Shown};
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    TextSpeed,
    AutoDelay,
    Music,
    Sound,
    Voice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Choose(usize),
    Back,
    Open(Screen),
    Save(usize),
    Load(usize),
    Toggle(Mode),
    Rollback,
    Adjust(Setting, i8),
    Restart,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hotspot {
    pub rect: Rect,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageInfo {
    Ready(u32, u32),
    Loading,
    Missing,
}

pub const SLOTS: usize = 6;

pub struct ViewInput<'a> {
    pub game: &'a Game,
    pub theme: &'a Theme,
    pub size: (f32, f32),
    pub mouse: (f32, f32),
    pub image: &'a dyn Fn(&str) -> ImageInfo,
    /// Preview text of each save slot, `None` when empty.
    pub slots: &'a [Option<String>],
    /// Lines scrolled back from the newest backlog entry.
    pub backlog_scroll: usize,
    pub dialogue_family: Option<&'a str>,
    pub interface_family: Option<&'a str>,
    pub can_quit: bool,
    pub notice: Option<&'a str>,
}

/// The texture name for an image's words, e.g. `bg room`.
pub fn image_name(words: &[String]) -> String {
    words.join(" ")
}

struct Builder<'a, 'f> {
    input: &'a ViewInput<'a>,
    fonts: &'f mut Fonts,
    hotspots: Vec<Hotspot>,
}

pub fn build(input: &ViewInput, fonts: &mut Fonts) -> (Frame, Vec<Hotspot>) {
    let mut b = Builder { input, fonts, hotspots: Vec::new() };
    let scene = b.scene();
    let game_ui = b.game_ui();
    let mut ui = vec![game_ui];
    if input.game.screen != Screen::Game {
        b.hotspots.clear();
        ui.push(b.overlay());
    }
    if let Some(notice) = input.notice {
        ui.push(b.notice(notice));
    }
    (Frame { scene, ui }, b.hotspots)
}

/// The topmost action under a point.
pub fn hit(hotspots: &[Hotspot], x: f32, y: f32) -> Option<Action> {
    hotspots.iter().rev().find(|h| h.rect.contains(x, y)).map(|h| h.action)
}

impl Builder<'_, '_> {
    fn size(&self) -> (f32, f32) {
        self.input.size
    }

    fn ui_text(&self, text: impl Into<String>, rect: Rect, size: f32, color: Color, align: TextAlign) -> Draw {
        Draw::Text(TextSpec {
            text: text.into(),
            rect,
            size,
            color,
            family: self.input.interface_family.map(String::from),
            align,
            ..TextSpec::default()
        })
    }

    fn scene(&mut self) -> Layer {
        let (w, h) = self.size();
        let stage = self.input.game.stage();
        let mut layer = Vec::new();
        if let Some(words) = &stage.background {
            let name = image_name(words);
            match (self.input.image)(&name) {
                ImageInfo::Ready(iw, ih) => {
                    let rect = cover_rect((iw as f32, ih as f32), (w, h));
                    layer.push(Draw::Image { name, rect, opacity: 1.0 });
                }
                ImageInfo::Missing => {
                    let screen = Rect::new(0.0, 0.0, w, h);
                    layer.push(Draw::Fill {
                        rect: screen,
                        top: Color::rgb(0x2b, 0x2d, 0x3a),
                        bottom: Color::rgb(0x16, 0x17, 0x1f),
                        radius: 0.0,
                    });
                    let label = Rect::new(0.0, 24.0, w, 40.0);
                    layer.push(self.ui_text(placeholder_label(&name), label, 22.0, gray(0.55), TextAlign::Center));
                }
                ImageInfo::Loading => {}
            }
        }
        for sprite in &stage.sprites {
            let mut words = vec![sprite.tag.clone()];
            words.extend(sprite.attrs.iter().cloned());
            let name = image_name(&words);
            let at = sprite.at.as_deref();
            match (self.input.image)(&name) {
                ImageInfo::Ready(iw, ih) => {
                    let rect = sprite_rect(at, (iw as f32, ih as f32), (w, h));
                    layer.push(Draw::Image { name, rect, opacity: 1.0 });
                }
                ImageInfo::Missing => {
                    let (pw, ph) = (h * 0.34, h * 0.78);
                    let rect = Rect::new((w - pw) * xalign(at), h - ph, pw, ph);
                    let fill = Color::rgba(255, 255, 255, 34);
                    layer.push(Draw::Fill { rect, top: fill, bottom: fill.with_alpha(0.3), radius: 18.0 });
                    let label = Rect::new(rect.x, rect.y + 24.0, rect.w, 40.0);
                    layer.push(self.ui_text(placeholder_label(&name), label, 20.0, gray(0.85), TextAlign::Center));
                }
                ImageInfo::Loading => {}
            }
        }
        layer
    }

    fn textbox_rect(&self) -> Rect {
        let (w, h) = self.size();
        let t = &self.input.theme.textbox;
        Rect::new(t.margin, h - t.margin - t.height, w - 2.0 * t.margin, t.height)
    }

    fn game_ui(&mut self) -> Layer {
        let theme = self.input.theme;
        let (w, h) = self.size();
        let mut layer = Vec::new();
        match &self.input.game.shown {
            Shown::Say { who, color, text, revealed, .. } => {
                let character = color.as_deref().and_then(Color::hex);
                self.textbox(&mut layer, who.as_deref(), character, text, Some(*revealed as usize));
                self.quick_menu(&mut layer);
            }
            Shown::Menu { caption, choices } => {
                let mut bottom = h;
                if let Some(caption) = caption {
                    self.textbox(&mut layer, None, None, caption, None);
                    bottom = self.textbox_rect().y;
                }
                self.choices(&mut layer, choices, bottom);
                self.quick_menu(&mut layer);
            }
            Shown::Pause { .. } | Shown::Nothing => {}
            Shown::End => {
                let title = Rect::new(0.0, h * 0.36, w, 60.0);
                layer.push(self.ui_text("The End", title, 44.0, theme.menu.text_color.0, TextAlign::Center));
                let rect = Rect::new((w - 280.0) / 2.0, h * 0.36 + 90.0, 280.0, theme.choice.height);
                self.button(&mut layer, rect, "Play again", Action::Restart, true);
            }
            Shown::Error(message) => {
                self.textbox(&mut layer, Some("Script error"), Some(Color::rgb(255, 140, 140)), message, None);
            }
        }
        let badge = match self.input.game.mode {
            Mode::Auto => Some("AUTO"),
            Mode::Skip => Some("SKIP"),
            Mode::Normal => None,
        };
        if let Some(badge) = badge {
            let rect = Rect::new(w - 120.0, 20.0, 96.0, 36.0);
            let accent = theme.menu.accent.0;
            layer.push(Draw::Fill { rect, top: accent.with_alpha(0.9), bottom: accent.with_alpha(0.9), radius: 18.0 });
            let mut text = self.ui_text(badge, rect, 18.0, Color::rgb(16, 16, 28), TextAlign::Center);
            if let Draw::Text(spec) = &mut text {
                spec.bold = true;
            }
            layer.push(text);
        }
        layer
    }

    fn textbox(
        &mut self,
        layer: &mut Layer,
        who: Option<&str>,
        color: Option<Color>,
        text: &str,
        reveal: Option<usize>,
    ) {
        let theme = self.input.theme;
        let t = &theme.textbox;
        let rect = self.textbox_rect();
        let (top, bottom) = t.fill();
        layer.push(Draw::Fill { rect, top, bottom, radius: t.radius });
        if let Some(who) = who {
            let n = &theme.nameplate;
            let measured = self.fonts.measure(who, n.text_size, self.input.interface_family, None);
            let plate =
                Rect::new(rect.x + t.padding, rect.y - n.text_size * 1.1, measured.width + 44.0, n.text_size * 2.0);
            let (top, bottom) = n.fill();
            layer.push(Draw::Fill { rect: plate, top, bottom, radius: n.radius });
            let name_color = n.text_color.map(|c| c.0).or(color).unwrap_or(t.text_color.0);
            let mut name = self.ui_text(who, plate, n.text_size, name_color, TextAlign::Center);
            if let Draw::Text(spec) = &mut name {
                spec.bold = true;
            }
            layer.push(name);
        }
        let top_pad = if who.is_some() { t.padding + theme.nameplate.text_size * 0.6 } else { t.padding };
        let body = Rect::new(rect.x + t.padding, rect.y + top_pad, rect.w - 2.0 * t.padding, rect.h - top_pad - 12.0);
        layer.push(Draw::Text(TextSpec {
            text: text.to_string(),
            rect: body,
            size: t.text_size,
            color: if who == Some("Script error") { Color::rgb(255, 190, 190) } else { t.text_color.0 },
            family: self.input.dialogue_family.map(String::from),
            reveal,
            ..TextSpec::default()
        }));
    }

    fn choices(&mut self, layer: &mut Layer, choices: &[String], bottom: f32) {
        let c = &self.input.theme.choice;
        let (w, _) = self.size();
        let n = choices.len() as f32;
        let total = n * c.height + (n - 1.0).max(0.0) * c.spacing;
        let mut y = ((bottom - total) / 2.0).max(16.0);
        let width = c.width.min(w - 64.0);
        for (i, choice) in choices.iter().enumerate() {
            let rect = Rect::new((w - width) / 2.0, y, width, c.height);
            self.button(layer, rect, choice, Action::Choose(i), true);
            y += c.height + c.spacing;
        }
    }

    fn quick_menu(&mut self, layer: &mut Layer) {
        let t = &self.input.theme.textbox;
        let rect = self.textbox_rect();
        let items = [
            ("Back", Action::Rollback),
            ("Auto", Action::Toggle(Mode::Auto)),
            ("Skip", Action::Toggle(Mode::Skip)),
            ("Save", Action::Open(Screen::Save)),
            ("Load", Action::Open(Screen::Load)),
            ("Menu", Action::Open(Screen::Menu)),
        ];
        let (item_w, item_h) = (64.0, 26.0);
        let mut x = rect.x + rect.w - t.padding - item_w * items.len() as f32;
        let y = rect.y + rect.h - item_h - 8.0;
        for (label, action) in items {
            let r = Rect::new(x, y, item_w, item_h);
            let active = match action {
                Action::Toggle(m) => self.input.game.mode == m,
                Action::Rollback => self.input.game.can_rollback(),
                _ => true,
            };
            let hovered = r.contains(self.input.mouse.0, self.input.mouse.1);
            let color = match (action, active, hovered) {
                (Action::Toggle(_), true, _) => self.input.theme.menu.accent.0,
                (_, false, _) => gray(0.4),
                (_, true, true) => Color::WHITE,
                (_, true, false) => gray(0.7),
            };
            layer.push(self.ui_text(label, r, 17.0, color, TextAlign::Center));
            self.hotspots.push(Hotspot { rect: r, action });
            x += item_w;
        }
    }

    fn button(&mut self, layer: &mut Layer, rect: Rect, label: &str, action: Action, enabled: bool) {
        let c = &self.input.theme.choice;
        let hovered = enabled && rect.contains(self.input.mouse.0, self.input.mouse.1);
        let (top, bottom) = c.fill();
        let (top, bottom) = if hovered {
            let hover = c.hover_color.0.with_alpha(c.opacity.max(0.9));
            (hover, hover)
        } else {
            (top, bottom)
        };
        layer.push(Draw::Fill { rect, top, bottom, radius: c.radius });
        let color = if enabled { c.text_color.0 } else { c.text_color.0.with_alpha(0.4) };
        let text = rect.inset(8.0);
        layer.push(self.ui_text(label, text, c.text_size, color, TextAlign::Center));
        if enabled {
            self.hotspots.push(Hotspot { rect, action });
        }
    }

    fn overlay(&mut self) -> Layer {
        let theme = self.input.theme;
        let (w, h) = self.size();
        let mut layer = Vec::new();
        let (top, bottom) = theme.menu.fill();
        layer.push(Draw::Fill { rect: Rect::new(0.0, 0.0, w, h), top, bottom, radius: 0.0 });
        let screen = self.input.game.screen;
        let title = match screen {
            Screen::Menu | Screen::Game => "Menu",
            Screen::Save => "Save",
            Screen::Load => "Load",
            Screen::Backlog => "History",
            Screen::Settings => "Settings",
        };
        let title_rect = Rect::new(0.0, 36.0, w, 56.0);
        let mut title = self.ui_text(title, title_rect, 38.0, theme.menu.accent.0, TextAlign::Center);
        if let Draw::Text(spec) = &mut title {
            spec.bold = true;
        }
        layer.push(title);
        match screen {
            Screen::Menu | Screen::Game => self.main_menu(&mut layer),
            Screen::Save | Screen::Load => self.slots(&mut layer, screen == Screen::Save),
            Screen::Backlog => self.backlog(&mut layer),
            Screen::Settings => self.settings(&mut layer),
        }
        if screen != Screen::Menu {
            let rect = Rect::new((w - 240.0) / 2.0, h - 88.0, 240.0, 52.0);
            self.button(&mut layer, rect, "Back", Action::Back, true);
        }
        layer
    }

    fn main_menu(&mut self, layer: &mut Layer) {
        let game = self.input.game;
        let (w, _) = self.size();
        let auto = if game.mode == Mode::Auto { "Auto: on" } else { "Auto: off" };
        let skip = if game.mode == Mode::Skip { "Skip: on" } else { "Skip: off" };
        let mut items = vec![
            ("Return", Action::Back),
            ("Save", Action::Open(Screen::Save)),
            ("Load", Action::Open(Screen::Load)),
            ("History", Action::Open(Screen::Backlog)),
            ("Settings", Action::Open(Screen::Settings)),
            (auto, Action::Toggle(Mode::Auto)),
            (skip, Action::Toggle(Mode::Skip)),
            ("Restart", Action::Restart),
        ];
        if self.input.can_quit {
            items.push(("Quit", Action::Quit));
        }
        let (bw, bh, gap) = (320.0, 48.0, 10.0);
        let mut y = 116.0;
        for (label, action) in items {
            let enabled = !(action == Action::Open(Screen::Save) && game.save().is_none());
            self.button(layer, Rect::new((w - bw) / 2.0, y, bw, bh), label, action, enabled);
            y += bh + gap;
        }
    }

    fn slots(&mut self, layer: &mut Layer, saving: bool) {
        let (w, _) = self.size();
        let (cols, sw, sh, gap) = (2, 460.0, 120.0, 20.0);
        let left = (w - (cols as f32 * sw + gap)) / 2.0;
        for i in 0..SLOTS {
            let (col, row) = (i % cols, i / cols);
            let rect = Rect::new(left + col as f32 * (sw + gap), 120.0 + row as f32 * (sh + gap), sw, sh);
            let preview = self.input.slots.get(i).cloned().flatten();
            let enabled = saving || preview.is_some();
            let label = match &preview {
                Some(p) => format!("Slot {}\n{}", i + 1, ellipsize(p, 60)),
                None => format!("Slot {}\nEmpty", i + 1),
            };
            let action = if saving { Action::Save(i) } else { Action::Load(i) };
            self.button(layer, rect, &label, action, enabled);
        }
    }

    fn backlog(&mut self, layer: &mut Layer) {
        let theme = self.input.theme;
        let (w, h) = self.size();
        let area = Rect::new(w * 0.12, 110.0, w * 0.76, h - 220.0);
        let size = 24.0;
        let lines = self.input.game.backlog();
        let end = lines.len().saturating_sub(self.input.backlog_scroll);
        let mut y = area.y + area.h;
        for line in lines[..end].iter().rev() {
            let text = match &line.who {
                Some(who) => format!("{who}: {}", line.text),
                None => line.text.clone(),
            };
            let measured = self.fonts.measure(&text, size, self.input.dialogue_family, Some(area.w));
            if y - measured.height < area.y {
                break;
            }
            y -= measured.height;
            layer.push(Draw::Text(TextSpec {
                text,
                rect: Rect::new(area.x, y, area.w, measured.height + 4.0),
                size,
                color: if line.who.is_none() { gray(0.75) } else { theme.menu.text_color.0 },
                family: self.input.dialogue_family.map(String::from),
                ..TextSpec::default()
            }));
            y -= 14.0;
        }
        if lines.is_empty() {
            layer.push(self.ui_text("Nothing yet.", area, 24.0, gray(0.6), TextAlign::Center));
        }
    }

    fn settings(&mut self, layer: &mut Layer) {
        let s = self.input.game.settings;
        let theme = self.input.theme;
        let (w, _) = self.size();
        let percent = |v: f32| format!("{}%", (v * 100.0).round());
        let speed =
            if s.text_speed >= Settings::INSTANT { "Instant".to_string() } else { format!("{} cps", s.text_speed) };
        let rows = [
            ("Text speed", speed, Setting::TextSpeed),
            ("Auto delay", format!("{:.1} s", s.auto_delay), Setting::AutoDelay),
            ("Music volume", percent(s.music), Setting::Music),
            ("Sound volume", percent(s.sound), Setting::Sound),
            ("Voice volume", percent(s.voice), Setting::Voice),
        ];
        let left = (w - 640.0) / 2.0;
        let mut y = 130.0;
        for (label, value, setting) in rows {
            let row = Rect::new(left, y, 300.0, 52.0);
            layer.push(self.ui_text(label, row, 26.0, theme.menu.text_color.0, TextAlign::Center));
            self.button(layer, Rect::new(left + 320.0, y, 52.0, 52.0), "-", Action::Adjust(setting, -1), true);
            let value_rect = Rect::new(left + 380.0, y, 180.0, 52.0);
            layer.push(self.ui_text(value, value_rect, 24.0, theme.menu.accent.0, TextAlign::Center));
            self.button(layer, Rect::new(left + 568.0, y, 52.0, 52.0), "+", Action::Adjust(setting, 1), true);
            y += 72.0;
        }
    }

    fn notice(&mut self, notice: &str) -> Layer {
        let (w, _) = self.size();
        let rect = Rect::new((w - 520.0) / 2.0, 20.0, 520.0, 48.0);
        let fill = Color::rgba(10, 10, 18, 230);
        vec![
            Draw::Fill { rect, top: fill, bottom: fill, radius: 12.0 },
            self.ui_text(notice, rect.inset(6.0), 20.0, Color::WHITE, TextAlign::Center),
        ]
    }
}

fn placeholder_label(name: &str) -> String {
    format!("missing image: {name}")
}

fn gray(level: f32) -> Color {
    let v = (level * 255.0) as u8;
    Color::rgb(v, v, v)
}

fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Applies one step of a settings button.
pub fn adjust(settings: &mut Settings, setting: Setting, direction: i8) {
    let d = direction as f32;
    match setting {
        Setting::TextSpeed => {
            let next = if settings.text_speed >= Settings::INSTANT && d < 0.0 {
                Settings::INSTANT - 20.0
            } else {
                settings.text_speed + 20.0 * d
            };
            settings.text_speed = next.clamp(20.0, Settings::INSTANT);
        }
        Setting::AutoDelay => settings.auto_delay = (settings.auto_delay + 0.3 * d).clamp(0.3, 5.0),
        Setting::Music => settings.music = step_volume(settings.music, d),
        Setting::Sound => settings.sound = step_volume(settings.sound, d),
        Setting::Voice => settings.voice = step_volume(settings.voice, d),
    }
}

fn step_volume(v: f32, d: f32) -> f32 {
    ((v * 10.0).round() + d).clamp(0.0, 10.0) / 10.0
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const SCRIPT: &str = "character e \"Eileen\" color=\"#c8ffc8\"\n\nlabel start:\n    scene bg room\n    show e happy at left\n    show k at right\n    e \"Hello.\"\n    menu:\n        \"One\":\n            pass\n        \"Two\":\n            pass\n        \"Three\":\n            pass\n    return\n";

    fn game() -> Game {
        let sources = [(PathBuf::from("script/a.kai"), SCRIPT.to_string())];
        let project = kairos_project::Project::from_sources("name = \"t\"", sources).unwrap();
        let mut game = Game::new(project.program).unwrap();
        game.start("start");
        game
    }

    fn images(name: &str) -> ImageInfo {
        match name {
            "bg room" => ImageInfo::Ready(1920, 1080),
            "e happy" => ImageInfo::Ready(600, 1200),
            _ => ImageInfo::Missing,
        }
    }

    fn view(game: &Game, mouse: (f32, f32)) -> (Frame, Vec<Hotspot>) {
        let theme = Theme::default();
        let slots = [None, Some("Eileen: Hello.".to_string()), None, None, None, None];
        let input = ViewInput {
            game,
            theme: &theme,
            size: (1280.0, 720.0),
            mouse,
            image: &images,
            slots: &slots,
            backlog_scroll: 0,
            dialogue_family: None,
            interface_family: None,
            can_quit: true,
            notice: None,
        };
        build(&input, &mut Fonts::new())
    }

    fn texts(layer: &Layer) -> Vec<&str> {
        layer
            .iter()
            .filter_map(|d| match d {
                Draw::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_scene_draws_the_background_then_sprites_at_their_positions() {
        let game = game();
        let (frame, _) = view(&game, (0.0, 0.0));
        assert_eq!(
            frame.scene[0],
            Draw::Image { name: "bg room".into(), rect: Rect::new(0.0, 0.0, 1280.0, 720.0), opacity: 1.0 }
        );
        assert_eq!(
            frame.scene[1],
            Draw::Image { name: "e happy".into(), rect: Rect::new(0.0, 0.0, 360.0, 720.0), opacity: 1.0 }
        );
    }

    #[test]
    fn missing_images_become_labelled_placeholders() {
        let game = game();
        let (frame, _) = view(&game, (0.0, 0.0));
        let Draw::Fill { rect, .. } = frame.scene[2] else { panic!("{:?}", frame.scene[2]) };
        assert_eq!(rect.x + rect.w, 1280.0);
        assert!(texts(&frame.scene).contains(&"missing image: k"));
    }

    #[test]
    fn dialogue_shows_the_nameplate_and_the_typewriter_progress() {
        let mut game = game();
        game.tick(0.05);
        let (frame, _) = view(&game, (0.0, 0.0));
        let ui = &frame.ui[0];
        assert!(texts(ui).contains(&"Eileen"));
        let line = ui.iter().find_map(|d| match d {
            Draw::Text(t) if t.text == "Hello." => Some(t),
            _ => None,
        });
        assert_eq!(line.unwrap().reveal, Some(2));
    }

    #[test]
    fn menu_choices_are_stacked_centred_and_clickable() {
        let mut game = game();
        game.proceed();
        game.proceed();
        let (_, hotspots) = view(&game, (0.0, 0.0));
        let choices: Vec<&Hotspot> = hotspots.iter().filter(|h| matches!(h.action, Action::Choose(_))).collect();
        assert_eq!(choices.len(), 3);
        let r = choices[1].rect;
        assert_eq!(r.x + r.w / 2.0, 640.0);
        assert!(choices[0].rect.y + choices[0].rect.h < choices[1].rect.y);
        let centre = (choices[0].rect.y + choices[2].rect.y + choices[2].rect.h) / 2.0;
        assert!((centre - 360.0).abs() < 1.0, "{centre}");
        assert_eq!(hit(&hotspots, r.x + 5.0, r.y + 5.0), Some(Action::Choose(1)));
        assert_eq!(hit(&hotspots, 5.0, 5.0), None);
    }

    #[test]
    fn hovering_a_choice_changes_its_fill() {
        let mut game = game();
        game.proceed();
        game.proceed();
        let (_, hotspots) = view(&game, (0.0, 0.0));
        let r = hotspots.iter().find(|h| h.action == Action::Choose(0)).unwrap().rect;
        let fill = |frame: &Frame| {
            frame.ui[0].iter().find_map(|d| match d {
                Draw::Fill { rect, top, .. } if *rect == r => Some(*top),
                _ => None,
            })
        };
        let (plain, _) = view(&game, (0.0, 0.0));
        let (hovered, _) = view(&game, (r.x + 1.0, r.y + 1.0));
        assert_ne!(fill(&plain), fill(&hovered));
    }

    #[test]
    fn overlays_replace_the_game_hotspots() {
        let mut game = game();
        game.screen = Screen::Load;
        let (frame, hotspots) = view(&game, (0.0, 0.0));
        assert_eq!(frame.ui.len(), 2);
        assert!(hotspots.iter().all(|h| !matches!(h.action, Action::Rollback | Action::Choose(_))));
        let loads: Vec<Action> = hotspots.iter().map(|h| h.action).filter(|a| matches!(a, Action::Load(_))).collect();
        assert_eq!(loads, [Action::Load(1)], "only filled slots can be loaded");
        assert!(hotspots.iter().any(|h| h.action == Action::Back));
    }

    #[test]
    fn settings_steps_clamp_and_wrap_to_instant() {
        let mut s = Settings::default();
        adjust(&mut s, Setting::Music, 1);
        adjust(&mut s, Setting::Music, 1);
        adjust(&mut s, Setting::Music, 1);
        assert_eq!(s.music, 1.0);
        s.text_speed = 180.0;
        adjust(&mut s, Setting::TextSpeed, 1);
        assert_eq!(s.text_speed, Settings::INSTANT);
        adjust(&mut s, Setting::TextSpeed, -1);
        assert_eq!(s.text_speed, 180.0);
    }

    #[test]
    fn long_slot_previews_are_shortened() {
        assert_eq!(ellipsize("short", 10), "short");
        assert_eq!(ellipsize("a much longer preview", 8), "a much…");
    }
}
