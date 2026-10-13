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
    FallbackFont(String),
}

impl Request {
    /// Paths relative to the assets directory, in lookup order.
    pub fn candidates(&self, index: &AssetIndex) -> Vec<String> {
        match self {
            Request::Image(name) => index.image_candidates(&name.split(' ').map(String::from).collect::<Vec<_>>()),
            Request::Audio { asset, .. } | Request::TitleMusic(asset) => index.audio_candidates(asset),
            Request::AudioMetadata => vec!["audio/audio.toml".into()],
            Request::Font { path, .. } => vec![path.clone()],
            Request::FallbackFont(path) => vec![path.clone()],
        }
    }
}

pub struct Loaded {
    pub request: Request,
    pub bytes: Option<Vec<u8>>,
    pub resolved_path: Option<String>,
    pub decoded_image: Option<Result<image::RgbaImage, String>>,
}

pub struct Assets {
    #[cfg(not(target_arch = "wasm32"))]
    sender: Option<std::sync::mpsc::Sender<Request>>,
    #[cfg(not(target_arch = "wasm32"))]
    ready: std::sync::mpsc::Receiver<Loaded>,
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<std::thread::JoinHandle<()>>,
    #[cfg(target_arch = "wasm32")]
    source: Source,
    #[cfg(target_arch = "wasm32")]
    ready: std::rc::Rc<std::cell::RefCell<Vec<Loaded>>>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct PackCache {
    bytes: Option<Vec<u8>>,
    pending: Option<js_sys::Promise>,
}

enum Source {
    #[cfg(not(target_arch = "wasm32"))]
    Dir { root: std::path::PathBuf, index: AssetIndex },
    #[cfg(not(target_arch = "wasm32"))]
    Pack { index: kairos_project::assets::PackIndex, assets: AssetIndex },
    #[cfg(target_arch = "wasm32")]
    Url {
        base: String,
        index: AssetIndex,
        pack: Option<(String, Option<String>, std::rc::Rc<std::cell::RefCell<PackCache>>)>,
    },
}

impl Assets {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn dir(dir: std::path::PathBuf, key: Option<&str>, max_texture_size: u32) -> Result<Assets, String> {
        let pack_path = dir.parent().unwrap_or(&dir).join(kairos_project::assets::PACK_FILE);
        let source = if pack_path.is_file() {
            let index = kairos_project::assets::PackIndex::open(&pack_path, key)
                .map_err(|error| format!("{}: {error}", pack_path.display()))?;
            let assets = AssetIndex::new(index.paths());
            Source::Pack { index, assets }
        } else {
            let index = collect_paths(&dir).map(AssetIndex::new).unwrap_or_default();
            Source::Dir { root: dir, index }
        };
        let (sender, requests) = std::sync::mpsc::channel();
        let (results, ready) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let (resolved_path, bytes) = match &source {
                    Source::Dir { root, index } => read_request(root, index, &request),
                    Source::Pack { index, assets } => read_pack_request(index, assets, &request),
                }
                .unzip();
                let decoded_image = match (&request, bytes.as_deref()) {
                    (Request::Image(_), Some(bytes)) => {
                        Some(decode_image(bytes, max_texture_size).map_err(|e| e.to_string()))
                    }
                    _ => None,
                };
                let bytes = if matches!(&request, Request::Image(_)) { None } else { bytes };
                if results.send(Loaded { request, bytes, resolved_path, decoded_image }).is_err() {
                    break;
                }
            }
        });
        Ok(Assets { sender: Some(sender), ready, worker: Some(worker) })
    }

    /// Fetches `base` + the asset path, e.g. `kairos-asset://project/` + `images/bg room.png`.
    #[cfg(target_arch = "wasm32")]
    pub fn url(base: String, paths: Vec<String>, pack: Option<String>, key: Option<String>) -> Assets {
        let pack = pack.map(|path| (path, key, Default::default()));
        Assets { source: Source::Url { base, index: AssetIndex::new(paths), pack }, ready: Default::default() }
    }

    pub fn request(&mut self, request: Request) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(sender) = &self.sender {
            let _ = sender.send(request);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let Source::Url { base, index, pack } = &self.source;
            fetch_request(base.clone(), index.clone(), pack.clone(), request, self.ready.clone());
        }
    }

    pub fn poll(&mut self) -> Vec<Loaded> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.ready.try_iter().collect();
        #[cfg(target_arch = "wasm32")]
        std::mem::take(&mut *self.ready.borrow_mut())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn decode_image(bytes: &[u8], max: u32) -> Result<image::RgbaImage, image::ImageError> {
    let image = image::load_from_memory(bytes)?;
    let image = if image.width() > max || image.height() > max {
        image.resize(max, max, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    Ok(image.into_rgba8())
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Assets {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
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

#[cfg(not(target_arch = "wasm32"))]
fn read_pack_request(
    index: &kairos_project::assets::PackIndex,
    assets: &AssetIndex,
    request: &Request,
) -> Option<(String, Vec<u8>)> {
    let read = |path: &str| index.read(path).ok().flatten();
    if let Request::Image(name) = request {
        let tag = name.split(' ').next().unwrap_or_default();
        let manifest = assets.layer_manifest(tag);
        if assets.contains(&manifest) {
            let text = String::from_utf8(read(&manifest)?).ok()?;
            let paths = layered_paths(name, assets, &text).ok()?;
            let images = paths.iter().map(|path| read(path)).collect::<Option<Vec<_>>>()?;
            return Some((manifest, compose_images(images)?));
        }
    }
    request.candidates(assets).into_iter().find_map(|path| read(&path).map(|bytes| (path, bytes)))
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
    pack: Option<(String, Option<String>, std::rc::Rc<std::cell::RefCell<PackCache>>)>,
    request: Request,
    ready: std::rc::Rc<std::cell::RefCell<Vec<Loaded>>>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let found = match &request {
            Request::Image(name) => fetch_image(&base, &index, name, pack.as_ref()).await,
            _ => fetch_paths(&base, &request.candidates(&index), pack.as_ref()).await,
        };
        let (resolved_path, bytes) = found.unzip();
        ready.borrow_mut().push(Loaded { request, bytes, resolved_path, decoded_image: None });
    });
}

#[cfg(target_arch = "wasm32")]
async fn fetch_image(
    base: &str,
    index: &AssetIndex,
    name: &str,
    pack: Option<&(String, Option<String>, std::rc::Rc<std::cell::RefCell<PackCache>>)>,
) -> Option<(String, Vec<u8>)> {
    let tag = name.split(' ').next().unwrap_or_default();
    let manifest = index.layer_manifest(tag);
    if !index.contains(&manifest) {
        return fetch_paths(
            base,
            &index.image_candidates(&name.split(' ').map(String::from).collect::<Vec<_>>()),
            pack,
        )
        .await;
    }
    let (_, text) = fetch_paths(base, std::slice::from_ref(&manifest), pack).await?;
    let paths = layered_paths(name, index, &String::from_utf8(text).ok()?).ok()?;
    let mut layers = Vec::with_capacity(paths.len());
    for path in paths {
        layers.push(fetch_paths(base, &[path], pack).await?.1);
    }
    Some((manifest, compose_images(layers)?))
}

#[cfg(target_arch = "wasm32")]
async fn fetch_paths(
    base: &str,
    paths: &[String],
    pack: Option<&(String, Option<String>, std::rc::Rc<std::cell::RefCell<PackCache>>)>,
) -> Option<(String, Vec<u8>)> {
    if let Some((pack_path, key, cache)) = pack {
        let bytes = fetch_pack_singleflight(base, pack_path, cache).await?;
        for path in paths {
            if let Ok(Some(bytes)) = kairos_project::assets::pack_entry(&bytes, path, key.as_deref()) {
                return Some((path.clone(), bytes));
            }
        }
        return None;
    }
    for path in paths {
        if let Some(bytes) = fetch_bytes(&format!("{base}{}", encode_path(path))).await {
            return Some((path.clone(), bytes));
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
async fn fetch_pack_singleflight(
    base: &str,
    path: &str,
    cache: &std::rc::Rc<std::cell::RefCell<PackCache>>,
) -> Option<Vec<u8>> {
    use wasm_bindgen_futures::JsFuture;

    if let Some(bytes) = cache.borrow().bytes.clone() {
        return Some(bytes);
    }
    let pending = if let Some(pending) = cache.borrow().pending.clone() {
        pending
    } else {
        let url = format!("{base}{}", encode_path(path));
        let pending = wasm_bindgen_futures::future_to_promise(async move {
            let bytes = fetch_bytes(&url).await.ok_or(wasm_bindgen::JsValue::NULL)?;
            Ok(wasm_bindgen::JsValue::from(js_sys::Uint8Array::from(bytes.as_slice())))
        });
        cache.borrow_mut().pending = Some(pending.clone());
        pending
    };
    let result = JsFuture::from(pending).await.ok().map(|value| js_sys::Uint8Array::new(&value).to_vec());
    let mut cache = cache.borrow_mut();
    cache.pending = None;
    if let Some(bytes) = &result {
        cache.bytes = Some(bytes.clone());
    }
    result
}

#[cfg(target_arch = "wasm32")]
async fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let Some(window) = web_sys::window() else { return None };
    let response = JsFuture::from(window.fetch_with_str(url)).await.ok()?.dyn_into::<web_sys::Response>().ok()?;
    if !response.ok() {
        return None;
    }
    let buffer = JsFuture::from(response.array_buffer().ok()?).await.ok()?;
    Some(js_sys::Uint8Array::new(&buffer).to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn poll_until(assets: &mut Assets, count: usize) -> Vec<Loaded> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let loaded = assets.poll();
            if loaded.len() >= count {
                return loaded;
            }
            assert!(Instant::now() < deadline, "asset worker did not complete");
            std::thread::yield_now();
        }
    }

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
        std::fs::write(dir.join("images/backgrounds/bg room.jpg"), b"invalid image").unwrap();
        std::fs::write(dir.join("audio/music/theme.ogg"), b"ogg").unwrap();
        let mut assets = Assets::dir(dir.clone(), None, 4096).unwrap();
        assets.request(Request::Image("bg room".into()));
        assets.request(Request::Audio {
            channel: "music".into(),
            asset: "theme".into(),
            looped: true,
            queue: false,
            fadein: None,
        });
        let loaded = poll_until(&mut assets, 2);
        assert!(loaded[0].decoded_image.as_ref().unwrap().is_err());
        assert_eq!(loaded[0].resolved_path.as_deref(), Some("images/backgrounds/bg room.jpg"));
        assert_eq!(loaded[1].bytes.as_deref(), Some(&b"ogg"[..]));
        assert_eq!(loaded[1].resolved_path.as_deref(), Some("audio/music/theme.ogg"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_requests_complete_through_nonblocking_poll() {
        let dir = std::env::temp_dir().join(format!("kairos-poll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = vec![42; 8 * 1024 * 1024];
        std::fs::write(dir.join("font.ttf"), &bytes).unwrap();
        let mut assets = Assets::dir(dir.clone(), None, 4096).unwrap();
        assets.request(Request::Font { dialogue: false, path: "font.ttf".into() });
        let started = Instant::now();
        let _ = assets.poll();
        assert!(started.elapsed() < Duration::from_millis(100));
        let loaded = poll_until(&mut assets, 1).remove(0);
        assert_eq!(loaded.bytes.as_deref(), Some(bytes.as_slice()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_archive_returns_an_error_instead_of_using_loose_assets() {
        let root = std::env::temp_dir().join(format!("kairos-corrupt-assets-{}", std::process::id()));
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/images.png"), b"loose fallback").unwrap();
        std::fs::write(root.join(kairos_project::assets::PACK_FILE), b"not a pack").unwrap();
        let result = Assets::dir(root.join("assets"), None, 4096);
        assert!(result.err().unwrap().contains("corrupt kpak archive"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packed_sources_resolve_assets_without_loose_files() {
        let root = std::env::temp_dir().join(format!("kairos-packed-assets-{}", std::process::id()));
        let assets_dir = root.join("assets");
        std::fs::create_dir_all(assets_dir.join("images")).unwrap();
        std::fs::write(assets_dir.join("images/bg room.png"), b"packed image").unwrap();
        kairos_project::assets::pack(&assets_dir, &root.join(kairos_project::assets::PACK_FILE), Some("paper-key"))
            .unwrap();
        std::fs::remove_dir_all(&assets_dir).unwrap();
        let mut assets = Assets::dir(assets_dir, Some("paper-key"), 4096).unwrap();
        assets.request(Request::Image("bg room".into()));
        let loaded = poll_until(&mut assets, 1).remove(0);
        assert_eq!(loaded.resolved_path.as_deref(), Some("images/bg room.png"));
        assert!(loaded.decoded_image.unwrap().is_err());
        std::fs::remove_dir_all(root).unwrap();
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
        let mut assets = Assets::dir(dir.clone(), None, 4096).unwrap();
        assets.request(Request::Image("e happy".into()));
        let composed = poll_until(&mut assets, 1).remove(0).decoded_image.unwrap().unwrap();
        assert_eq!(composed.get_pixel(0, 0), &image::Rgba([127, 0, 128, 254]));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
