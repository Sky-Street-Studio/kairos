//! The browser build: `WebPlayer` drives the player on a canvas for the editor's preview pane.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use kairos_project::Project;
use kairos_render::Renderer;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MouseEvent, WheelEvent};

use crate::assets::Assets;
use crate::gpu::Gpu;
use crate::player::{Button, Key, Player};
use crate::saves::SaveStore;
use crate::theme::Theme;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectJson {
    manifest: String,
    scripts: Vec<ScriptJson>,
    #[serde(default)]
    translations: Vec<TranslationJson>,
    asset_base_url: String,
    #[serde(default)]
    assets: Vec<String>,
    /// Contents of `ui/theme.toml`, if the project has one.
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
struct ScriptJson {
    path: String,
    source: String,
}

#[derive(Deserialize)]
struct TranslationJson {
    lang: String,
    source: String,
}

struct Loaded {
    project: Project,
    theme: Theme,
    title: Option<String>,
    asset_base_url: String,
    assets: Vec<String>,
}

type Handler = Box<dyn Fn(&mut Inner, web_sys::Event)>;
type Listener = (&'static str, Closure<dyn FnMut(web_sys::Event)>);

struct Inner {
    canvas: HtmlCanvasElement,
    gpu: Option<Gpu>,
    loaded: Option<Loaded>,
    player: Option<Player>,
    listeners: Vec<Listener>,
    frame_request: Option<i32>,
    frame_callback: Option<Closure<dyn FnMut(f64)>>,
    last_time: Option<f64>,
    wheel: f64,
    destroyed: bool,
}

#[wasm_bindgen]
pub struct WebPlayer {
    inner: Rc<RefCell<Inner>>,
}

fn error(message: impl AsRef<str>) -> JsError {
    JsError::new(message.as_ref())
}

#[wasm_bindgen]
impl WebPlayer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> WebPlayer {
        console_error_panic_hook::set_once();
        canvas.set_tab_index(0);
        let inner = Rc::new(RefCell::new(Inner {
            canvas,
            gpu: None,
            loaded: None,
            player: None,
            listeners: Vec::new(),
            frame_request: None,
            frame_callback: None,
            last_time: None,
            wheel: 0.0,
            destroyed: false,
        }));
        listen(&inner);
        WebPlayer { inner }
    }

    /// Initialises WebGPU, or WebGL2 where WebGPU is unavailable.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn ready(&self) -> js_sys::Promise {
        let inner = self.inner.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let canvas = inner.borrow().canvas.clone();
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
            let instance = wgpu::util::new_instance_with_webgpu_detection(desc).await;
            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                .map_err(|e| JsValue::from(error(e.to_string())))?;
            let gpu = Gpu::new(&instance, surface, canvas.width(), canvas.height())
                .await
                .map_err(|e| JsValue::from(error(e)))?;
            let mut this = inner.borrow_mut();
            if this.destroyed {
                return Ok(JsValue::UNDEFINED);
            }
            this.gpu = Some(gpu);
            this.build_player().map_err(|e| JsValue::from(error(e)))?;
            drop(this);
            schedule_frame(&inner);
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Compiles a project. Throws an `Error` with one `path:line: message` line per diagnostic.
    pub fn load(&self, project_json: &str) -> Result<(), JsError> {
        let json: ProjectJson = serde_json::from_str(project_json).map_err(|e| error(format!("project json: {e}")))?;
        let sources = json.scripts.into_iter().map(|s| (PathBuf::from(s.path), s.source));
        let mut project = Project::from_sources(&json.manifest, sources).map_err(|e| error(e.to_string()))?;
        for translation in json.translations {
            project.set_translation(&translation.lang, &translation.source).map_err(|e| error(e.to_string()))?;
        }
        if !project.diagnostics.is_empty() {
            return Err(error(project.diagnostics_report().trim_end()));
        }
        let theme = Theme::parse(json.theme.as_deref().unwrap_or_default()).map_err(error)?;
        let mut inner = self.inner.borrow_mut();
        inner.loaded = Some(Loaded {
            project,
            theme,
            title: json.title,
            asset_base_url: json.asset_base_url,
            assets: json.assets,
        });
        inner.build_player().map_err(error)
    }

    /// Plays from the manifest's start label.
    pub fn start(&self) -> Result<(), JsError> {
        let mut inner = self.inner.borrow_mut();
        let player = inner.player.as_mut().ok_or_else(|| error("call ready() and load() before start()"))?;
        player.start();
        Ok(())
    }

    /// Plays from the first statement at or after `line` (1-based) in the script at `path`.
    #[wasm_bindgen(js_name = playFromLine)]
    pub fn play_from_line(&self, path: &str, line: u32) -> Result<(), JsError> {
        let mut inner = self.inner.borrow_mut();
        let pc = {
            let loaded = inner.loaded.as_ref().ok_or_else(|| error("call load() before playFromLine()"))?;
            loaded
                .project
                .pc_at(Path::new(path), line as usize)
                .ok_or_else(|| error(format!("{path}:{line}: no statement at or after this line")))?
        };
        let player = inner.player.as_mut().ok_or_else(|| error("call ready() before playFromLine()"))?;
        player.start_at(pc);
        Ok(())
    }

    /// Sets the canvas size in CSS pixels; the drawing buffer follows `devicePixelRatio`.
    pub fn resize(&self, width: u32, height: u32) {
        let mut inner = self.inner.borrow_mut();
        let ratio = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0);
        let (w, h) = (((width as f64) * ratio).round() as u32, ((height as f64) * ratio).round() as u32);
        inner.canvas.set_width(w.max(1));
        inner.canvas.set_height(h.max(1));
        if let Some(gpu) = &mut inner.gpu {
            gpu.resize(w, h);
        }
        if let Some(player) = &mut inner.player {
            player.resize(w.max(1), h.max(1));
        }
    }

    /// Stops rendering and audio and detaches every canvas listener.
    pub fn destroy(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.destroyed = true;
        if let (Some(id), Some(window)) = (inner.frame_request.take(), web_sys::window()) {
            let _ = window.cancel_animation_frame(id);
        }
        inner.frame_callback = None;
        for (name, listener) in std::mem::take(&mut inner.listeners) {
            let _ = inner.canvas.remove_event_listener_with_callback(name, listener.as_ref().unchecked_ref());
        }
        inner.player = None;
        inner.gpu = None;
    }
}

impl Inner {
    fn build_player(&mut self) -> Result<(), String> {
        let (Some(gpu), Some(loaded)) = (&self.gpu, &self.loaded) else { return Ok(()) };
        let window = loaded.project.manifest.window;
        let renderer = Renderer::new(
            gpu.device.clone(),
            gpu.queue.clone(),
            gpu.view_format,
            (window.width, window.height),
            gpu.size(),
        );
        let assets = Assets::url(loaded.asset_base_url.clone(), loaded.assets.clone());
        let saves = SaveStore::for_project(&loaded.project.manifest.name);
        let mut player = Player::new(renderer, &loaded.project, loaded.theme.clone(), assets, saves, false)?;
        if let Some(title) = &loaded.title {
            player.set_title(title)?;
        }
        self.player = Some(player);
        Ok(())
    }

    fn to_canvas(&self, event: &MouseEvent) -> (f32, f32) {
        let client = (self.canvas.client_width().max(1) as f64, self.canvas.client_height().max(1) as f64);
        let sx = self.canvas.width() as f64 / client.0;
        let sy = self.canvas.height() as f64 / client.1;
        ((event.offset_x() as f64 * sx) as f32, (event.offset_y() as f64 * sy) as f32)
    }
}

fn schedule_frame(inner: &Rc<RefCell<Inner>>) {
    let Some(window) = web_sys::window() else { return };
    let mut this = inner.borrow_mut();
    if this.destroyed {
        return;
    }
    if this.frame_callback.is_none() {
        let weak = Rc::downgrade(inner);
        this.frame_callback = Some(Closure::new(move |time: f64| {
            let Some(inner) = weak.upgrade() else { return };
            {
                let mut this = inner.borrow_mut();
                let Inner { gpu, player, last_time, destroyed, .. } = &mut *this;
                if *destroyed {
                    return;
                }
                let dt = last_time.map(|last| ((time - last) / 1000.0) as f32).unwrap_or(0.0);
                *last_time = Some(time);
                if let (Some(gpu), Some(player)) = (gpu, player) {
                    gpu.draw(|view| player.frame(dt, view));
                }
            }
            schedule_frame(&inner);
        }));
    }
    let callback = this.frame_callback.as_ref().expect("set above");
    this.frame_request = window.request_animation_frame(callback.as_ref().unchecked_ref()).ok();
}

fn listen(inner: &Rc<RefCell<Inner>>) {
    let canvas = inner.borrow().canvas.clone();
    let add = |name: &'static str, handler: Handler| {
        let weak = Rc::downgrade(inner);
        let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            if let Some(inner) = weak.upgrade() {
                handler(&mut inner.borrow_mut(), event);
            }
        });
        let _ = canvas.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
        inner.borrow_mut().listeners.push((name, closure));
    };

    add(
        "pointermove",
        Box::new(|inner, event| {
            let Some(event) = event.dyn_ref::<MouseEvent>() else { return };
            let (x, y) = inner.to_canvas(event);
            if let Some(player) = &mut inner.player {
                player.pointer_moved(x, y);
            }
        }),
    );
    add(
        "pointerdown",
        Box::new(|inner, event| {
            let Some(event) = event.dyn_ref::<MouseEvent>() else { return };
            let _ = inner.canvas.focus();
            let (x, y) = inner.to_canvas(event);
            let button = if event.button() == 2 { Button::Secondary } else { Button::Primary };
            if let Some(player) = &mut inner.player {
                player.pointer_moved(x, y);
                player.pointer_down(button);
            }
        }),
    );
    add("contextmenu", Box::new(|_, event| event.prevent_default()));
    add(
        "wheel",
        Box::new(|inner, event| {
            let Some(event) = event.dyn_ref::<WheelEvent>() else { return };
            event.prevent_default();
            let step = if event.delta_mode() == WheelEvent::DOM_DELTA_PIXEL { 60.0 } else { 1.0 };
            inner.wheel += event.delta_y() / step;
            while inner.wheel.abs() >= 1.0 {
                let direction = inner.wheel.signum();
                inner.wheel -= direction;
                if let Some(player) = &mut inner.player {
                    player.wheel(-direction as f32);
                }
            }
        }),
    );
    add(
        "keydown",
        Box::new(|inner, event| {
            let Some(event) = event.dyn_ref::<KeyboardEvent>() else { return };
            let key = match event.key().as_str() {
                " " | "Enter" => Some(Key::Advance),
                "Escape" => Some(Key::Escape),
                "PageUp" => Some(Key::Back),
                "PageDown" => Some(Key::Forward),
                "Tab" => Some(Key::Skip),
                "a" | "A" => Some(Key::Auto),
                k => k.parse::<u8>().ok().map(Key::Digit),
            };
            if let (Some(key), Some(player)) = (key, &mut inner.player) {
                event.prevent_default();
                player.key(key);
            }
        }),
    );
}
