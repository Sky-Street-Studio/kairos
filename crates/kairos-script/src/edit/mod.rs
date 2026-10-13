//! Structured edits to `.kai` source: blocks for the visual editor and operations that change only
//! the lines they touch.

// Later edit modules are the callers of `Lines` and `quote`; drop this once they land.
#[allow(dead_code)]
mod text;

pub use text::label_from_name;
