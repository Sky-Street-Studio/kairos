//! The graphical player: a window (or a browser canvas) that plays a Kairos project.

pub mod assets;
pub mod game;
mod gpu;
#[cfg(not(target_arch = "wasm32"))]
mod native;
pub mod player;
pub mod saves;
pub mod theme;
pub mod view;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::{load_theme, run};
#[cfg(target_arch = "wasm32")]
pub use web::WebPlayer;
