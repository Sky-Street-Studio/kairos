//! The browser build: `WebPlayer` drives the player on a canvas for the editor's preview pane.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use kairos_project::{PlayerProjectJson, Project};
use kairos_render::Renderer;
use kairos_vm::Value;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MouseEvent, WheelEvent};

use crate::assets::Assets;
use crate::gpu::Gpu;
use crate::player::{Button, Key, Player};
use crate::saves::SaveStore;
use crate::theme::Theme;

struct Loaded {
    project: Project,
    theme: Theme,
    title: Option<String>,
    asset_base_url: String,
    assets: Vec<String>,
}

type Handler = Box<dyn Fn(&mut Inner, web_sys::Event)>;
type Listener = (&'static str, Closure<dyn FnMut(web_sys::Event)>);

#[derive(Serialize)]
struct DebugLocation {
    label: String,
    path: String,
    line: usize,
}

#[derive(Serialize)]
struct DebugState {
    label: String,
    path: String,
    line: usize,
    variables: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(rename = "callStack")]
    call_stack: Vec<DebugLocation>,
    history: usize,
    position: usize,
}

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
    debug_callback: Option<js_sys::Function>,
    last_debug_state: Option<String>,
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
            debug_callback: None,
            last_debug_state: None,
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
    pub fn load(&self, project_json: &str, on_debug: Option<js_sys::Function>) -> Result<(), JsError> {
        let json: PlayerProjectJson =
            serde_json::from_str(project_json).map_err(|e| error(format!("project json: {e}")))?;
        let sources = json.scripts.into_iter().map(|s| (PathBuf::from(s.path), s.source));
        let mut project = Project::from_sources(&json.manifest, sources).map_err(|e| error(e.to_string()))?;
        for screen in json.screens {
            let path = PathBuf::from(screen.path);
            match kairos_project::screens::parse_screen(&screen.source) {
                Ok(definition) => {
                    let expected = path.file_stem().and_then(|name| name.to_str()).unwrap_or_default();
                    if expected != definition.id {
                        project.screen_diagnostics.push((
                            path.clone(),
                            kairos_project::screens::ScreenDiagnostic {
                                node: None,
                                message: format!("screen id `{}` does not match filename `{expected}`", definition.id),
                            },
                        ));
                    } else if project.screens.insert(definition.id.clone(), definition).is_some() {
                        project.screen_diagnostics.push((
                            path.clone(),
                            kairos_project::screens::ScreenDiagnostic {
                                node: None,
                                message: "duplicate screen id".into(),
                            },
                        ));
                    }
                }
                Err(diagnostics) => project
                    .screen_diagnostics
                    .extend(diagnostics.into_iter().map(|diagnostic| (path.clone(), diagnostic))),
            }
        }
        project.validate_screens();
        project.include_screen_diagnostics();
        for translation in json.translations {
            project.set_translation(&translation.lang, &translation.source).map_err(|e| error(e.to_string()))?;
        }
        if !project.diagnostics.is_empty() || !project.screen_diagnostics.is_empty() {
            return Err(error(project.diagnostics_report().trim_end()));
        }
        let theme = Theme::parse(json.theme.as_deref().unwrap_or_default()).map_err(error)?;
        let mut inner = self.inner.borrow_mut();
        inner.debug_callback = on_debug;
        inner.last_debug_state = None;
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
        inner.last_debug_state = None;
        Ok(())
    }

    /// Plays from the first statement at or after `line` (1-based) in the script at `path`.
    #[wasm_bindgen(js_name = playFromLine)]
    pub fn play_from_line(&self, path: &str, line: u32, overrides_json: Option<String>) -> Result<(), JsError> {
        let mut inner = self.inner.borrow_mut();
        let pc = {
            let loaded = inner.loaded.as_ref().ok_or_else(|| error("call load() before playFromLine()"))?;
            loaded
                .project
                .pc_at(Path::new(path), line as usize)
                .ok_or_else(|| error(format!("{path}:{line}: no statement at or after this line")))?
        };
        let overrides = overrides_json
            .map(|json| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&json))
            .transpose()
            .map_err(|e| error(format!("overrides json: {e}")))?
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| Ok((name.clone(), from_json(value, &name)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, JsError>>()?;
        let player = inner.player.as_mut().ok_or_else(|| error("call ready() before playFromLine()"))?;
        player.debug_start_at(pc, &overrides).map_err(|e| error(e.to_string()))?;
        inner.last_debug_state = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = previewAt)]
    pub fn preview_at(&self, path: &str, line: u32) -> Result<(), JsError> {
        let mut inner = self.inner.borrow_mut();
        let pc = {
            let loaded = inner.loaded.as_ref().ok_or_else(|| error("call load() before previewAt()"))?;
            loaded
                .project
                .pc_at(Path::new(path), line as usize)
                .ok_or_else(|| error(format!("{path}:{line}: no statement here")))?
        };
        let player = inner.player.as_mut().ok_or_else(|| error("call ready() before previewAt()"))?;
        player.preview_at(pc);
        inner.last_debug_state = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = spriteAt)]
    pub fn sprite_at(&self, x: f32, y: f32) -> Option<String> {
        self.inner.borrow().player.as_ref().and_then(|p| p.sprite_at(x, y))
    }

    #[wasm_bindgen(js_name = debugState)]
    pub fn debug_state(&self) -> Result<String, JsError> {
        debug_state(&self.inner.borrow())
    }

    #[wasm_bindgen(js_name = setVariable)]
    pub fn set_variable(&self, name: &str, json: &str) -> Result<(), JsError> {
        let value =
            from_json(serde_json::from_str(json).map_err(|e| error(format!("variable `{name}` json: {e}")))?, name)?;
        let mut inner = self.inner.borrow_mut();
        inner
            .player
            .as_mut()
            .ok_or_else(|| error("call ready() and load() before setVariable()"))?
            .debug_set_variable(name, value)
            .map_err(|e| error(e.to_string()))?;
        inner.last_debug_state = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = rollbackTo)]
    pub fn rollback_to(&self, position: u32) -> Result<(), JsError> {
        let mut inner = self.inner.borrow_mut();
        let player = inner.player.as_mut().ok_or_else(|| error("call ready() and load() before rollbackTo()"))?;
        if !player.debug_rollback_to(position as usize) {
            return Err(error(format!("history position {position} is not earlier than the current position")));
        }
        inner.last_debug_state = None;
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

fn from_json(value: serde_json::Value, name: &str) -> Result<Value, JsError> {
    match value {
        serde_json::Value::Null => Ok(Value::None),
        serde_json::Value::Bool(value) => Ok(Value::Bool(value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(Value::Int)
            .or_else(|| value.as_f64().map(Value::Float))
            .ok_or_else(|| error(format!("variable `{name}` number is out of range"))),
        serde_json::Value::String(value) => Ok(Value::Str(value)),
        serde_json::Value::Array(items) => {
            Ok(Value::List(items.into_iter().map(|item| from_json(item, name)).collect::<Result<_, _>>()?))
        }
        serde_json::Value::Object(map) => {
            Ok(Value::Dict(map.into_iter().map(|(k, v)| Ok((k, from_json(v, name)?))).collect::<Result<_, JsError>>()?))
        }
    }
}

fn to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::None => serde_json::Value::Null,
        Value::Bool(value) => (*value).into(),
        Value::Int(value) => (*value).into(),
        Value::Float(value) => {
            serde_json::Number::from_f64(*value).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null)
        }
        Value::Str(value) => value.clone().into(),
        Value::List(items) => items.iter().map(to_json).collect(),
        Value::Dict(map) => map.iter().map(|(k, v)| (k.clone(), to_json(v))).collect(),
    }
}

fn debug_state(inner: &Inner) -> Result<String, JsError> {
    let loaded = inner.loaded.as_ref().ok_or_else(|| error("call load() before debugState()"))?;
    let player = inner.player.as_ref().ok_or_else(|| error("call ready() before debugState()"))?;
    let program = &loaded.project.program;
    let location = |pc: usize| {
        let file = program.files.get(pc).and_then(|index| loaded.project.files.get(*index));
        DebugLocation {
            label: program.label_at(pc).unwrap_or_default().to_string(),
            path: file.map(|path| path.to_string_lossy().into_owned()).unwrap_or_default(),
            line: program.lines.get(pc).copied().unwrap_or_default(),
        }
    };
    let current = location(player.debug_pc());
    let mut call_stack: Vec<_> = player.debug_call_stack().iter().rev().map(|pc| location(*pc)).collect();
    call_stack.push(DebugLocation { label: current.label.clone(), path: current.path.clone(), line: current.line });
    let history = player.debug_history_len();
    serde_json::to_string(&DebugState {
        label: current.label,
        path: current.path,
        line: current.line,
        variables: player.debug_variables().iter().map(|(name, value)| (name.clone(), to_json(value))).collect(),
        call_stack,
        history,
        position: history.saturating_sub(1),
    })
    .map_err(|e| error(format!("debug state: {e}")))
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
            let callback = {
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
                let state = debug_state(&this).ok();
                match (state, this.debug_callback.clone()) {
                    (Some(state), Some(callback)) if this.last_debug_state.as_ref() != Some(&state) => {
                        this.last_debug_state = Some(state.clone());
                        Some((callback, state))
                    }
                    _ => None,
                }
            };
            if let Some((callback, state)) = callback {
                let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&state));
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
                "Tab" => Some(if event.shift_key() { Key::FocusPrevious } else { Key::FocusNext }),
                "ArrowUp" | "ArrowLeft" => Some(Key::FocusPrevious),
                "ArrowDown" | "ArrowRight" => Some(Key::FocusNext),
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
