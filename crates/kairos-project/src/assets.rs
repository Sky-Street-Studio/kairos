//! Asset names resolve to paths relative to the project's `assets/` directory.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use kairos_script::{Stmt, StmtKind};

pub const ASSETS_DIR: &str = "assets";
pub const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
pub const AUDIO_EXTENSIONS: [&str; 4] = ["ogg", "mp3", "wav", "flac"];
pub const PACK_FILE: &str = "assets.kpak";

#[derive(Debug, Clone, Copy)]
pub struct PackEntry {
    offset: u64,
    size: u64,
}

#[derive(Debug, Clone)]
pub struct PackIndex {
    path: PathBuf,
    entries: BTreeMap<String, PackEntry>,
    key: Option<Vec<u8>>,
}

impl PackIndex {
    pub fn open(path: &Path, key: Option<&str>) -> std::io::Result<PackIndex> {
        let mut file = std::fs::File::open(path)?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        if &magic != b"KPAK0001" {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid kpak header"));
        }
        let count = read_u32(&mut file)? as usize;
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            let path_len = read_u32(&mut file)? as usize;
            let mut path_bytes = vec![0; path_len];
            file.read_exact(&mut path_bytes)?;
            let entry_path = String::from_utf8(path_bytes)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            let offset = read_u64(&mut file)?;
            let size = read_u64(&mut file)?;
            if entry_path.starts_with('/') || entry_path.split('/').any(|part| part == "..") {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid path in kpak index"));
            }
            entries.insert(entry_path, PackEntry { offset, size });
        }
        Ok(PackIndex { path: path.to_path_buf(), entries, key: key.map(str::as_bytes).map(<[u8]>::to_vec) })
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn read(&self, path: &str) -> std::io::Result<Option<Vec<u8>>> {
        let Some(entry) = self.entries.get(path) else { return Ok(None) };
        let size = usize::try_from(entry.size)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "kpak entry is too large"))?;
        let mut bytes = vec![0; size];
        let mut file = std::fs::File::open(&self.path)?;
        file.seek(SeekFrom::Start(entry.offset))?;
        file.read_exact(&mut bytes)?;
        xor_bytes(&mut bytes, self.key.as_deref());
        Ok(Some(bytes))
    }
}

pub fn pack_entry(data: &[u8], wanted: &str, key: Option<&str>) -> std::io::Result<Option<Vec<u8>>> {
    let mut cursor = std::io::Cursor::new(data);
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic)?;
    if &magic != b"KPAK0001" {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid kpak header"));
    }
    let count = read_cursor_u32(&mut cursor)?;
    for _ in 0..count {
        let path_len = read_cursor_u32(&mut cursor)? as usize;
        let mut path = vec![0; path_len];
        cursor.read_exact(&mut path)?;
        let path =
            std::str::from_utf8(&path).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let offset = read_cursor_u64(&mut cursor)? as usize;
        let size = read_cursor_u64(&mut cursor)? as usize;
        if path == wanted {
            let end = offset
                .checked_add(size)
                .filter(|end| *end <= data.len())
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "kpak entry is outside archive"))?;
            let mut bytes = data[offset..end].to_vec();
            xor_bytes(&mut bytes, key.map(str::as_bytes));
            return Ok(Some(bytes));
        }
    }
    Ok(None)
}

fn read_cursor_u32(cursor: &mut std::io::Cursor<&[u8]>) -> std::io::Result<u32> {
    let mut bytes = [0; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_cursor_u64(cursor: &mut std::io::Cursor<&[u8]>) -> std::io::Result<u64> {
    let mut bytes = [0; 8];
    cursor.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn pack(root: &Path, destination: &Path, key: Option<&str>) -> std::io::Result<Vec<String>> {
    let paths = collect_paths(root)?;
    let header_size = 12u64 + paths.iter().map(|path| 4 + path.len() as u64 + 16).sum::<u64>();
    let mut data_offset = header_size;
    let mut entries = Vec::with_capacity(paths.len());
    for path in &paths {
        let size = std::fs::metadata(root.join(path))?.len();
        entries.push((path, data_offset, size));
        data_offset += size;
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(destination)?;
    file.write_all(b"KPAK0001")?;
    file.write_all(&(paths.len() as u32).to_le_bytes())?;
    for (path, offset, size) in &entries {
        file.write_all(&(path.len() as u32).to_le_bytes())?;
        file.write_all(path.as_bytes())?;
        file.write_all(&offset.to_le_bytes())?;
        file.write_all(&size.to_le_bytes())?;
    }
    for path in &paths {
        let mut bytes = std::fs::read(root.join(path))?;
        xor_bytes(&mut bytes, key.map(str::as_bytes));
        file.write_all(&bytes)?;
    }
    Ok(paths)
}

fn read_u32(file: &mut std::fs::File) -> std::io::Result<u32> {
    let mut bytes = [0; 4];
    file.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(file: &mut std::fs::File) -> std::io::Result<u64> {
    let mut bytes = [0; 8];
    file.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn xor_bytes(bytes: &mut [u8], key: Option<&[u8]>) {
    if let Some(key) = key.filter(|key| !key.is_empty()) {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte ^= key[index % key.len()];
        }
    }
}

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

/// How many statements load each asset, keyed by path relative to `assets/`. Only the file the
/// player would actually load counts: the first candidate, or each part of a layered sprite.
pub fn usage<'a>(
    scripts: impl IntoIterator<Item = &'a [Stmt]>,
    index: &AssetIndex,
    layers: &BTreeMap<String, LayerConfig>,
) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for stmts in scripts {
        count_usage(stmts, index, layers, &mut counts);
    }
    counts
}

fn count_usage(
    stmts: &[Stmt],
    index: &AssetIndex,
    layers: &BTreeMap<String, LayerConfig>,
    counts: &mut BTreeMap<String, u32>,
) {
    for stmt in stmts {
        let loaded = match &stmt.kind {
            StmtKind::Scene { image: Some(words), .. } => index.image_candidates(words).into_iter().take(1).collect(),
            StmtKind::Show { image, .. } => match layers.get(&image[0]) {
                Some(config) => config.files(&image[0], &image[1..], index),
                None => index.image_candidates(image).into_iter().take(1).collect(),
            },
            StmtKind::Play { asset, .. } => index.audio_candidates(asset).into_iter().take(1).collect(),
            StmtKind::Label { body, .. } => {
                count_usage(body, index, layers, counts);
                Vec::new()
            }
            StmtKind::If(branches) => {
                for branch in branches {
                    count_usage(&branch.body, index, layers, counts);
                }
                Vec::new()
            }
            StmtKind::Menu { choices, .. } => {
                for choice in choices {
                    count_usage(&choice.body, index, layers, counts);
                }
                Vec::new()
            }
            _ => Vec::new(),
        };
        for path in loaded {
            *counts.entry(path).or_insert(0) += 1;
        }
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
    fn kpak_index_reads_entries_with_or_without_xor_obfuscation() {
        let root = std::env::temp_dir().join(format!("kairos-kpak-{}", std::process::id()));
        let assets = root.join("assets");
        std::fs::create_dir_all(assets.join("images")).unwrap();
        std::fs::write(assets.join("images/bg room.png"), b"image bytes").unwrap();
        let packed = root.join(PACK_FILE);
        let paths = pack(&assets, &packed, Some("paper-key")).unwrap();
        assert_eq!(paths, ["images/bg room.png"]);
        let index = PackIndex::open(&packed, Some("paper-key")).unwrap();
        assert_eq!(index.paths().collect::<Vec<_>>(), ["images/bg room.png"]);
        let archive = std::fs::read(&packed).unwrap();
        assert_eq!(
            pack_entry(&archive, "images/bg room.png", Some("paper-key")).unwrap().as_deref(),
            Some(&b"image bytes"[..])
        );
        assert_eq!(index.read("images/bg room.png").unwrap().as_deref(), Some(&b"image bytes"[..]));
        assert_eq!(index.read("missing.png").unwrap(), None);
        std::fs::remove_dir_all(root).unwrap();
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
    fn usage_counts_the_file_each_statement_loads() {
        let index = AssetIndex::new([
            "images/bg room.png",
            "images/bg room.webp",
            "images/bg/room.png",
            "images/sprites/kai/kai smile.png",
            "images/sprites/e/layers.toml",
            "images/sprites/e/base/body.png",
            "images/sprites/e/face/happy.png",
            "images/sprites/e/face/neutral.png",
            "audio/music/theme.ogg",
            "audio/sfx/door.wav",
        ]);
        let layers: BTreeMap<String, LayerConfig> =
            [("e".into(), toml::from_str("order = ['base', 'face']\n[default]\nface = 'neutral'\n").unwrap())].into();
        let (main, _) = kairos_script::parse(
            "label start:\n    scene bg room\n    show kai smile at left\n    show e\n    menu:\n        \"Go\":\n            show e happy\n            play music \"theme.ogg\"\n    if x:\n        play sound \"sfx/door\"\n    # show kai smile\n",
        );
        let (other, _) = kairos_script::parse("label more:\n    scene bg room\n    show missing\n");

        let counts = usage([main.as_slice(), other.as_slice()], &index, &layers);

        assert_eq!(
            counts,
            BTreeMap::from([
                ("images/bg room.png".into(), 2),
                ("images/sprites/kai/kai smile.png".into(), 1),
                ("images/sprites/e/base/body.png".into(), 2),
                ("images/sprites/e/face/neutral.png".into(), 1),
                ("images/sprites/e/face/happy.png".into(), 1),
                ("audio/music/theme.ogg".into(), 1),
                ("audio/sfx/door.wav".into(), 1),
            ])
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
