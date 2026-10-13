//! Structured edits to `.kai` source: blocks for the visual editor and operations that change only
//! the lines they touch.

mod blocks;
mod ops;
mod text;
mod tree;

pub use blocks::{Block, BlockKind, ChoiceOption, IfBranch, Scene, SetOp, scene_blocks};
pub use ops::{Applied, EditError, Focus, Op, Place, SourceFile, apply};
pub use text::label_from_name;
