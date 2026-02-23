//! Platform-agnostic input event types.
//!
//! Consumers translate platform events (winit, SDL, web) into [`InputEvent`]
//! and feed them to [`super::InputHandler::handle_event`]. The handler returns
//! [`InteractionEvent`] values describing semantic interactions.

use selean_common::types::NodeId;

/// Mouse/touch button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// Active keyboard modifiers at the time of an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
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
        x: f32,
        y: f32,
        modifiers: Modifiers,
    },
    /// Pointer button pressed at screen-space position.
    PointerDown {
        x: f32,
        y: f32,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// Pointer button released at screen-space position.
    PointerUp {
        x: f32,
        y: f32,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// Scroll input in screen-space pixels. `dx`/`dy` are pixel deltas.
    ScrollDelta {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        modifiers: Modifiers,
    },
}

/// Semantic interaction produced by [`super::InputHandler`] after processing raw events.
#[derive(Debug, Clone)]
pub enum InteractionEvent {
    /// Hover target changed. `old` is `None` if nothing was hovered before.
    HoverChanged {
        old: Option<NodeId>,
        new: Option<NodeId>,
    },
    /// Node was clicked (press + release without exceeding drag threshold).
    Clicked {
        node_id: NodeId,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// Click on empty canvas (no node hit).
    ClickedCanvas {
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// Selection set changed.
    SelectionChanged { selected: Vec<NodeId> },
    /// Drag gesture started on a node (threshold exceeded).
    DragStarted {
        node_id: NodeId,
        world_x: f32,
        world_y: f32,
    },
    /// Drag moved. `delta_x`/`delta_y` are world-space deltas since last move.
    DragMoved {
        node_id: NodeId,
        world_x: f32,
        world_y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    /// Drag gesture ended.
    DragEnded {
        node_id: NodeId,
        world_x: f32,
        world_y: f32,
    },
    /// Scroll offset applied to a container.
    ScrollApplied { node_id: NodeId, offset: [f32; 2] },
    /// Camera panned to new position.
    CameraPanned { pan_x: f32, pan_y: f32 },
    /// Camera zoomed.
    CameraZoomed {
        zoom: f32,
        focus_x: f32,
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
