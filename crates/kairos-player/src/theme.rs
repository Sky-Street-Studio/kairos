//! Game UI styling from `ui/theme.toml`. Every key is optional.

use kairos_render::Color;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hex(pub Color);

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::hex(&s).map(Hex).ok_or_else(|| serde::de::Error::custom(format!("`{s}` is not a #rrggbb colour")))
    }
}

fn hex(s: &str) -> Hex {
    Hex(Color::hex(s).expect("built-in colour"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleButton {
    Start,
    Continue,
    Load,
    Settings,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Title {
    pub title: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub music: Option<String>,
    pub buttons: Vec<TitleButton>,
}

impl Default for Title {
    fn default() -> Self {
        Title {
            title: None,
            background: None,
            logo: None,
            music: None,
            buttons: vec![
                TitleButton::Start,
                TitleButton::Continue,
                TitleButton::Load,
                TitleButton::Settings,
                TitleButton::Quit,
            ],
        }
    }
}

impl Title {
    pub fn parse(text: &str) -> Result<Title, String> {
        toml::from_str(text).map_err(|e| format!("ui/title.toml: {}", e.message()))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub fonts: Fonts,
    pub textbox: Textbox,
    pub nameplate: Nameplate,
    pub choice: Choice,
    pub menu: Menu,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Fonts {
    /// Font files relative to `assets/`, used for dialogue and for everything else.
    pub dialogue: Option<String>,
    pub interface: Option<String>,
    pub fallback: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Textbox {
    pub color: Hex,
    pub color_bottom: Option<Hex>,
    pub opacity: f32,
    pub height: f32,
    pub margin: f32,
    pub padding: f32,
    pub radius: f32,
    pub text_color: Hex,
    pub text_size: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Nameplate {
    pub color: Hex,
    pub color_bottom: Option<Hex>,
    pub opacity: f32,
    /// Overrides the speaking character's colour.
    pub text_color: Option<Hex>,
    pub text_size: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Choice {
    pub color: Hex,
    pub color_bottom: Option<Hex>,
    pub opacity: f32,
    pub hover_color: Hex,
    pub text_color: Hex,
    pub text_size: f32,
    pub width: f32,
    pub height: f32,
    pub spacing: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Menu {
    pub color: Hex,
    pub color_bottom: Option<Hex>,
    pub opacity: f32,
    pub accent: Hex,
    pub text_color: Hex,
}

/// A panel's colours with opacity applied; `bottom` differs from `top` for a vertical gradient.
fn gradient(color: Hex, color_bottom: Option<Hex>, opacity: f32) -> (Color, Color) {
    (color.0.with_alpha(opacity), color_bottom.unwrap_or(color).0.with_alpha(opacity))
}

macro_rules! panel {
    ($($t:ty),*) => {$(
        impl $t {
            pub fn fill(&self) -> (Color, Color) {
                gradient(self.color, self.color_bottom, self.opacity)
            }
        }
    )*};
}

panel!(Textbox, Nameplate, Choice, Menu);

impl Default for Textbox {
    fn default() -> Self {
        Textbox {
            color: hex("#1b1b2b"),
            color_bottom: Some(hex("#0d0d16")),
            opacity: 0.88,
            height: 190.0,
            margin: 24.0,
            padding: 28.0,
            radius: 14.0,
            text_color: hex("#f2f2f7"),
            text_size: 28.0,
        }
    }
}

impl Default for Nameplate {
    fn default() -> Self {
        Nameplate {
            color: hex("#2c2c48"),
            color_bottom: None,
            opacity: 0.95,
            text_color: None,
            text_size: 26.0,
            radius: 10.0,
        }
    }
}

impl Default for Choice {
    fn default() -> Self {
        Choice {
            color: hex("#23233a"),
            color_bottom: Some(hex("#17172a")),
            opacity: 0.92,
            hover_color: hex("#454578"),
            text_color: hex("#f2f2f7"),
            text_size: 26.0,
            width: 720.0,
            height: 58.0,
            spacing: 16.0,
            radius: 12.0,
        }
    }
}

impl Default for Menu {
    fn default() -> Self {
        Menu {
            color: hex("#0b0b12"),
            color_bottom: None,
            opacity: 0.86,
            accent: hex("#9db8ff"),
            text_color: hex("#f2f2f7"),
        }
    }
}

impl Theme {
    pub fn parse(text: &str) -> Result<Theme, String> {
        toml::from_str(text).map_err(|e| format!("ui/theme.toml: {}", e.message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_title_uses_the_default_buttons() {
        assert_eq!(Title::parse("").unwrap(), Title::default());
    }

    #[test]
    fn title_keys_are_optional_and_button_names_are_checked() {
        let title =
            Title::parse("title = \"Hello\"\nbackground = \"bg title\"\nbuttons = [\"start\", \"load\"]\n").unwrap();
        assert_eq!(title.title.as_deref(), Some("Hello"));
        assert_eq!(title.background.as_deref(), Some("bg title"));
        assert_eq!(title.buttons, [TitleButton::Start, TitleButton::Load]);
        assert!(Title::parse("buttons = [\"play\"]\n").is_err());
    }

    #[test]
    fn an_empty_theme_is_the_default() {
        assert_eq!(Theme::parse("").unwrap(), Theme::default());
    }

    #[test]
    fn unset_keys_keep_their_section_defaults() {
        let theme = Theme::parse("[textbox]\ncolor = \"#ff0000\"\nopacity = 0.5\n").unwrap();
        assert_eq!(theme.textbox.color, Hex(Color::rgb(255, 0, 0)));
        assert_eq!(theme.textbox.opacity, 0.5);
        assert_eq!(theme.textbox.height, Textbox::default().height);
        assert_eq!(theme.textbox.color_bottom, Textbox::default().color_bottom);
        assert_eq!(theme.choice, Choice::default());
    }

    #[test]
    fn gradients_apply_opacity_to_both_ends() {
        let theme =
            Theme::parse("[nameplate]\ncolor = \"#000000\"\ncolor_bottom = \"#ffffff\"\nopacity = 0.5\n").unwrap();
        assert_eq!(theme.nameplate.fill(), (Color::rgba(0, 0, 0, 128), Color::rgba(255, 255, 255, 128)));
        let solid = Theme::default().menu.fill();
        assert_eq!(solid.0, solid.1);
    }

    #[test]
    fn fonts_and_text_colours_are_read() {
        let theme = Theme::parse(
            "[fonts]\ndialogue = \"fonts/Serif.ttf\"\nfallback = [\"fonts/CJK.otf\"]\n[nameplate]\ntext_color = \"#abc\"\n[menu]\naccent = \"#112233\"\n",
        )
        .unwrap();
        assert_eq!(theme.fonts.dialogue.as_deref(), Some("fonts/Serif.ttf"));
        assert_eq!(theme.fonts.fallback, ["fonts/CJK.otf"]);
        assert_eq!(theme.nameplate.text_color, Some(Hex(Color::rgb(0xaa, 0xbb, 0xcc))));
        assert_eq!(theme.menu.accent, Hex(Color::rgb(0x11, 0x22, 0x33)));
    }

    #[test]
    fn bad_colours_and_unknown_keys_are_errors() {
        assert!(Theme::parse("[textbox]\ncolor = \"red\"\n").unwrap_err().contains("not a #rrggbb"));
        assert!(Theme::parse("[textbox]\ncolour = \"#fff\"\n").is_err());
    }
}
