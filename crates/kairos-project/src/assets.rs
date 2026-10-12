//! The asset naming rule. The editor resolves assets the same way, so changes here are breaking.
//!
//! Paths are relative to the project's `assets/` directory (on the web, to `assetBaseUrl`).

pub const ASSETS_DIR: &str = "assets";
pub const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
pub const AUDIO_EXTENSIONS: [&str; 4] = ["ogg", "mp3", "wav", "flac"];

/// `["bg", "room"]` becomes `images/bg room.png`, `images/bg room.jpg`, ... in lookup order.
pub fn image_candidates(words: &[String]) -> Vec<String> {
    let name = words.join(" ");
    IMAGE_EXTENSIONS.iter().map(|ext| format!("images/{name}.{ext}")).collect()
}

/// `theme.ogg` becomes `audio/theme.ogg`; a name without an extension tries each audio extension.
pub fn audio_candidates(asset: &str) -> Vec<String> {
    let file = asset.rsplit('/').next().unwrap_or(asset);
    if file.contains('.') {
        vec![format!("audio/{asset}")]
    } else {
        AUDIO_EXTENSIONS.iter().map(|ext| format!("audio/{asset}.{ext}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split(' ').map(String::from).collect()
    }

    #[test]
    fn image_words_join_with_spaces_and_try_each_extension_in_order() {
        assert_eq!(
            image_candidates(&words("bg room")),
            ["images/bg room.png", "images/bg room.jpg", "images/bg room.jpeg", "images/bg room.webp"]
        );
        assert_eq!(image_candidates(&words("e happy"))[0], "images/e happy.png");
    }

    #[test]
    fn audio_keeps_an_explicit_extension() {
        assert_eq!(audio_candidates("theme.ogg"), ["audio/theme.ogg"]);
        assert_eq!(audio_candidates("sfx/door.wav"), ["audio/sfx/door.wav"]);
    }

    #[test]
    fn audio_without_an_extension_tries_each_format() {
        assert_eq!(
            audio_candidates("theme"),
            ["audio/theme.ogg", "audio/theme.mp3", "audio/theme.wav", "audio/theme.flac"]
        );
        assert_eq!(audio_candidates("v1.0/line")[0], "audio/v1.0/line.ogg");
    }
}
