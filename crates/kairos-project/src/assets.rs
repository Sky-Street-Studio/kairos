//! Asset names resolve to paths relative to the project's `assets/` directory.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const ASSETS_DIR: &str = "assets";
pub const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
pub const AUDIO_EXTENSIONS: [&str; 4] = ["ogg", "mp3", "wav", "flac"];

#[derive(Debug, Clone, Default)]
pub struct AssetIndex {
    paths: Vec<String>,
}

impl AssetIndex {
    pub fn new(paths: impl IntoIterator<Item = impl AsRef<str>>) -> AssetIndex {
        let mut paths: Vec<_> = paths
            .into_iter()
            .map(|path| path.as_ref().trim_start_matches("assets/").replace('\\', "/"))
            .filter(|path| !path.is_empty())
            .collect();
        paths.sort();
        paths.dedup();
        AssetIndex { paths }
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    pub fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|candidate| candidate == path)
    }

    pub fn image_candidates(&self, words: &[String]) -> Vec<String> {
        let name = words.join(" ");
        let mut paths: Vec<_> = self
            .paths
            .iter()
            .filter(|path| {
                path.strip_prefix("images/")
                    .is_some_and(|path| file_stem(path) == name && extension_rank(path, &IMAGE_EXTENSIONS).is_some())
            })
            .cloned()
            .collect();
        paths.sort_by(|a, b| {
            let (ad, ae) = (parent(a), extension_rank(a, &IMAGE_EXTENSIONS).unwrap_or(usize::MAX));
            let (bd, be) = (parent(b), extension_rank(b, &IMAGE_EXTENSIONS).unwrap_or(usize::MAX));
            ad.cmp(bd).then(ae.cmp(&be))
        });
        paths
    }

    pub fn audio_candidates(&self, asset: &str) -> Vec<String> {
        if asset.contains('/') {
            let base = format!("audio/{asset}");
            if Path::new(asset).extension().is_some() {
                return self.paths.iter().filter(|path| *path == &base).cloned().collect();
            }
            return AUDIO_EXTENSIONS
                .iter()
                .map(|extension| format!("{base}.{extension}"))
                .filter(|path| self.contains(path))
                .collect();
        }

        if Path::new(asset).extension().is_some() {
            let mut paths: Vec<_> = self
                .paths
                .iter()
                .filter(|path| path.strip_prefix("audio/").is_some_and(|path| file_name(path) == asset))
                .cloned()
                .collect();
            paths.sort();
            return paths;
        }

        let mut paths: Vec<_> = self
            .paths
            .iter()
            .filter(|path| {
                path.strip_prefix("audio/")
                    .is_some_and(|path| file_stem(path) == asset && extension_rank(path, &AUDIO_EXTENSIONS).is_some())
            })
            .cloned()
            .collect();
        paths.sort_by(|a, b| {
            let (ad, ae) = (parent(a), extension_rank(a, &AUDIO_EXTENSIONS).unwrap_or(usize::MAX));
            let (bd, be) = (parent(b), extension_rank(b, &AUDIO_EXTENSIONS).unwrap_or(usize::MAX));
            ae.cmp(&be).then(ad.cmp(bd))
        });
        paths
    }

    pub fn duplicate_images(&self) -> BTreeMap<String, Vec<String>> {
        self.duplicates("images/", &IMAGE_EXTENSIONS)
    }

    pub fn duplicate_audio(&self) -> BTreeMap<String, Vec<String>> {
        self.duplicates("audio/", &AUDIO_EXTENSIONS)
    }

    pub fn layer_manifest(&self, tag: &str) -> String {
        format!("images/sprites/{tag}/layers.toml")
    }

    fn duplicates(&self, prefix: &str, extensions: &[&str]) -> BTreeMap<String, Vec<String>> {
        let mut by_name = BTreeMap::<String, Vec<String>>::new();
        for path in &self.paths {
            let Some(relative) = path.strip_prefix(prefix) else { continue };
            if extension_rank(relative, extensions).is_none() {
                continue;
            }
            by_name.entry(file_stem(relative).to_string()).or_default().push(path.clone());
        }
        by_name.retain(|_, paths| paths.iter().map(|path| parent(path)).collect::<BTreeSet<_>>().len() > 1);
        by_name
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LayerConfig {
    pub order: Vec<String>,
    #[serde(default)]
    pub default: BTreeMap<String, String>,
}

impl LayerConfig {
    pub fn has_attribute(&self, tag: &str, attr: &str, index: &AssetIndex) -> bool {
        self.order.iter().any(|group| {
            let prefix = format!("images/sprites/{tag}/{group}/");
            index.paths().iter().any(|path| {
                path.strip_prefix(&prefix).is_some_and(|path| !path.contains('/'))
                    && file_stem(path) == attr
                    && extension_rank(path, &IMAGE_EXTENSIONS).is_some()
            })
        })
    }

    pub fn files(&self, tag: &str, attrs: &[String], index: &AssetIndex) -> Vec<String> {
        let mut files = Vec::new();
        for group in &self.order {
            let prefix = format!("images/sprites/{tag}/{group}/");
            let group_files: Vec<_> = index
                .paths()
                .iter()
                .filter(|path| {
                    path.strip_prefix(&prefix).is_some_and(|path| !path.contains('/'))
                        && extension_rank(path, &IMAGE_EXTENSIONS).is_some()
                })
                .collect();
            let stems: BTreeSet<_> = group_files.iter().map(|path| file_stem(path)).collect();
            let selected = attrs
                .iter()
                .find(|attr| group_files.iter().any(|path| file_stem(path) == attr.as_str()))
                .cloned()
                .or_else(|| self.default.get(group).cloned())
                .or_else(|| (stems.len() == 1).then(|| (*stems.first().unwrap()).to_string()));
            if let Some(selected) = selected {
                let mut candidates: Vec<_> =
                    group_files.into_iter().filter(|path| file_stem(path) == selected).cloned().collect();
                candidates.sort_by_key(|path| extension_rank(path, &IMAGE_EXTENSIONS).unwrap_or(usize::MAX));
                if let Some(path) = candidates.into_iter().next() {
                    files.push(path);
                }
            }
        }
        files
    }
}

pub fn collect_paths(root: &Path) -> std::io::Result<Vec<String>> {
    fn visit(root: &Path, dir: &Path, paths: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                visit(root, &path, paths)?;
            } else if kind.is_file() {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                paths.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }

    let mut paths = Vec::new();
    if root.is_dir() {
        visit(root, root, &mut paths)?;
    }
    paths.sort();
    Ok(paths)
}

/// `e happy` is searched by stem beneath every directory in `images/`.
pub fn image_candidates(words: &[String]) -> Vec<String> {
    let name = words.join(" ");
    IMAGE_EXTENSIONS.iter().map(|ext| format!("images/{name}.{ext}")).collect()
}

/// Audio names containing `/` stay relative to `audio/`; bare names search recursively.
pub fn audio_candidates(asset: &str) -> Vec<String> {
    if asset.contains('/') {
        let file = asset.rsplit('/').next().unwrap_or(asset);
        if Path::new(file).extension().is_some() {
            vec![format!("audio/{asset}")]
        } else {
            AUDIO_EXTENSIONS.iter().map(|ext| format!("audio/{asset}.{ext}")).collect()
        }
    } else if Path::new(asset).extension().is_some() {
        vec![format!("audio/{asset}")]
    } else {
        AUDIO_EXTENSIONS.iter().map(|ext| format!("audio/{asset}.{ext}")).collect()
    }
}

fn extension_rank(path: &str, extensions: &[&str]) -> Option<usize> {
    let extension = Path::new(path).extension()?.to_str()?;
    extensions.iter().position(|candidate| *candidate == extension)
}

fn file_stem(path: &str) -> &str {
    Path::new(path).file_stem().and_then(|stem| stem.to_str()).unwrap_or("")
}

fn file_name(path: &str) -> &str {
    Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or("")
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map(|(directory, _)| directory).unwrap_or("")
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

    #[test]
    fn image_index_finds_stems_recursively_and_orders_extensions_per_directory() {
        let index = AssetIndex::new([
            "assets/images/bg/room.webp",
            "images/room.jpg",
            "images/room.png",
            "images/deep/room.png",
        ]);
        assert_eq!(
            index.image_candidates(&words("room")),
            ["images/room.png", "images/room.jpg", "images/bg/room.webp", "images/deep/room.png"]
        );
    }

    #[test]
    fn audio_index_matches_bare_names_across_directories() {
        let index = AssetIndex::new([
            "audio/music/theme.mp3",
            "audio/theme.ogg",
            "audio/voice/theme.ogg",
            "audio/music/intro.ogg",
        ]);
        assert_eq!(
            index.audio_candidates("theme"),
            ["audio/theme.ogg", "audio/voice/theme.ogg", "audio/music/theme.mp3"]
        );
        assert_eq!(index.audio_candidates("intro.ogg"), ["audio/music/intro.ogg"]);
        assert_eq!(index.audio_candidates("music/intro"), ["audio/music/intro.ogg"]);
    }

    #[test]
    fn layered_groups_select_attributes_then_defaults_then_single_files() {
        let paths = [
            "images/sprites/e/base/body.png",
            "images/sprites/e/face/happy.png",
            "images/sprites/e/face/neutral.png",
            "images/sprites/e/outfit/casual.png",
        ];
        let config: LayerConfig =
            toml::from_str("order = ['base', 'outfit', 'face']\n[default]\noutfit = 'casual'\nface = 'neutral'\n")
                .unwrap();
        assert_eq!(
            config.files("e", &["happy".into()], &AssetIndex::new(paths)),
            ["images/sprites/e/base/body.png", "images/sprites/e/outfit/casual.png", "images/sprites/e/face/happy.png"]
        );
    }

    #[test]
    fn duplicate_asset_names_ignore_extension_variants_in_one_directory() {
        let index = AssetIndex::new([
            "images/bg room.png",
            "images/bg room.webp",
            "images/other/bg room.jpg",
            "audio/theme.ogg",
            "audio/music/theme.mp3",
        ]);
        assert_eq!(
            index.duplicate_images(),
            [(
                "bg room".into(),
                vec!["images/bg room.png".into(), "images/bg room.webp".into(), "images/other/bg room.jpg".into()]
            )]
            .into()
        );
        assert_eq!(
            index.duplicate_audio(),
            [("theme".into(), vec!["audio/music/theme.mp3".into(), "audio/theme.ogg".into()])].into()
        );
        let variants =
            AssetIndex::new(["images/bg room.png", "images/bg room.webp", "audio/theme.ogg", "audio/theme.mp3"]);
        assert!(variants.duplicate_images().is_empty());
        assert!(variants.duplicate_audio().is_empty());
    }
}
