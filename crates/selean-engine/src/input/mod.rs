//! Input handling and interaction state for the design canvas.
//!
//! This module provides platform-agnostic input event types and an
//! [`InputHandler`] that converts raw pointer/scroll/keyboard events
//! into semantic interactions (hover, selection, drag, scroll routing,
//! camera pan/zoom).

mod event;
mod handler;
mod state;

pub use event::{InputEvent, InteractionEvent, Modifiers, PointerButton};
pub use handler::InputHandler;
pub use state::InteractionState;
