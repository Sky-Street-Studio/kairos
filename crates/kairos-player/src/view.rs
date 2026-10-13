//! Builds a render frame and its clickable regions from the game state. Pure: the same input
//! always gives the same frame, so rollback and load only need a redraw.

use kairos_render::layout::{cover_rect, sprite_rect, xalign};
use kairos_render::{Color, Draw, Fonts, Frame, Layer, Rect, TextAlign, TextSpec};

use crate::game::{Game, Mode, Screen, Settings, Shown};
use crate::theme::{Theme, Title, TitleButton};
use kairos_project::screens::{self, AnchorX, AnchorY, Dimension, NodeType, ScreenDefinition, ScreenNode, SizingMode};
use kairos_vm::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    TextSpeed,
    AutoDelay,
    Music,
    Sound,
    Voice,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Choose(usize),
    Back,
    Open(Screen),
    Save(usize),
    Load(usize),
    Toggle(Mode),
    Rollback,
    Adjust(Setting, i8),
    Language(i8),
    Restart,
    Start,
    Continue,
    MainMenu,
    Quit,
    Custom { screen: String, node: String, selection: Option<usize>, repeat_path: Vec<String> },
}

#[derive(Debug, Clone, PartialEq)]
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
    pub title: &'a Title,
    pub title_text: &'a str,
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
    pub screens: &'a BTreeMap<String, ScreenDefinition>,
    pub screen_stack: &'a [String],
    pub variables: &'a BTreeMap<String, Value>,
    pub focused_custom: Option<&'a str>,
}

pub(crate) fn sprite_rects(
    stage: &kairos_vm::Stage,
    image: &dyn Fn(&str) -> ImageInfo,
    (w, h): (f32, f32),
) -> Vec<(String, Rect)> {
    stage
        .sprites
        .iter()
        .map(|sprite| {
            let mut words = vec![sprite.tag.clone()];
            words.extend(sprite.attrs.iter().cloned());
            let at = sprite.at.as_deref();
            let rect = match image(&image_name(&words)) {
                ImageInfo::Ready(iw, ih) => sprite_rect(at, (iw as f32, ih as f32), (w, h)),
                _ => {
                    let (pw, ph) = (h * 0.34, h * 0.78);
                    Rect::new((w - pw) * xalign(at), h - ph, pw, ph)
                }
            };
            (sprite.tag.clone(), rect)
        })
        .collect()
}

pub(crate) fn sprite_hit_rects(
    screen: Screen,
    stage: &kairos_vm::Stage,
    image: &dyn Fn(&str) -> ImageInfo,
    size: (f32, f32),
) -> Vec<(String, Rect)> {
    if screen == Screen::Title {
        return Vec::new();
    }
    stage
        .sprites
        .iter()
        .zip(sprite_rects(stage, image, size))
        .filter_map(|(sprite, (tag, rect))| {
            let mut words = vec![sprite.tag.clone()];
            words.extend(sprite.attrs.iter().cloned());
            (!matches!(image(&image_name(&words)), ImageInfo::Loading)).then_some((tag, rect))
        })
        .collect()
}

/// The texture name for an image's words, e.g. `bg room`.
pub fn image_name(words: &[String]) -> String {
    words.join(" ")
}

struct RepeatScope {
    item: Value,
    index: Value,
    key: Option<Value>,
    path: Vec<String>,
}

pub(crate) fn custom_focus_id(screen: &str, node: &str, selection: Option<usize>, repeat_path: &[String]) -> String {
    let selection = selection.map_or_else(|| "-".into(), |index| index.to_string());
    format!("{screen}/{node}/{repeat_path:?}/{selection}")
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
    b.custom_screens(&mut ui);
    (Frame { scene, ui }, b.hotspots)
}

/// The topmost action under a point.
pub fn hit(hotspots: &[Hotspot], x: f32, y: f32) -> Option<Action> {
    hotspots.iter().rev().find(|h| h.rect.contains(x, y)).map(|h| h.action.clone())
}

impl Builder<'_, '_> {
    fn custom_screens(&mut self, ui: &mut Vec<Layer>) {
        let mut screens: Vec<_> = self
            .input
            .screen_stack
            .iter()
            .enumerate()
            .filter_map(|(index, id)| self.input.screens.get(id).map(|screen| (screen.layer, index, id, screen)))
            .collect();
        screens.sort_by_key(|(layer, index, ..)| (*layer, *index));
        for (_, _, id, screen) in screens {
            if screen.modal {
                self.hotspots.clear();
            }
            let size = self.size();
            let Some(root) = screen.nodes.iter().find(|node| node.parent.is_none()) else { continue };
            let rect = Rect::new(0.0, 0.0, size.0, size.1);
            let mut layer = Vec::new();
            if screen.modal {
                layer.push(Draw::fill(rect, Color::rgba(0, 0, 0, 120)));
            }
            self.draw_screen_node(id, screen, root, rect, &mut layer, None);
            ui.push(layer);
        }
    }

    fn draw_screen_node(
        &mut self,
        screen_id: &str,
        screen: &ScreenDefinition,
        node: &ScreenNode,
        rect: Rect,
        layer: &mut Layer,
        scope: Option<&RepeatScope>,
    ) {
        let theme = &self.input.theme.menu;
        let color = node.color.as_deref().and_then(Color::hex).unwrap_or(theme.accent.0);
        let children: Vec<_> = screen.nodes.iter().filter(|child| child.parent.as_deref() == Some(&node.id)).collect();
        let binding = node.binding.as_deref().and_then(|path| screen_binding(self.input.variables, path, scope));
        if node.kind == NodeType::Repeat {
            let instances: Vec<_> = match binding {
                Some(Value::List(items)) => {
                    items.iter().enumerate().map(|(index, item)| (item.clone(), index, None)).collect()
                }
                Some(Value::Dict(items)) => items
                    .iter()
                    .enumerate()
                    .map(|(index, (key, item))| (item.clone(), index, Some(key.clone())))
                    .collect(),
                _ => Vec::new(),
            };
            let height = rect.h / instances.len().max(1) as f32;
            for (index, (item, order, key)) in instances.into_iter().enumerate() {
                let child_rect = Rect::new(rect.x, rect.y + index as f32 * height, rect.w, height);
                let mut path = scope.map_or_else(Vec::new, |scope| scope.path.clone());
                path.push(key.as_ref().map_or_else(|| format!("index:{order}"), |key| format!("key:{key}")));
                let repeated = RepeatScope { item, index: Value::Int(order as i64), key: key.map(Value::Str), path };
                for child in &children {
                    self.draw_screen_node(screen_id, screen, child, child_rect, layer, Some(&repeated));
                }
            }
            return;
        }
        if matches!(node.kind, NodeType::Stack | NodeType::Row | NodeType::Column | NodeType::Grid) {
            let inside = Rect::new(
                rect.x + node.padding,
                rect.y + node.padding,
                (rect.w - node.padding * 2.0).max(0.0),
                (rect.h - node.padding * 2.0).max(0.0),
            );
            let child_rects = match node.kind {
                NodeType::Row => self.row_rects(&children, inside, node.gap, scope),
                NodeType::Column => self.column_rects(&children, inside, node.gap, scope),
                NodeType::Grid => self.grid_rects(&children, inside, node.gap, node.columns.unwrap_or(1), scope),
                _ => children.iter().map(|child| intersect(self.fit_node(child, inside, scope), inside)).collect(),
            };
            for (child, child_rect) in children.into_iter().zip(child_rects) {
                self.draw_screen_node(screen_id, screen, child, child_rect, layer, scope);
            }
            return;
        }
        match node.kind {
            NodeType::Text => {
                let text = node
                    .text
                    .as_deref()
                    .map(|text| interpolate_screen_text(text, self.input.variables, scope))
                    .or_else(|| binding.map(value_text))
                    .unwrap_or_default();
                layer.push(self.ui_text(
                    text,
                    rect,
                    24.0,
                    node.color.as_deref().and_then(Color::hex).unwrap_or(theme.text_color.0),
                    TextAlign::Left,
                ));
            }
            NodeType::Image => {
                let name = node.image.as_deref().or_else(|| {
                    binding.and_then(|value| match value {
                        Value::Str(name) => Some(name.as_str()),
                        _ => None,
                    })
                });
                if let Some(name) = name
                    && matches!((self.input.image)(name), ImageInfo::Ready(_, _))
                {
                    layer.push(Draw::Image { name: name.to_string(), rect, opacity: 1.0 });
                }
            }
            NodeType::Button => {
                let focused = self.input.focused_custom
                    == Some(&custom_focus_id(
                        screen_id,
                        &node.id,
                        None,
                        scope.map_or(&[], |scope| scope.path.as_slice()),
                    ));
                screen_panel(
                    layer,
                    rect,
                    if focused { color } else { theme.color.0 },
                    self.input.theme.choice.hover_color.0,
                );
                layer.push(self.ui_text(
                    node.text.as_deref().unwrap_or("Button"),
                    rect.inset(8.0),
                    22.0,
                    theme.text_color.0,
                    TextAlign::Center,
                ));
                self.hotspots.push(Hotspot {
                    rect,
                    action: Action::Custom {
                        screen: screen_id.into(),
                        node: node.id.clone(),
                        selection: None,
                        repeat_path: scope.map_or_else(Vec::new, |scope| scope.path.clone()),
                    },
                });
            }
            NodeType::Bar => {
                screen_panel(layer, rect, theme.color.0, self.input.theme.choice.hover_color.0);
                if let Some(value) = binding.and_then(number_value) {
                    let ratio = ((value - node.min.unwrap_or(0.0))
                        / (node.max.unwrap_or(1.0) - node.min.unwrap_or(0.0)))
                    .clamp(0.0, 1.0) as f32;
                    layer.push(Draw::fill(Rect::new(rect.x, rect.y, rect.w * ratio, rect.h), color));
                }
            }
            NodeType::Toggle => {
                let enabled = matches!(binding, Some(Value::Bool(true)));
                let focused = self.input.focused_custom
                    == Some(&custom_focus_id(
                        screen_id,
                        &node.id,
                        None,
                        scope.map_or(&[], |scope| scope.path.as_slice()),
                    ));
                screen_panel(
                    layer,
                    rect,
                    if enabled || focused { color } else { theme.color.0 },
                    self.input.theme.choice.hover_color.0,
                );
                layer.push(self.ui_text(
                    node.text.as_deref().unwrap_or(if enabled { "On" } else { "Off" }),
                    rect.inset(6.0),
                    20.0,
                    theme.text_color.0,
                    TextAlign::Center,
                ));
                self.hotspots.push(Hotspot {
                    rect,
                    action: Action::Custom {
                        screen: screen_id.into(),
                        node: node.id.clone(),
                        selection: None,
                        repeat_path: scope.map_or_else(Vec::new, |scope| scope.path.clone()),
                    },
                });
            }
            NodeType::Slider => {
                let focused = self.input.focused_custom
                    == Some(&custom_focus_id(
                        screen_id,
                        &node.id,
                        None,
                        scope.map_or(&[], |scope| scope.path.as_slice()),
                    ));
                screen_panel(
                    layer,
                    rect,
                    if focused { color } else { theme.color.0 },
                    self.input.theme.choice.hover_color.0,
                );
                if let Some(value) = binding.and_then(number_value) {
                    let ratio = ((value - node.min.unwrap_or(0.0))
                        / (node.max.unwrap_or(1.0) - node.min.unwrap_or(0.0)))
                    .clamp(0.0, 1.0) as f32;
                    layer.push(Draw::fill(Rect::new(rect.x, rect.y, rect.w * ratio, rect.h), color));
                }
                self.hotspots.push(Hotspot {
                    rect,
                    action: Action::Custom {
                        screen: screen_id.into(),
                        node: node.id.clone(),
                        selection: None,
                        repeat_path: scope.map_or_else(Vec::new, |scope| scope.path.clone()),
                    },
                });
            }
            NodeType::List => {
                let values: Vec<Value> = match binding {
                    Some(Value::List(items)) => items.clone(),
                    Some(Value::Dict(items)) => items.keys().cloned().map(Value::Str).collect(),
                    _ => Vec::new(),
                };
                let row_height = rect.h / values.len().max(1) as f32;
                for (index, value) in values.iter().enumerate() {
                    let row = Rect::new(rect.x, rect.y + row_height * index as f32, rect.w, row_height);
                    let focused = self.input.focused_custom
                        == Some(&custom_focus_id(
                            screen_id,
                            &node.id,
                            Some(index),
                            scope.map_or(&[], |scope| scope.path.as_slice()),
                        ));
                    screen_panel(
                        layer,
                        row,
                        if focused { color } else { theme.color.0 },
                        self.input.theme.choice.hover_color.0,
                    );
                    layer.push(self.ui_text(
                        value_text(value),
                        row.inset(4.0),
                        20.0,
                        theme.text_color.0,
                        TextAlign::Left,
                    ));
                    self.hotspots.push(Hotspot {
                        rect: row,
                        action: Action::Custom {
                            screen: screen_id.into(),
                            node: node.id.clone(),
                            selection: Some(index),
                            repeat_path: scope.map_or_else(Vec::new, |scope| scope.path.clone()),
                        },
                    });
                }
            }
            NodeType::Stack | NodeType::Row | NodeType::Column | NodeType::Grid | NodeType::Repeat => unreachable!(),
        }
    }

    fn row_rects(&mut self, children: &[&ScreenNode], rect: Rect, gap: f32, scope: Option<&RepeatScope>) -> Vec<Rect> {
        let sizes = self.distribute(children, true, rect.w, gap, scope);
        let mut x = rect.x;
        children
            .iter()
            .zip(sizes)
            .map(|(child, width)| {
                let cell = Rect::new(x, rect.y, width, rect.h);
                x += width + gap;
                intersect(self.fit_node(child, cell, scope), rect)
            })
            .collect()
    }

    fn column_rects(
        &mut self,
        children: &[&ScreenNode],
        rect: Rect,
        gap: f32,
        scope: Option<&RepeatScope>,
    ) -> Vec<Rect> {
        let sizes = self.distribute(children, false, rect.h, gap, scope);
        let mut y = rect.y;
        children
            .iter()
            .zip(sizes)
            .map(|(child, height)| {
                let cell = Rect::new(rect.x, y, rect.w, height);
                y += height + gap;
                intersect(self.fit_node(child, cell, scope), rect)
            })
            .collect()
    }

    fn grid_rects(
        &mut self,
        children: &[&ScreenNode],
        rect: Rect,
        gap: f32,
        columns: usize,
        scope: Option<&RepeatScope>,
    ) -> Vec<Rect> {
        let columns = columns.max(1);
        let rows = children.len().div_ceil(columns).max(1);
        let width = (rect.w - gap * (columns - 1) as f32) / columns as f32;
        let height = (rect.h - gap * (rows - 1) as f32) / rows as f32;
        children
            .iter()
            .enumerate()
            .map(|(index, child)| {
                let (row, col) = (index / columns, index % columns);
                let cell =
                    Rect::new(rect.x + col as f32 * (width + gap), rect.y + row as f32 * (height + gap), width, height);
                intersect(self.fit_node(child, cell, scope), rect)
            })
            .collect()
    }

    fn distribute(
        &mut self,
        children: &[&ScreenNode],
        horizontal: bool,
        available: f32,
        gap: f32,
        scope: Option<&RepeatScope>,
    ) -> Vec<f32> {
        let fixed: Vec<_> = children.iter().map(|child| self.dimension_size(child, horizontal, scope)).collect();
        let used: f32 = fixed.iter().flatten().sum();
        let fill_count = fixed.iter().filter(|size| size.is_none()).count();
        let fill =
            ((available - gap * children.len().saturating_sub(1) as f32 - used).max(0.0)) / fill_count.max(1) as f32;
        fixed.into_iter().map(|size| size.unwrap_or(fill)).collect()
    }

    fn dimension_size(&mut self, node: &ScreenNode, horizontal: bool, scope: Option<&RepeatScope>) -> Option<f32> {
        match if horizontal { node.width } else { node.height } {
            Some(Dimension::Pixels(size)) => Some(size),
            Some(Dimension::Mode(SizingMode::Content)) => Some(self.content_size(node, horizontal, scope)),
            Some(Dimension::Mode(SizingMode::Fill)) | None => None,
        }
    }

    fn fit_node(&mut self, node: &ScreenNode, cell: Rect, scope: Option<&RepeatScope>) -> Rect {
        let width = self.dimension_size(node, true, scope).unwrap_or(cell.w).min(cell.w);
        let height = self.dimension_size(node, false, scope).unwrap_or(cell.h).min(cell.h);
        let horizontal = match node.anchor_x.unwrap_or(AnchorX::Left) {
            AnchorX::Left => 0.0,
            AnchorX::Center => 0.5,
            AnchorX::Right => 1.0,
        };
        let vertical = match node.anchor_y.unwrap_or(AnchorY::Top) {
            AnchorY::Top => 0.0,
            AnchorY::Center => 0.5,
            AnchorY::Bottom => 1.0,
        };
        Rect::new(cell.x + (cell.w - width) * horizontal, cell.y + (cell.h - height) * vertical, width, height)
    }

    fn content_size(&mut self, node: &ScreenNode, horizontal: bool, scope: Option<&RepeatScope>) -> f32 {
        match node.kind {
            NodeType::Text | NodeType::Button | NodeType::Toggle => {
                let text = node
                    .text
                    .as_deref()
                    .map(|text| interpolate_screen_text(text, self.input.variables, scope))
                    .or_else(|| {
                        node.binding
                            .as_deref()
                            .and_then(|binding| screen_binding(self.input.variables, binding, scope))
                            .map(value_text)
                    })
                    .unwrap_or_default();
                let measured = self.fonts.measure(&text, 24.0, self.input.interface_family, None);
                if horizontal {
                    measured.width + if node.kind == NodeType::Text { 0.0 } else { 24.0 }
                } else {
                    measured.height + if node.kind == NodeType::Text { 0.0 } else { 16.0 }
                }
            }
            NodeType::Image => {
                let name = node.image.as_deref().or_else(|| {
                    node.binding
                        .as_deref()
                        .and_then(|binding| screen_binding(self.input.variables, binding, scope))
                        .and_then(|value| match value {
                            Value::Str(name) => Some(name.as_str()),
                            _ => None,
                        })
                });
                match name.map(|name| (self.input.image)(name)) {
                    Some(ImageInfo::Ready(width, height)) => {
                        if horizontal {
                            width as f32
                        } else {
                            height as f32
                        }
                    }
                    _ => 64.0,
                }
            }
            _ => {
                if horizontal {
                    120.0
                } else {
                    40.0
                }
            }
        }
    }

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
        if self.input.game.screen == Screen::Title {
            if let Some(name) = &self.input.title.background {
                match (self.input.image)(name) {
                    ImageInfo::Ready(iw, ih) => {
                        let rect = cover_rect((iw as f32, ih as f32), (w, h));
                        layer.push(Draw::Image { name: name.clone(), rect, opacity: 1.0 });
                    }
                    ImageInfo::Missing => {}
                    ImageInfo::Loading => {}
                }
            }
        } else if let Some(words) = &stage.background {
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
        if self.input.game.screen != Screen::Title {
            for (sprite, (_, rect)) in stage.sprites.iter().zip(sprite_rects(stage, self.input.image, (w, h))) {
                let mut words = vec![sprite.tag.clone()];
                words.extend(sprite.attrs.iter().cloned());
                let name = image_name(&words);
                match (self.input.image)(&name) {
                    ImageInfo::Ready(_, _) => layer.push(Draw::Image { name, rect, opacity: 1.0 }),
                    ImageInfo::Missing => {
                        let fill = Color::rgba(255, 255, 255, 34);
                        layer.push(Draw::Fill { rect, top: fill, bottom: fill.with_alpha(0.3), radius: 18.0 });
                        let label = Rect::new(rect.x, rect.y + 24.0, rect.w, 40.0);
                        layer.push(self.ui_text(placeholder_label(&name), label, 20.0, gray(0.85), TextAlign::Center));
                    }
                    ImageInfo::Loading => {}
                }
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
            Shown::Pause { .. } | Shown::OpenLoad | Shown::OpenSettings | Shown::Quit | Shown::Nothing => {}
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
                Action::Open(Screen::Save) => self.input.game.save().is_some(),
                _ => true,
            };
            let hovered = r.contains(self.input.mouse.0, self.input.mouse.1);
            let color = match (action.clone(), active, hovered) {
                (Action::Toggle(_), true, _) => self.input.theme.menu.accent.0,
                (_, false, _) => gray(0.4),
                (_, true, true) => Color::WHITE,
                (_, true, false) => gray(0.7),
            };
            layer.push(self.ui_text(label, r, 17.0, color, TextAlign::Center));
            if active || action != Action::Open(Screen::Save) {
                self.hotspots.push(Hotspot { rect: r, action });
            }
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
        let screen = self.input.game.screen;
        if screen == Screen::Title {
            return self.title_screen();
        }
        let mut layer = Vec::new();
        let (top, bottom) = theme.menu.fill();
        layer.push(Draw::Fill { rect: Rect::new(0.0, 0.0, w, h), top, bottom, radius: 0.0 });
        let title = match screen {
            Screen::Menu | Screen::Game => "Menu",
            Screen::Title => unreachable!(),
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
            Screen::Title => unreachable!(),
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

    fn title_screen(&mut self) -> Layer {
        let (w, h) = self.size();
        let menu = &self.input.theme.menu;
        let (top, bottom) = menu.fill();
        let mut layer = vec![Draw::Fill { rect: Rect::new(0.0, 0.0, w, h), top, bottom, radius: 0.0 }];
        let logo =
            self.input.title.logo.as_ref().filter(|logo| matches!((self.input.image)(logo), ImageInfo::Ready(_, _)));
        if let Some(logo) = logo {
            if let ImageInfo::Ready(iw, ih) = (self.input.image)(logo) {
                let height = 170.0;
                let width = (height * iw as f32 / ih as f32).min(w * 0.8);
                let rect = Rect::new((w - width) / 2.0, h * 0.12, width, height);
                layer.push(Draw::Image { name: logo.clone(), rect, opacity: 1.0 });
            }
        } else {
            let rect = Rect::new(0.0, h * 0.18, w, 64.0);
            let mut title = self.ui_text(self.input.title_text, rect, 48.0, menu.accent.0, TextAlign::Center);
            if let Draw::Text(spec) = &mut title {
                spec.bold = true;
            }
            layer.push(title);
        }
        let visible: Vec<_> = self
            .input
            .title
            .buttons
            .iter()
            .filter_map(|button| match button {
                TitleButton::Start => Some(("Start", Action::Start)),
                TitleButton::Continue if self.input.slots.iter().any(Option::is_some) => {
                    Some(("Continue", Action::Continue))
                }
                TitleButton::Continue => None,
                TitleButton::Load => Some(("Load", Action::Open(Screen::Load))),
                TitleButton::Settings => Some(("Settings", Action::Open(Screen::Settings))),
                TitleButton::Quit if self.input.can_quit => Some(("Quit", Action::Quit)),
                TitleButton::Quit => None,
            })
            .collect();
        let bh = 48.0;
        let gap = 10.0;
        let mut y = (h * 0.48).max(220.0);
        for (label, action) in visible {
            let rect = Rect::new((w - 320.0) / 2.0, y, 320.0, bh);
            self.button(&mut layer, rect, label, action, true);
            y += bh + gap;
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
            ("Main menu", Action::MainMenu),
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
            ("Skip", if s.skip_unseen { "All text" } else { "Read text" }.to_string(), Setting::Skip),
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
        let row = Rect::new(left, y, 300.0, 52.0);
        layer.push(self.ui_text("Language", row, 26.0, theme.menu.text_color.0, TextAlign::Center));
        self.button(layer, Rect::new(left + 320.0, y, 52.0, 52.0), "-", Action::Language(-1), true);
        layer.push(self.ui_text(
            self.input.game.language_label(),
            Rect::new(left + 380.0, y, 180.0, 52.0),
            24.0,
            theme.menu.accent.0,
            TextAlign::Center,
        ));
        self.button(layer, Rect::new(left + 568.0, y, 52.0, 52.0), "+", Action::Language(1), true);
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

fn intersect(rect: Rect, clip: Rect) -> Rect {
    let x = rect.x.max(clip.x);
    let y = rect.y.max(clip.y);
    let right = (rect.x + rect.w).min(clip.x + clip.w);
    let bottom = (rect.y + rect.h).min(clip.y + clip.h);
    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

fn screen_panel(layer: &mut Layer, rect: Rect, fill: Color, border: Color) {
    layer.push(Draw::fill(rect, border));
    layer.push(Draw::fill(rect.inset(2.0), fill));
}

fn screen_binding<'a>(
    variables: &'a BTreeMap<String, Value>,
    path: &str,
    scope: Option<&'a RepeatScope>,
) -> Option<&'a Value> {
    let parts = screens::parse_binding(path).ok()?;
    let (name, rest) = screens::binding_root(&parts)?;
    let root = match name.as_str() {
        "item" => &scope?.item,
        "index" => &scope?.index,
        "key" => scope?.key.as_ref()?,
        _ => variables.get(&name)?,
    };
    screens::read_binding(root, rest)
}

fn interpolate_screen_text(text: &str, variables: &BTreeMap<String, Value>, scope: Option<&RepeatScope>) -> String {
    let mut result = String::new();
    let mut tail = text;
    while let Some((start, end)) = screens::next_interpolation(tail) {
        result.push_str(&tail[..start]);
        if let Some(value) = screen_binding(variables, &tail[start + 1..end], scope) {
            result.push_str(&value_text(value));
        }
        tail = &tail[end + 1..];
    }
    result.push_str(tail);
    result
}

fn value_text(value: &Value) -> String {
    match value {
        Value::None => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Str(value) => value.clone(),
        Value::List(_) | Value::Dict(_) => String::new(),
    }
}

fn number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
        _ => None,
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
        Setting::Skip => settings.skip_unseen = !settings.skip_unseen,
    }
}

fn step_volume(v: f32, d: f32) -> f32 {
    ((v * 10.0).round() + d).clamp(0.0, 10.0) / 10.0
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    static EMPTY_SCREENS: BTreeMap<String, ScreenDefinition> = BTreeMap::new();
    static EMPTY_VARIABLES: BTreeMap<String, Value> = BTreeMap::new();

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
        let title = Title::default();
        let slots = [None, Some("Eileen: Hello.".to_string()), None, None, None, None];
        let input = ViewInput {
            game,
            theme: &theme,
            title: &title,
            title_text: "Test",
            size: (1280.0, 720.0),
            mouse,
            image: &images,
            slots: &slots,
            backlog_scroll: 0,
            dialogue_family: None,
            interface_family: None,
            can_quit: true,
            notice: None,
            screens: &EMPTY_SCREENS,
            screen_stack: &[],
            variables: &EMPTY_VARIABLES,
            focused_custom: None,
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
        let loads: Vec<Action> =
            hotspots.iter().map(|h| h.action.clone()).filter(|a| matches!(a, Action::Load(_))).collect();
        assert_eq!(loads, [Action::Load(1)], "only filled slots can be loaded");
        assert!(hotspots.iter().any(|h| h.action == Action::Back));
    }

    #[test]
    fn title_screen_shows_start_and_continue_only_when_saves_exist() {
        let mut started_game = game();
        started_game.screen = Screen::Title;
        let (_, hotspots) = view(&started_game, (0.0, 0.0));
        assert!(hotspots.iter().any(|h| h.action == Action::Start));
        assert!(hotspots.iter().any(|h| h.action == Action::Continue));

        let mut empty_game = game();
        empty_game.screen = Screen::Title;
        let theme = Theme::default();
        let title = Title { buttons: vec![TitleButton::Start, TitleButton::Continue], ..Title::default() };
        let slots: [Option<String>; SLOTS] = std::array::from_fn(|_| None);
        let input = ViewInput {
            game: &empty_game,
            theme: &theme,
            title: &title,
            title_text: "Test",
            size: (1280.0, 720.0),
            mouse: (0.0, 0.0),
            image: &images,
            slots: &slots,
            backlog_scroll: 0,
            dialogue_family: None,
            interface_family: None,
            can_quit: false,
            notice: None,
            screens: &EMPTY_SCREENS,
            screen_stack: &[],
            variables: &EMPTY_VARIABLES,
            focused_custom: None,
        };
        let (_, hotspots) = build(&input, &mut Fonts::new());
        assert!(!hotspots.iter().any(|h| h.action == Action::Continue));
    }

    #[test]
    fn quick_menu_save_is_inert_while_saving_is_disabled() {
        let sources = [(PathBuf::from("script/a.kai"), "label splashscreen:\n    \"Splash\"\n".to_string())];
        let project = kairos_project::Project::from_sources("name = \"t\"", sources).unwrap();
        let mut splash = Game::new(project.program).unwrap();
        splash.start("splashscreen");
        let (_, hotspots) = view(&splash, (0.0, 0.0));
        assert!(!hotspots.iter().any(|h| h.action == Action::Open(Screen::Save)));
        let (_, hotspots) = view(&game(), (0.0, 0.0));
        assert!(hotspots.iter().any(|h| h.action == Action::Open(Screen::Save)));
    }

    #[test]
    fn custom_screen_renders_bound_values_and_selectable_list_items() {
        let mut game = game();
        game.show_custom_screen("phone");
        let screen = kairos_project::screens::parse_screen(
            r#"
            id = "phone"
            modal = true
            [[nodes]]
            id = "root"
            type = "column"
            [[nodes]]
            id = "title"
            parent = "root"
            type = "text"
            text = "Gold: [gold]"
            [[nodes]]
            id = "items"
            parent = "root"
            type = "list"
            binding = "inventory"
            target = "selected"
            [[nodes]]
            id = "party"
            parent = "root"
            type = "repeat"
            binding = "members"
            [[nodes]]
            id = "member_name"
            parent = "party"
            type = "text"
            binding = "item.name"
            [[nodes]]
            id = "member_button"
            parent = "party"
            type = "button"
            text = "Select"
        "#,
        )
        .unwrap();
        let screens = BTreeMap::from([("phone".into(), screen)]);
        let variables = BTreeMap::from([
            ("gold".into(), Value::Int(4)),
            ("inventory".into(), Value::List(vec![Value::Str("key".into()), Value::Str("map".into())])),
            ("selected".into(), Value::Str("key".into())),
            (
                "members".into(),
                Value::List(vec![
                    Value::Dict(BTreeMap::from([("name".into(), Value::Str("Mira".into()))])),
                    Value::Dict(BTreeMap::from([("name".into(), Value::Str("Kai".into()))])),
                ]),
            ),
        ]);
        let theme = Theme::default();
        let title = Title::default();
        let slots: [Option<String>; SLOTS] = std::array::from_fn(|_| None);
        let input = ViewInput {
            game: &game,
            theme: &theme,
            title: &title,
            title_text: "Test",
            size: (1280.0, 720.0),
            mouse: (0.0, 0.0),
            image: &images,
            slots: &slots,
            backlog_scroll: 0,
            dialogue_family: None,
            interface_family: None,
            can_quit: false,
            notice: None,
            screens: &screens,
            screen_stack: game.custom_screens(),
            variables: &variables,
            focused_custom: None,
        };
        let (frame, hotspots) = build(&input, &mut Fonts::new());
        assert!(texts(frame.ui.last().unwrap()).contains(&"Gold: 4"));
        let list_rows: Vec<_> = hotspots
            .iter()
            .filter(
                |hotspot| matches!(&hotspot.action, Action::Custom { node, selection: Some(_), .. } if node == "items"),
            )
            .collect();
        assert_eq!(list_rows.len(), 2);
        assert!(list_rows[0].rect.y < list_rows[1].rect.y);
        let rendered = texts(frame.ui.last().unwrap());
        assert!(rendered.contains(&"Mira"));
        assert!(rendered.contains(&"Kai"));
        let repeated: Vec<_> = hotspots
            .iter()
            .filter(|hotspot| matches!(&hotspot.action, Action::Custom { node, .. } if node == "member_button"))
            .collect();
        assert_eq!(repeated.len(), 2);
        assert_ne!(repeated[0].action, repeated[1].action);
    }

    #[test]
    fn settings_lists_source_language_and_language_controls() {
        let mut game = game();
        game.screen = Screen::Settings;
        let (frame, hotspots) = view(&game, (0.0, 0.0));
        assert!(texts(&frame.ui[1]).contains(&"Language"));
        assert!(texts(&frame.ui[1]).contains(&"Source"));
        assert!(hotspots.iter().any(|hotspot| hotspot.action == Action::Language(-1)));
        assert!(hotspots.iter().any(|hotspot| hotspot.action == Action::Language(1)));
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
        adjust(&mut s, Setting::Skip, -1);
        assert!(s.skip_unseen);
        adjust(&mut s, Setting::Skip, 1);
        assert!(!s.skip_unseen);
    }

    #[test]
    fn long_slot_previews_are_shortened() {
        assert_eq!(ellipsize("short", 10), "short");
        assert_eq!(ellipsize("a much longer preview", 8), "a much…");
    }

    #[test]
    fn sprite_hit_rects_include_only_drawn_sprites_on_the_game_screen() {
        let stage = kairos_vm::Stage {
            background: None,
            sprites: vec![
                kairos_vm::Sprite { tag: "e".into(), attrs: vec!["happy".into()], at: Some("left".into()) },
                kairos_vm::Sprite { tag: "k".into(), attrs: vec![], at: Some("right".into()) },
                kairos_vm::Sprite { tag: "m".into(), attrs: vec![], at: Some("center".into()) },
            ],
        };
        let image = |name: &str| match name {
            "e happy" => ImageInfo::Ready(400, 800),
            "k" => ImageInfo::Missing,
            _ => ImageInfo::Loading,
        };
        assert!(sprite_hit_rects(Screen::Title, &stage, &image, (1280.0, 720.0)).is_empty());
        let rects = sprite_hit_rects(Screen::Game, &stage, &image, (1280.0, 720.0));
        assert_eq!(rects.iter().map(|(tag, _)| tag.as_str()).collect::<Vec<_>>(), ["e", "k"]);
    }

    #[test]
    fn sprite_rects_follow_draw_order() {
        let stage = kairos_vm::Stage {
            background: None,
            sprites: vec![
                kairos_vm::Sprite { tag: "e".into(), attrs: vec!["happy".into()], at: Some("left".into()) },
                kairos_vm::Sprite { tag: "k".into(), attrs: vec![], at: Some("right".into()) },
            ],
        };
        let rects = sprite_rects(&stage, &|_: &str| ImageInfo::Ready(400, 800), (1280.0, 720.0));
        assert_eq!(rects.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["e", "k"]);
        assert!(rects[0].1.x < rects[1].1.x);
    }
}
