use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use kairos_project::Project;
use kairos_render::Renderer;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Window, WindowId};

use crate::assets::Assets;
use crate::gpu::Gpu;
use crate::player::{Button, Key, Player};
use crate::saves::SaveStore;
use crate::theme::Theme;

/// Opens a window and plays the project until the window closes.
pub fn run(project: Project) -> Result<(), Box<dyn std::error::Error>> {
    let theme = load_theme(&project.root)?;
    let event_loop = EventLoop::new()?;
    #[cfg(feature = "steam")]
    let steam = crate::steam::Steam::new(project.manifest.steam.as_ref(), &project.manifest.name);
    let mut app = App {
        project,
        theme,
        running: None,
        error: None,
        #[cfg(feature = "steam")]
        steam,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e.into()),
        None => Ok(()),
    }
}

/// Reads `ui/theme.toml`, or the default theme when the file does not exist.
pub fn run_bundled() -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let root = bundled_game_dir(&executable)?;
    run(Project::load(&root)?)
}

fn bundled_game_dir(executable: &Path) -> Result<PathBuf, String> {
    let directory = executable.parent().ok_or("player executable has no parent directory")?;
    let sibling = directory.join("game");
    let resources = directory.join("../Resources/game");
    if sibling.is_dir() {
        Ok(sibling)
    } else if resources.is_dir() {
        Ok(resources)
    } else {
        Err(format!("could not find bundled game/ beside {}", executable.display()))
    }
}

pub fn load_theme(root: &Path) -> Result<Theme, String> {
    match std::fs::read_to_string(root.join("ui/theme.toml")) {
        Ok(text) => Theme::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Theme::default()),
        Err(e) => Err(format!("ui/theme.toml: {e}")),
    }
}

struct App {
    project: Project,
    theme: Theme,
    running: Option<Running>,
    error: Option<String>,
    #[cfg(feature = "steam")]
    steam: Option<crate::steam::Steam>,
}

struct Running {
    player: Player,
    gpu: Gpu,
    last: Instant,
    window: Arc<Window>,
    shift: bool,
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<Running, String> {
        let size = self.project.manifest.window;
        let attributes = Window::default_attributes()
            .with_title(&self.project.manifest.name)
            .with_inner_size(LogicalSize::new(size.width, size.height));
        let window = Arc::new(event_loop.create_window(attributes).map_err(|e| e.to_string())?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            event_loop.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let physical = window.inner_size();
        let gpu = pollster::block_on(Gpu::new(&instance, surface, physical.width, physical.height))?;
        let renderer = Renderer::new(
            gpu.device.clone(),
            gpu.queue.clone(),
            gpu.view_format,
            (size.width, size.height),
            gpu.size(),
        );
        let assets = Assets::dir(
            self.project.assets_dir(),
            self.project.manifest.assets.packing_key.as_deref(),
            renderer.max_texture_size(),
        )?;
        let saves = SaveStore::for_project(&self.project.manifest.name);
        let mut player = Player::new(renderer, &self.project, self.theme.clone(), assets, saves, true)?;
        player.start_audio();
        player.start();
        Ok(Running { player, gpu, last: Instant::now(), window, shift: false })
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match self.open(event_loop) {
            Ok(running) => {
                running.window.request_redraw();
                self.running = Some(running);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(r) = &mut self.running {
            r.player.flush_persistent();
            #[cfg(feature = "steam")]
            if let Some(steam) = &self.steam {
                steam.sync_cloud();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(r) = &mut self.running else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                r.gpu.resize(size.width, size.height);
                r.player.resize(size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - r.last).as_secs_f32();
                r.last = now;
                let player = &mut r.player;
                r.gpu.draw(|view| player.frame(dt, view));
                #[cfg(feature = "steam")]
                if let Some(steam) = &mut self.steam {
                    steam.update();
                }
                if r.player.should_quit() {
                    event_loop.exit();
                    return;
                }
                r.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => r.shift = modifiers.state().shift_key(),
            WindowEvent::CursorMoved { position, .. } => r.player.pointer_moved(position.x as f32, position.y as f32),
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => match button {
                MouseButton::Left => r.player.pointer_down(Button::Primary),
                MouseButton::Right => r.player.pointer_down(Button::Secondary),
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 40.0) as f32,
                };
                if lines.abs() >= 0.5 {
                    r.player.wheel(lines);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let key = match &event.logical_key {
                    WinitKey::Named(NamedKey::Space | NamedKey::Enter) => Some(Key::Advance),
                    WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
                    WinitKey::Named(NamedKey::PageUp) => Some(Key::Back),
                    WinitKey::Named(NamedKey::PageDown) => Some(Key::Forward),
                    WinitKey::Named(NamedKey::Tab) => Some(if r.shift { Key::FocusPrevious } else { Key::FocusNext }),
                    WinitKey::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => Some(Key::FocusPrevious),
                    WinitKey::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => Some(Key::FocusNext),
                    WinitKey::Character(c) => match c.as_str() {
                        "a" | "A" => Some(Key::Auto),
                        s => s.parse::<u8>().ok().map(Key::Digit),
                    },
                    _ => None,
                };
                if let Some(key) = key {
                    r.player.key(key);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_game_uses_the_executable_sibling_or_macos_resources_folder() {
        let root = std::env::temp_dir().join(format!("kairos-bundled-game-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let executable = root.join("Demo.app/Contents/MacOS/kairos");
        let directory = executable.parent().unwrap();
        let resources = directory.join("../Resources/game");
        std::fs::create_dir_all(&resources).unwrap();
        assert_eq!(bundled_game_dir(&executable).unwrap(), resources);

        let sibling = directory.join("game");
        std::fs::create_dir_all(&sibling).unwrap();
        assert_eq!(bundled_game_dir(&executable).unwrap(), sibling);
        std::fs::remove_dir_all(root).unwrap();
    }
}
