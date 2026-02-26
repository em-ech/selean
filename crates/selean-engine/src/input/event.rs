//! Platform-agnostic input event types.
//!
//! Consumers translate platform events (winit, SDL, web) into [`InputEvent`]
//! and feed them to [`super::InputHandler::handle_event`]. The handler returns
//! [`InteractionEvent`] values describing semantic interactions.

use selean_common::types::NodeId;
use serde::Serialize;

/// Mouse/touch button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PointerButton {
    /// Primary (left) button.
    Left,
    /// Secondary (right) button.
    Right,
    /// Middle (wheel) button.
    Middle,
}

/// Active keyboard modifiers at the time of an input event.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Modifiers {
    /// Shift key held.
    pub shift: bool,
    /// Control key held.
    pub ctrl: bool,
    /// Alt/Option key held.
    pub alt: bool,
    /// Meta/Command/Windows key held.
    pub meta: bool,
}

/// Platform-agnostic input event.
///
/// Consumer translates platform events (winit, SDL, web) into these variants
/// and feeds them to [`super::InputHandler::handle_event`].
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Pointer moved to screen-space position.
    PointerMove {
        /// Screen-space X coordinate.
        x: f32,
        /// Screen-space Y coordinate.
        y: f32,
        /// Active keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Pointer button pressed at screen-space position.
    PointerDown {
        /// Screen-space X coordinate.
        x: f32,
        /// Screen-space Y coordinate.
        y: f32,
        /// Which button was pressed.
        button: PointerButton,
        /// Active keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Pointer button released at screen-space position.
    PointerUp {
        /// Screen-space X coordinate.
        x: f32,
        /// Screen-space Y coordinate.
        y: f32,
        /// Which button was released.
        button: PointerButton,
        /// Active keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Scroll input in screen-space pixels. `dx`/`dy` are pixel deltas.
    ScrollDelta {
        /// Screen-space X coordinate of the pointer.
        x: f32,
        /// Screen-space Y coordinate of the pointer.
        y: f32,
        /// Horizontal scroll delta in pixels.
        dx: f32,
        /// Vertical scroll delta in pixels.
        dy: f32,
        /// Active keyboard modifiers.
        modifiers: Modifiers,
    },
}

/// Semantic interaction produced by [`super::InputHandler`] after processing raw events.
#[derive(Debug, Clone, Serialize)]
pub enum InteractionEvent {
    /// Hover target changed. `old` is `None` if nothing was hovered before.
    HoverChanged {
        /// Previously hovered node.
        old: Option<NodeId>,
        /// Newly hovered node.
        new: Option<NodeId>,
    },
    /// Node was clicked (press + release without exceeding drag threshold).
    Clicked {
        /// Node that was clicked.
        node_id: NodeId,
        /// Which button was used.
        button: PointerButton,
        /// Active keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Click on empty canvas (no node hit).
    ClickedCanvas {
        /// Which button was used.
        button: PointerButton,
        /// Active keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Selection set changed.
    SelectionChanged {
        /// Current set of selected node IDs.
        selected: Vec<NodeId>,
    },
    /// Drag gesture started on a node (threshold exceeded).
    DragStarted {
        /// Node being dragged.
        node_id: NodeId,
        /// World-space X at drag start.
        world_x: f32,
        /// World-space Y at drag start.
        world_y: f32,
    },
    /// Drag moved. `delta_x`/`delta_y` are world-space deltas since last move.
    DragMoved {
        /// Node being dragged.
        node_id: NodeId,
        /// Current world-space X.
        world_x: f32,
        /// Current world-space Y.
        world_y: f32,
        /// World-space X delta since last move.
        delta_x: f32,
        /// World-space Y delta since last move.
        delta_y: f32,
    },
    /// Drag gesture ended.
    DragEnded {
        /// Node that was dragged.
        node_id: NodeId,
        /// World-space X at release.
        world_x: f32,
        /// World-space Y at release.
        world_y: f32,
    },
    /// Scroll offset applied to a container.
    ScrollApplied {
        /// Scroll container node.
        node_id: NodeId,
        /// New scroll offset `[x, y]`.
        offset: [f32; 2],
    },
    /// Camera panned to new position.
    CameraPanned {
        /// New camera pan X.
        pan_x: f32,
        /// New camera pan Y.
        pan_y: f32,
    },
    /// Camera zoomed.
    CameraZoomed {
        /// New zoom level.
        zoom: f32,
        /// Screen-space X focus point.
        focus_x: f32,
        /// Screen-space Y focus point.
        focus_y: f32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_default_all_false() {
        let m = Modifiers::default();
        assert!(!m.shift);
        assert!(!m.ctrl);
        assert!(!m.alt);
        assert!(!m.meta);
    }

    #[test]
    fn pointer_button_equality() {
        assert_eq!(PointerButton::Left, PointerButton::Left);
        assert_ne!(PointerButton::Left, PointerButton::Right);
        assert_ne!(PointerButton::Middle, PointerButton::Left);
    }

    #[test]
    fn input_event_debug_format() {
        let event = InputEvent::PointerMove {
            x: 10.0,
            y: 20.0,
            modifiers: Modifiers::default(),
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("PointerMove"));
    }

    #[test]
    fn interaction_event_variants_constructible() {
        let id = NodeId::new();
        let _ = InteractionEvent::HoverChanged {
            old: None,
            new: Some(id),
        };
        let _ = InteractionEvent::Clicked {
            node_id: id,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
        };
        let _ = InteractionEvent::ClickedCanvas {
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
        };
        let _ = InteractionEvent::SelectionChanged { selected: vec![id] };
        let _ = InteractionEvent::DragStarted {
            node_id: id,
            world_x: 0.0,
            world_y: 0.0,
        };
        let _ = InteractionEvent::DragMoved {
            node_id: id,
            world_x: 1.0,
            world_y: 1.0,
            delta_x: 1.0,
            delta_y: 1.0,
        };
        let _ = InteractionEvent::DragEnded {
            node_id: id,
            world_x: 0.0,
            world_y: 0.0,
        };
        let _ = InteractionEvent::ScrollApplied {
            node_id: id,
            offset: [0.0, 0.0],
        };
        let _ = InteractionEvent::CameraPanned {
            pan_x: 0.0,
            pan_y: 0.0,
        };
        let _ = InteractionEvent::CameraZoomed {
            zoom: 1.0,
            focus_x: 0.0,
            focus_y: 0.0,
        };
    }

    #[test]
    fn modifiers_with_shift() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        assert!(m.shift);
        assert!(!m.ctrl);
    }

    #[test]
    fn modifiers_with_all() {
        let m = Modifiers {
            shift: true,
            ctrl: true,
            alt: true,
            meta: true,
        };
        assert!(m.shift);
        assert!(m.ctrl);
        assert!(m.alt);
        assert!(m.meta);
    }
}
