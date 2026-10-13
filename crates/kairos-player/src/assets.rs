//! Asset fetching. Native reads files right away; the web fetches in the background. Both report
//! results through [`Assets::poll`] so the player handles them the same way.

#[cfg(not(target_arch = "wasm32"))]
use kairos_project::assets::collect_paths;
use kairos_project::assets::{AssetIndex, LayerConfig};

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// An image by its texture name, e.g. `bg room`.
    Image(String),
    Audio {
        channel: String,
        asset: String,
        looped: bool,
        queue: bool,
        fadein: Option<f64>,
    },
    /// Kept apart from [`Request::Audio`] so a late load can be dropped once the title has closed
    /// without also dropping an in-game play of the same track.
    TitleMusic(String),
    AudioMetadata,
    Font {
        dialogue: bool,
        path: String,
    },
}

impl Request {
    /// Paths relative to the assets directory, in lookup order.
    pub fn candidates(&self, index: &AssetIndex) -> Vec<String> {
        match self {
            Request::Image(name) => index.image_candidates(&name.split(' ').map(String::from).collect::<Vec<_>>()),
            Request::Audio { asset, .. } | Request::TitleMusic(asset) => index.audio_candidates(asset),
            Request::AudioMetadata => vec!["audio/audio.toml".into()],
            Request::Font { path, .. } => vec![path.clone()],
        }
    }
}

pub struct Loaded {
    pub request: Request,
    pub bytes: Option<Vec<u8>>,
    pub resolved_path: Option<String>,
}

pub struct Assets {
    source: Source,
    ready: std::rc::Rc<std::cell::RefCell<Vec<Loaded>>>,
}

enum Source {
    #[cfg(not(target_arch = "wasm32"))]
    Dir { root: std::path::PathBuf, index: AssetIndex },
    #[cfg(target_arch = "wasm32")]
    Url { base: String, index: AssetIndex },
}

impl Assets {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn dir(dir: std::path::PathBuf) -> Assets {
        let index = collect_paths(&dir).map(AssetIndex::new).unwrap_or_default();
        Assets { source: Source::Dir { root: dir, index }, ready: Default::default() }
    }

    /// Fetches `base` + the asset path, e.g. `kairos-asset://project/` + `images/bg room.png`.
    #[cfg(target_arch = "wasm32")]
    pub fn url(base: String, paths: Vec<String>) -> Assets {
        Assets { source: Source::Url { base, index: AssetIndex::new(paths) }, ready: Default::default() }
    }

    pub fn request(&mut self, request: Request) {
        match &self.source {
            #[cfg(not(target_arch = "wasm32"))]
            Source::Dir { root, index } => {
                let (resolved_path, bytes) = read_request(root, index, &request).unzip();
                self.ready.borrow_mut().push(Loaded { request, bytes, resolved_path });
            }
            #[cfg(target_arch = "wasm32")]
            Source::Url { base, index } => fetch_request(base.clone(), index.clone(), request, self.ready.clone()),
        }
    }

    pub fn poll(&mut self) -> Vec<Loaded> {
        std::mem::take(&mut *self.ready.borrow_mut())
    }
}

fn layered_paths(name: &str, index: &AssetIndex, text: &str) -> Result<Vec<String>, toml::de::Error> {
    let config: LayerConfig = toml::from_str(text)?;
    let mut words = name.split(' ');
    let tag = words.next().unwrap_or_default();
    Ok(config.files(tag, &words.map(String::from).collect::<Vec<_>>(), index))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_request(root: &std::path::Path, index: &AssetIndex, request: &Request) -> Option<(String, Vec<u8>)> {
    if let Request::Image(name) = request {
        let mut words = name.split(' ');
        let tag = words.next().unwrap_or_default();
        let manifest = index.layer_manifest(tag);
        if index.contains(&manifest) {
            let text = std::fs::read_to_string(root.join(&manifest)).ok()?;
            let paths = layered_paths(name, index, &text).ok()?;
            let images = paths.iter().map(|path| std::fs::read(root.join(path)).ok()).collect::<Option<Vec<_>>>()?;
            return Some((manifest, compose_images(images)?));
        }
    }
    request
        .candidates(index)
        .into_iter()
        .find_map(|path| std::fs::read(root.join(&path)).ok().map(|bytes| (path, bytes)))
}

fn compose_images(images: Vec<Vec<u8>>) -> Option<Vec<u8>> {
    let mut images = images.into_iter();
    let mut composed = image::load_from_memory(&images.next()?).ok()?.into_rgba8();
    for bytes in images {
        let layer = image::load_from_memory(&bytes).ok()?.into_rgba8();
        image::imageops::overlay(&mut composed, &layer, 0, 0);
    }
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(composed).write_to(&mut bytes, image::ImageFormat::Png).ok()?;
    Some(bytes.into_inner())
}

/// Percent-encodes a relative path for a URL, keeping `/`.
pub fn encode_path(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(target_arch = "wasm32")]
fn fetch_request(
    base: String,
    index: AssetIndex,
    request: Request,
    ready: std::rc::Rc<std::cell::RefCell<Vec<Loaded>>>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let found = match &request {
            Request::Image(name) => fetch_image(&base, &index, name).await,
            _ => fetch_paths(&base, &request.candidates(&index)).await,
        };
        let (resolved_path, bytes) = found.unzip();
        ready.borrow_mut().push(Loaded { request, bytes, resolved_path });
    });
}

#[cfg(target_arch = "wasm32")]
async fn fetch_image(base: &str, index: &AssetIndex, name: &str) -> Option<(String, Vec<u8>)> {
    let tag = name.split(' ').next().unwrap_or_default();
    let manifest = index.layer_manifest(tag);
    if !index.contains(&manifest) {
        return fetch_paths(base, &index.image_candidates(&name.split(' ').map(String::from).collect::<Vec<_>>()))
            .await;
    }
    let (_, text) = fetch_paths(base, std::slice::from_ref(&manifest)).await?;
    let paths = layered_paths(name, index, &String::from_utf8(text).ok()?).ok()?;
    let mut layers = Vec::with_capacity(paths.len());
    for path in paths {
        layers.push(fetch_paths(base, &[path]).await?.1);
    }
    Some((manifest, compose_images(layers)?))
}

#[cfg(target_arch = "wasm32")]
async fn fetch_paths(base: &str, paths: &[String]) -> Option<(String, Vec<u8>)> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    for path in paths {
        let Some(window) = web_sys::window() else { break };
        let url = format!("{base}{}", encode_path(path));
        let Ok(response) = JsFuture::from(window.fetch_with_str(&url)).await else { continue };
        let Ok(response) = response.dyn_into::<web_sys::Response>() else { continue };
        if !response.ok() {
            continue;
        }
        let Ok(buffer) = response.array_buffer() else { continue };
        if let Ok(buffer) = JsFuture::from(buffer).await {
            return Some((path.clone(), js_sys::Uint8Array::new(&buffer).to_vec()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_requests_follow_the_shared_naming_rule() {
        let r = Request::Image("bg room".into());
        let index = AssetIndex::new(["images/bg room.png", "images/backgrounds/bg room.webp"]);
        assert_eq!(r.candidates(&index), ["images/bg room.png", "images/backgrounds/bg room.webp"]);
    }

    #[test]
    fn audio_requests_resolve_under_audio() {
        let r = Request::Audio {
            channel: "music".into(),
            asset: "theme.ogg".into(),
            looped: true,
            queue: false,
            fadein: None,
        };
        let index = AssetIndex::new(["assets/audio/theme.ogg"]);
        assert_eq!(r.candidates(&index), ["audio/theme.ogg"]);
        assert_eq!(Request::AudioMetadata.candidates(&index), ["audio/audio.toml"]);
        assert_eq!(Request::TitleMusic("theme".into()).candidates(&index), ["audio/theme.ogg"]);
    }

    #[test]
    fn urls_encode_spaces_but_keep_slashes() {
        assert_eq!(encode_path("images/bg room.png"), "images/bg%20room.png");
        assert_eq!(encode_path("audio/é.ogg"), "audio/%C3%A9.ogg");
    }

    #[test]
    fn directory_sources_use_recursive_indexes_for_images_and_audio() {
        let dir = std::env::temp_dir().join(format!("kairos-assets-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("images/backgrounds")).unwrap();
        std::fs::create_dir_all(dir.join("audio/music")).unwrap();
        std::fs::write(dir.join("images/backgrounds/bg room.jpg"), b"jpg").unwrap();
        std::fs::write(dir.join("audio/music/theme.ogg"), b"ogg").unwrap();
        let mut assets = Assets::dir(dir.clone());
        assets.request(Request::Image("bg room".into()));
        assets.request(Request::Audio {
            channel: "music".into(),
            asset: "theme".into(),
            looped: true,
            queue: false,
            fadein: None,
        });
        let loaded = assets.poll();
        assert_eq!(loaded[0].bytes.as_deref(), Some(&b"jpg"[..]));
        assert_eq!(loaded[0].resolved_path.as_deref(), Some("images/backgrounds/bg room.jpg"));
        assert_eq!(loaded[1].bytes.as_deref(), Some(&b"ogg"[..]));
        assert_eq!(loaded[1].resolved_path.as_deref(), Some("audio/music/theme.ogg"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn layered_images_are_composited_in_configured_order() {
        let dir = std::env::temp_dir().join(format!("kairos-layered-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("images/sprites/e/base")).unwrap();
        std::fs::create_dir_all(dir.join("images/sprites/e/face")).unwrap();
        std::fs::write(
            dir.join("images/sprites/e/layers.toml"),
            "order = ['base', 'face']\n[default]\nface = 'neutral'\n",
        )
        .unwrap();
        for (path, color) in [
            ("images/sprites/e/base/body.png", image::Rgba([255, 0, 0, 255])),
            ("images/sprites/e/face/happy.png", image::Rgba([0, 0, 255, 128])),
            ("images/sprites/e/face/neutral.png", image::Rgba([0, 255, 0, 255])),
        ] {
            let image = image::RgbaImage::from_pixel(1, 1, color);
            image.save(dir.join(path)).unwrap();
        }
        let mut assets = Assets::dir(dir.clone());
        assets.request(Request::Image("e happy".into()));
        let bytes = assets.poll().remove(0).bytes.unwrap();
        let composed = image::load_from_memory(&bytes).unwrap().into_rgba8();
        assert_eq!(composed.get_pixel(0, 0), &image::Rgba([127, 0, 128, 254]));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
