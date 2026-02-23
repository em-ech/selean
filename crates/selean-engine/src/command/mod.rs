//! Undo/redo command system for the scene graph.
//!
//! Every undoable mutation is wrapped in a [`Command`] that captures old state
//! on execute and restores it on undo. [`CommandHistory`] manages the undo/redo
//! stacks with configurable limits and group batching for multi-step operations.

// All Command impls that return string literals trigger this; the trait uses
// `&str` because CommandGroup borrows from `self.label`.
#![allow(clippy::unnecessary_literal_bound)]

mod batch;
mod hierarchy;
mod property;
mod traits;

pub mod history;

pub use batch::CommandGroup;
pub use hierarchy::{
    AddChildCommand, AddRootCommand, RemoveNodeCommand, ReorderChildrenCommand, ReparentCommand,
};
pub use history::CommandHistory;
pub use property::{
    SetAssetRefCommand, SetBlendModeCommand, SetBoundsCommand, SetClipModeCommand, SetFillCommand,
    SetFontSizeCommand, SetNameCommand, SetOpacityCommand, SetPathDataCommand,
    SetScrollOffsetCommand, SetStrokeCommand, SetStrokeWidthCommand, SetTextContentCommand,
    SetTransformCommand, SetVisibleCommand,
};
pub use traits::Command;
