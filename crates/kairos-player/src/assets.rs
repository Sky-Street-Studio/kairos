//! Asset fetching. Native reads files right away; the web fetches in the background. Both report
//! results through [`Assets::poll`] so the player handles them the same way.

use kairos_project::assets::{audio_candidates, image_candidates};

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
    AudioMetadata,
    Font {
        dialogue: bool,
        path: String,
    },
}

impl Request {
    /// Paths relative to the assets directory, in lookup order.
    pub fn candidates(&self) -> Vec<String> {
        match self {
            Request::Image(name) => image_candidates(&name.split(' ').map(String::from).collect::<Vec<_>>()),
            Request::Audio { asset, .. } => audio_candidates(asset),
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
    Dir(std::path::PathBuf),
    #[cfg(target_arch = "wasm32")]
    Url(String),
}

impl Assets {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn dir(dir: std::path::PathBuf) -> Assets {
        Assets { source: Source::Dir(dir), ready: Default::default() }
    }

    /// Fetches `base` + the asset path, e.g. `kairos-asset://project/` + `images/bg room.png`.
    #[cfg(target_arch = "wasm32")]
    pub fn url(base: String) -> Assets {
        Assets { source: Source::Url(base), ready: Default::default() }
    }

    pub fn request(&mut self, request: Request) {
        let candidates = request.candidates();
        match &self.source {
            #[cfg(not(target_arch = "wasm32"))]
            Source::Dir(dir) => {
                let found =
                    candidates.iter().find_map(|c| std::fs::read(dir.join(c)).ok().map(|bytes| (c.clone(), bytes)));
                let (resolved_path, bytes) = found.map_or((None, None), |(path, bytes)| (Some(path), Some(bytes)));
                self.ready.borrow_mut().push(Loaded { request, bytes, resolved_path });
            }
            #[cfg(target_arch = "wasm32")]
            Source::Url(base) => {
                let urls = candidates.iter().map(|c| (format!("{base}{}", encode_path(c)), c.clone())).collect();
                fetch_first(urls, request, self.ready.clone());
            }
        }
    }

    pub fn poll(&mut self) -> Vec<Loaded> {
        std::mem::take(&mut *self.ready.borrow_mut())
    }
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
fn fetch_first(urls: Vec<(String, String)>, request: Request, ready: std::rc::Rc<std::cell::RefCell<Vec<Loaded>>>) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    wasm_bindgen_futures::spawn_local(async move {
        let mut found = None;
        for (url, path) in urls {
            let Some(window) = web_sys::window() else { break };
            let Ok(response) = JsFuture::from(window.fetch_with_str(&url)).await else { continue };
            let Ok(response) = response.dyn_into::<web_sys::Response>() else { continue };
            if !response.ok() {
                continue;
            }
            let Ok(buffer) = response.array_buffer() else { continue };
            if let Ok(buffer) = JsFuture::from(buffer).await {
                found = Some((path, js_sys::Uint8Array::new(&buffer).to_vec()));
                break;
            }
        }
        let (resolved_path, bytes) = found.map_or((None, None), |(path, bytes)| (Some(path), Some(bytes)));
        ready.borrow_mut().push(Loaded { request, bytes, resolved_path });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_requests_follow_the_shared_naming_rule() {
        let r = Request::Image("bg room".into());
        assert_eq!(r.candidates()[0], "images/bg room.png");
        assert_eq!(r.candidates().len(), 4);
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
        assert_eq!(r.candidates(), ["audio/theme.ogg"]);
        assert_eq!(Request::AudioMetadata.candidates(), ["audio/audio.toml"]);
    }

    #[test]
    fn urls_encode_spaces_but_keep_slashes() {
        assert_eq!(encode_path("images/bg room.png"), "images/bg%20room.png");
        assert_eq!(encode_path("audio/é.ogg"), "audio/%C3%A9.ogg");
    }

    #[test]
    fn a_directory_source_reports_hits_and_misses() {
        let dir = std::env::temp_dir().join(format!("kairos-assets-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("images")).unwrap();
        std::fs::write(dir.join("images/bg room.jpg"), b"jpg").unwrap();
        let mut assets = Assets::dir(dir.clone());
        assets.request(Request::Image("bg room".into()));
        assets.request(Request::Image("bg void".into()));
        let loaded = assets.poll();
        assert_eq!(loaded[0].bytes.as_deref(), Some(&b"jpg"[..]));
        assert_eq!(loaded[0].resolved_path.as_deref(), Some("images/bg room.jpg"));
        assert_eq!(loaded[1].bytes, None);
        assert_eq!(loaded[1].resolved_path, None);
        assert!(assets.poll().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
