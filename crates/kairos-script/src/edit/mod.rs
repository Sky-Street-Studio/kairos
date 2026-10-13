//! Structured edits to `.kai` source: blocks for the visual editor and operations that change only
//! the lines they touch.

mod blocks;

// `quote`, `Lines::splice` and the tree lookups are still waiting for the ops module; drop this once it lands.
#[allow(dead_code)]
mod text;

// `find`, `find_body` and `Body::end` are still waiting for the ops module; drop this once it lands.
#[allow(dead_code)]
mod tree;

pub use blocks::{Block, BlockKind, ChoiceOption, IfBranch, Scene, SetOp, scene_blocks};
pub use text::label_from_name;
