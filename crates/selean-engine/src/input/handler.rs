//! Input event processing and interaction management.
//!
//! [`InputHandler`] converts raw [`InputEvent`] values into semantic
//! [`InteractionEvent`] values by maintaining [`InteractionState`] across
//! events. It performs hit testing via the scene graph and coordinate
//! conversion via the camera.

use selean_common::types::NodeId;

use super::event::{InputEvent, InteractionEvent, Modifiers, PointerButton};
use super::state::{CameraPanPhase, DragPhase, InteractionState};
use crate::renderer::Camera;
use crate::scene::SceneGraph;

/// Processes raw input events into semantic interaction events.
///
/// Owns [`InteractionState`] (hover target, selection set, drag phase, camera
/// pan phase). Borrows `&mut SceneGraph` for hit testing and scroll offset
/// mutation, and `&mut Camera` for coordinate conversion and camera manipulation.
pub struct InputHandler {
    state: InteractionState,
}

impl InputHandler {
    /// Creates a new handler with default state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: InteractionState::default(),
        }
    }

    /// Creates a new handler with a custom drag threshold in screen pixels.
    #[must_use]
    pub fn with_drag_threshold(threshold: f32) -> Self {
        Self {
            state: InteractionState {
                drag_threshold: threshold,
                ..InteractionState::default()
            },
        }
    }

    /// Processes a raw input event and returns semantic interaction events.
    ///
    /// `scene` is borrowed mutably for hit testing and scroll offset mutation.
    /// `camera` is borrowed mutably for coordinate conversion and camera pan/zoom.
    pub fn handle_event(
        &mut self,
        event: &InputEvent,
        scene: &mut SceneGraph,
        camera: &mut Camera,
    ) -> Vec<InteractionEvent> {
        match event {
            InputEvent::PointerMove { x, y, .. } => self.handle_pointer_move(*x, *y, scene, camera),
            InputEvent::PointerDown {
                x,
                y,
                button,
                modifiers,
            } => self.handle_pointer_down(*x, *y, *button, *modifiers, scene, camera),
            InputEvent::PointerUp {
                x,
                y,
                button,
                modifiers,
            } => self.handle_pointer_up(*x, *y, *button, *modifiers, scene, camera),
            InputEvent::ScrollDelta {
                x,
                y,
                dx,
                dy,
                modifiers,
            } => self.handle_scroll(*x, *y, *dx, *dy, *modifiers, scene, camera),
        }
    }

    /// Read-only access to current interaction state.
    #[must_use]
    pub fn state(&self) -> &InteractionState {
        &self.state
    }

    /// Mutable access to interaction state for external manipulation.
    pub fn state_mut(&mut self) -> &mut InteractionState {
        &mut self.state
    }

    fn handle_pointer_move(
        &mut self,
        x: f32,
        y: f32,
        scene: &mut SceneGraph,
        camera: &mut Camera,
    ) -> Vec<InteractionEvent> {
        // Camera pan takes priority over everything.
        if let Some(ref pan_phase) = self.state.camera_pan {
            let dx = x - pan_phase.start_screen.0;
            let dy = y - pan_phase.start_screen.1;
            let new_pan_x = pan_phase.start_pan.0 - dx / camera.zoom();
            let new_pan_y = pan_phase.start_pan.1 - dy / camera.zoom();
            camera.set_pan(new_pan_x, new_pan_y);
            return vec![InteractionEvent::CameraPanned {
                pan_x: new_pan_x,
                pan_y: new_pan_y,
            }];
        }

        // Drag in progress.
        if let Some(ref mut drag) = self.state.drag {
            let (world_x, world_y) = camera.screen_to_world(x, y);

            if !drag.threshold_exceeded {
                let dist_sq = (x - drag.start_screen.0).powi(2) + (y - drag.start_screen.1).powi(2);
                if dist_sq >= self.state.drag_threshold.powi(2) {
                    drag.threshold_exceeded = true;
                    drag.last_world = (world_x, world_y);
                    return vec![InteractionEvent::DragStarted {
                        node_id: drag.node_id,
                        world_x,
                        world_y,
                    }];
                }
                return vec![];
            }

            let delta_x = world_x - drag.last_world.0;
            let delta_y = world_y - drag.last_world.1;
            drag.last_world = (world_x, world_y);
            return vec![InteractionEvent::DragMoved {
                node_id: drag.node_id,
                world_x,
                world_y,
                delta_x,
                delta_y,
            }];
        }

        // Hover detection.
        let (world_x, world_y) = camera.screen_to_world(x, y);
        let hits = scene.hit_test(world_x, world_y);
        let new_hover = hits.first().copied();
        if new_hover != self.state.hover_target {
            let old = self.state.hover_target;
            self.state.hover_target = new_hover;
            return vec![InteractionEvent::HoverChanged {
                old,
                new: new_hover,
            }];
        }
        vec![]
    }

    fn handle_pointer_down(
        &mut self,
        x: f32,
        y: f32,
        button: PointerButton,
        _modifiers: Modifiers,
        scene: &mut SceneGraph,
        camera: &Camera,
    ) -> Vec<InteractionEvent> {
        // Middle button starts camera pan.
        if button == PointerButton::Middle {
            let (pan_x, pan_y) = camera.pan();
            self.state.camera_pan = Some(CameraPanPhase {
                start_screen: (x, y),
                start_pan: (pan_x, pan_y),
            });
            return vec![];
        }

        // Ignore non-middle buttons during active camera pan.
        if self.state.camera_pan.is_some() {
            return vec![];
        }

        // Hit test for potential drag/click.
        let (world_x, world_y) = camera.screen_to_world(x, y);
        let hits = scene.hit_test(world_x, world_y);
        if let Some(&node_id) = hits.first() {
            self.state.drag = Some(DragPhase {
                node_id,
                start_world: (world_x, world_y),
                start_screen: (x, y),
                last_world: (world_x, world_y),
                threshold_exceeded: false,
            });
        }
        vec![]
    }

    fn handle_pointer_up(
        &mut self,
        x: f32,
        y: f32,
        button: PointerButton,
        modifiers: Modifiers,
        scene: &mut SceneGraph,
        camera: &Camera,
    ) -> Vec<InteractionEvent> {
        let (world_x, world_y) = camera.screen_to_world(x, y);

        // Middle button release ends camera pan.
        if button == PointerButton::Middle {
            if self.state.camera_pan.is_some() {
                self.state.camera_pan = None;
            }
            return vec![];
        }

        // Drag end or click.
        if let Some(drag) = self.state.drag.take() {
            if drag.threshold_exceeded {
                return vec![InteractionEvent::DragEnded {
                    node_id: drag.node_id,
                    world_x,
                    world_y,
                }];
            }
            // Threshold not exceeded: treat as click.
            return self.handle_click(world_x, world_y, button, modifiers, scene);
        }

        // PointerUp without matching PointerDown: treat as click.
        self.handle_click(world_x, world_y, button, modifiers, scene)
    }

    fn handle_click(
        &mut self,
        world_x: f32,
        world_y: f32,
        button: PointerButton,
        modifiers: Modifiers,
        scene: &mut SceneGraph,
    ) -> Vec<InteractionEvent> {
        let hits = scene.hit_test(world_x, world_y);
        let mut events = Vec::new();

        if let Some(&node_id) = hits.first() {
            events.push(InteractionEvent::Clicked {
                node_id,
                button,
                modifiers,
            });
            if modifiers.shift {
                self.state.selection.toggle(node_id);
            } else {
                self.state.selection.select_one(node_id);
            }
            events.push(InteractionEvent::SelectionChanged {
                selected: self.state.selection.ids().to_vec(),
            });
        } else {
            events.push(InteractionEvent::ClickedCanvas { button, modifiers });
            if !self.state.selection.is_empty() {
                self.state.selection.clear();
                events.push(InteractionEvent::SelectionChanged {
                    selected: Vec::new(),
                });
            }
        }
        events
    }

    fn handle_scroll(
        &mut self,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        modifiers: Modifiers,
        scene: &mut SceneGraph,
        camera: &mut Camera,
    ) -> Vec<InteractionEvent> {
        // Ctrl/Meta + scroll = zoom.
        if modifiers.ctrl || modifiers.meta {
            let factor = (-dy * 0.002_f32).exp();
            camera.zoom_at(factor, x, y);
            return vec![InteractionEvent::CameraZoomed {
                zoom: camera.zoom(),
                focus_x: x,
                focus_y: y,
            }];
        }

        // Scroll routing: walk from hit node up through ancestors.
        let (world_x, world_y) = camera.screen_to_world(x, y);
        let hits = scene.hit_test(world_x, world_y);
        let mut remaining_dx = dx;
        let mut remaining_dy = dy;
        let mut events = Vec::new();

        if let Some(&hit_node) = hits.first() {
            let mut current = Some(hit_node);
            while let Some(node_id) = current {
                if let Some(max) = scene.max_scroll(node_id) {
                    let scrollable_x = max[0] > 0.0;
                    let scrollable_y = max[1] > 0.0;

                    if (scrollable_x && remaining_dx.abs() > f32::EPSILON)
                        || (scrollable_y && remaining_dy.abs() > f32::EPSILON)
                    {
                        let current_offset = scene.scroll_offset(node_id).unwrap_or([0.0, 0.0]);
                        let new_x = current_offset[0] + remaining_dx;
                        let new_y = current_offset[1] + remaining_dy;
                        let clamped_x = new_x.clamp(0.0, max[0]);
                        let clamped_y = new_y.clamp(0.0, max[1]);

                        let consumed_dx = clamped_x - current_offset[0];
                        let consumed_dy = clamped_y - current_offset[1];

                        if consumed_dx.abs() > f32::EPSILON || consumed_dy.abs() > f32::EPSILON {
                            scene.set_scroll_offset(node_id, clamped_x, clamped_y);
                            events.push(InteractionEvent::ScrollApplied {
                                node_id,
                                offset: [clamped_x, clamped_y],
                            });
                            remaining_dx -= consumed_dx;
                            remaining_dy -= consumed_dy;
                        }
                    }
                }

                if remaining_dx.abs() < f32::EPSILON && remaining_dy.abs() < f32::EPSILON {
                    break;
                }

                current = scene.parent(node_id);
            }
        }

        // Remaining delta pans the camera.
        if remaining_dx.abs() > f32::EPSILON || remaining_dy.abs() > f32::EPSILON {
            let pan_dx = remaining_dx / camera.zoom();
            let pan_dy = remaining_dy / camera.zoom();
            camera.pan_by(pan_dx, pan_dy);
            let (pan_x, pan_y) = camera.pan();
            events.push(InteractionEvent::CameraPanned { pan_x, pan_y });
        }

        events
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use crate::scene::{BoundingBox, SceneNode, SceneNodeKind};

    /// Creates a frame node at the given position.
    fn frame_node(name: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(x, y, w, h),
        )
    }

    /// Creates a scene with one node at (100, 100, 50, 50) and a camera at origin.
    /// Returns (scene, camera, node_id).
    fn setup_single_node() -> (SceneGraph, Camera, NodeId) {
        let mut scene = SceneGraph::new();
        let node = frame_node("Node", 100.0, 100.0, 50.0, 50.0);
        let id = node.id;
        scene.add_root(node);
        scene.recompute_world_transforms();
        let camera = Camera::new(800.0, 600.0);
        (scene, camera, id)
    }

    /// Converts world coords to screen coords for our test camera (800x600, origin, zoom=1).
    fn world_to_screen(wx: f32, wy: f32) -> (f32, f32) {
        (wx + 400.0, wy + 300.0)
    }

    #[test]
    fn hover_on_node_emits_hover_changed() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::new();

        // Move to center of node: world (125, 125) = screen (525, 425).
        let (sx, sy) = world_to_screen(125.0, 125.0);
        let events = handler.handle_event(
            &InputEvent::PointerMove {
                x: sx,
                y: sy,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        assert_eq!(events.len(), 1);
        match &events[0] {
            InteractionEvent::HoverChanged { old, new } => {
                assert_eq!(*old, None);
                assert_eq!(*new, Some(node_id));
            }
            other => panic!("expected HoverChanged, got {other:?}"),
        }
    }

    #[test]
    fn hover_off_node_emits_hover_changed_to_none() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::new();

        // Hover on node first.
        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerMove {
                x: sx,
                y: sy,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        // Move off node: world (0, 0) = screen (400, 300).
        let events = handler.handle_event(
            &InputEvent::PointerMove {
                x: 400.0,
                y: 300.0,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        assert_eq!(events.len(), 1);
        match &events[0] {
            InteractionEvent::HoverChanged { old, new } => {
                assert_eq!(*old, Some(node_id));
                assert_eq!(*new, None);
            }
            other => panic!("expected HoverChanged, got {other:?}"),
        }
    }

    #[test]
    fn hover_same_node_no_event() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();

        let (sx, sy) = world_to_screen(125.0, 125.0);
        let move_event = InputEvent::PointerMove {
            x: sx,
            y: sy,
            modifiers: Modifiers::default(),
        };

        handler.handle_event(&move_event, &mut scene, &mut camera);
        // Second move to same area, still on node.
        let (sx2, sy2) = world_to_screen(130.0, 130.0);
        let events = handler.handle_event(
            &InputEvent::PointerMove {
                x: sx2,
                y: sy2,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        assert!(events.is_empty());
    }

    #[test]
    fn click_node_selects_it() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::new();

        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        let events = handler.handle_event(
            &InputEvent::PointerUp {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        assert!(events.len() >= 2);
        match &events[0] {
            InteractionEvent::Clicked { node_id: id, .. } => assert_eq!(*id, node_id),
            other => panic!("expected Clicked, got {other:?}"),
        }
        match &events[1] {
            InteractionEvent::SelectionChanged { selected } => {
                assert_eq!(selected, &[node_id]);
            }
            other => panic!("expected SelectionChanged, got {other:?}"),
        }
    }

    #[test]
    fn click_canvas_deselects() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::new();

        // Select node first.
        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        handler.handle_event(
            &InputEvent::PointerUp {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert!(handler.state().selection.contains(node_id));

        // Click on empty canvas.
        handler.handle_event(
            &InputEvent::PointerDown {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        let events = handler.handle_event(
            &InputEvent::PointerUp {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        assert!(handler.state().selection.is_empty());
        let has_canvas_click = events
            .iter()
            .any(|e| matches!(e, InteractionEvent::ClickedCanvas { .. }));
        assert!(has_canvas_click);
        let has_selection_changed = events.iter().any(|e| {
            matches!(
                e,
                InteractionEvent::SelectionChanged { selected } if selected.is_empty()
            )
        });
        assert!(has_selection_changed);
    }

    #[test]
    fn shift_click_toggles_selection() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::new();

        // Add a second node.
        let node2 = frame_node("Node2", 200.0, 200.0, 50.0, 50.0);
        let id2 = node2.id;
        scene.add_root(node2);
        scene.recompute_world_transforms();

        // Click first node (no shift).
        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        handler.handle_event(
            &InputEvent::PointerUp {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert_eq!(handler.state().selection.len(), 1);
        assert!(handler.state().selection.contains(node_id));

        // Shift-click second node.
        let (sx2, sy2) = world_to_screen(225.0, 225.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx2,
                y: sy2,
                button: PointerButton::Left,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            },
            &mut scene,
            &mut camera,
        );
        handler.handle_event(
            &InputEvent::PointerUp {
                x: sx2,
                y: sy2,
                button: PointerButton::Left,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            },
            &mut scene,
            &mut camera,
        );
        assert_eq!(handler.state().selection.len(), 2);
        assert!(handler.state().selection.contains(node_id));
        assert!(handler.state().selection.contains(id2));
    }

    #[test]
    fn shift_click_deselect() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::new();

        // Select node.
        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        handler.handle_event(
            &InputEvent::PointerUp {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert!(handler.state().selection.contains(node_id));

        // Shift-click same node to deselect.
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            },
            &mut scene,
            &mut camera,
        );
        handler.handle_event(
            &InputEvent::PointerUp {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            },
            &mut scene,
            &mut camera,
        );
        assert!(!handler.state().selection.contains(node_id));
    }

    #[test]
    fn drag_below_threshold_is_click() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::with_drag_threshold(10.0);

        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        // Move 3 pixels (below 10px threshold).
        let move_events = handler.handle_event(
            &InputEvent::PointerMove {
                x: sx + 3.0,
                y: sy,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert!(move_events.is_empty());

        // Release: should be a click, not a drag end.
        let events = handler.handle_event(
            &InputEvent::PointerUp {
                x: sx + 3.0,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        let has_click = events
            .iter()
            .any(|e| matches!(e, InteractionEvent::Clicked { node_id: id, .. } if *id == node_id));
        assert!(has_click);
    }

    #[test]
    fn drag_above_threshold_emits_drag_events() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::with_drag_threshold(4.0);

        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        // Move 10 pixels (above 4px threshold).
        let events = handler.handle_event(
            &InputEvent::PointerMove {
                x: sx + 10.0,
                y: sy,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert_eq!(events.len(), 1);
        match &events[0] {
            InteractionEvent::DragStarted { node_id: id, .. } => assert_eq!(*id, node_id),
            other => panic!("expected DragStarted, got {other:?}"),
        }
    }

    #[test]
    fn drag_move_computes_world_deltas() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::with_drag_threshold(2.0);

        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        // Exceed threshold.
        handler.handle_event(
            &InputEvent::PointerMove {
                x: sx + 5.0,
                y: sy,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        // Further movement.
        let events = handler.handle_event(
            &InputEvent::PointerMove {
                x: sx + 15.0,
                y: sy + 10.0,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert_eq!(events.len(), 1);
        match &events[0] {
            InteractionEvent::DragMoved {
                node_id: id,
                delta_x,
                delta_y,
                ..
            } => {
                assert_eq!(*id, node_id);
                // Zoom=1, so screen delta = world delta. Moved from (sx+5, sy) to (sx+15, sy+10).
                assert!((delta_x - 10.0).abs() < 1e-3);
                assert!((delta_y - 10.0).abs() < 1e-3);
            }
            other => panic!("expected DragMoved, got {other:?}"),
        }
    }

    #[test]
    fn drag_end_clears_state() {
        let (mut scene, mut camera, node_id) = setup_single_node();
        let mut handler = InputHandler::with_drag_threshold(2.0);

        let (sx, sy) = world_to_screen(125.0, 125.0);
        handler.handle_event(
            &InputEvent::PointerDown {
                x: sx,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        handler.handle_event(
            &InputEvent::PointerMove {
                x: sx + 20.0,
                y: sy,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        let events = handler.handle_event(
            &InputEvent::PointerUp {
                x: sx + 20.0,
                y: sy,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert_eq!(events.len(), 1);
        match &events[0] {
            InteractionEvent::DragEnded { node_id: id, .. } => assert_eq!(*id, node_id),
            other => panic!("expected DragEnded, got {other:?}"),
        }
        assert!(handler.state().drag.is_none());
    }

    #[test]
    fn middle_button_starts_camera_pan() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();

        handler.handle_event(
            &InputEvent::PointerDown {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Middle,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert!(handler.state().camera_pan.is_some());
    }

    #[test]
    fn middle_button_move_pans_camera() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();

        handler.handle_event(
            &InputEvent::PointerDown {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Middle,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        let events = handler.handle_event(
            &InputEvent::PointerMove {
                x: 450.0,
                y: 350.0,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        assert_eq!(events.len(), 1);
        match &events[0] {
            InteractionEvent::CameraPanned { pan_x, pan_y } => {
                // Moved 50px right, 50px down at zoom=1.
                // Pan moves opposite direction: (-50, -50).
                assert!((*pan_x - (-50.0)).abs() < 1e-3);
                assert!((*pan_y - (-50.0)).abs() < 1e-3);
            }
            other => panic!("expected CameraPanned, got {other:?}"),
        }
        assert_eq!(camera.pan(), (-50.0, -50.0));
    }

    #[test]
    fn middle_button_release_ends_pan() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();

        handler.handle_event(
            &InputEvent::PointerDown {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Middle,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        handler.handle_event(
            &InputEvent::PointerUp {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Middle,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        assert!(handler.state().camera_pan.is_none());
    }

    #[test]
    fn scroll_on_scroll_container_applies_offset() {
        let mut scene = SceneGraph::new();
        let container = frame_node("Container", 100.0, 100.0, 200.0, 200.0);
        let container_id = container.id;
        scene.add_root(container);

        // Add a child that overflows the container.
        let child = frame_node("Child", 0.0, 0.0, 500.0, 500.0);
        scene.add_child(container_id, child);
        scene.recompute_world_transforms();

        let mut camera = Camera::new(800.0, 600.0);
        let mut handler = InputHandler::new();

        // Scroll at center of container: world (200, 200) = screen (600, 500).
        let (sx, sy) = world_to_screen(200.0, 200.0);
        let events = handler.handle_event(
            &InputEvent::ScrollDelta {
                x: sx,
                y: sy,
                dx: 0.0,
                dy: 30.0,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        let has_scroll = events
            .iter()
            .any(|e| matches!(e, InteractionEvent::ScrollApplied { node_id, .. } if *node_id == container_id));
        assert!(has_scroll, "expected ScrollApplied for container");
    }

    #[test]
    fn scroll_routes_to_nearest_scrollable_ancestor() {
        let mut scene = SceneGraph::new();

        // Outer container (scrollable).
        let outer = frame_node("Outer", 100.0, 100.0, 200.0, 200.0);
        let outer_id = outer.id;
        scene.add_root(outer);

        // Inner non-scrollable group.
        let inner = frame_node("Inner", 100.0, 100.0, 200.0, 200.0);
        let inner_id = inner.id;
        scene.add_child(outer_id, inner);

        // Leaf inside inner (hit target).
        let leaf = frame_node("Leaf", 100.0, 100.0, 50.0, 50.0);
        scene.add_child(inner_id, leaf);

        // Add overflowing content to outer so it becomes scrollable.
        let big_child = frame_node("BigChild", 0.0, 0.0, 500.0, 500.0);
        scene.add_child(outer_id, big_child);

        scene.recompute_world_transforms();

        let mut camera = Camera::new(800.0, 600.0);
        let mut handler = InputHandler::new();

        // Scroll at the leaf position.
        let (sx, sy) = world_to_screen(125.0, 125.0);
        let events = handler.handle_event(
            &InputEvent::ScrollDelta {
                x: sx,
                y: sy,
                dx: 0.0,
                dy: 20.0,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        // Should route to outer container (inner has no overflow).
        let scroll_applied = events.iter().find(
            |e| matches!(e, InteractionEvent::ScrollApplied { node_id, .. } if *node_id == outer_id),
        );
        assert!(
            scroll_applied.is_some(),
            "expected scroll to route to outer container"
        );
    }

    #[test]
    fn scroll_on_canvas_pans_camera() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();

        // Scroll on empty area: world (0, 0) = screen (400, 300).
        let events = handler.handle_event(
            &InputEvent::ScrollDelta {
                x: 400.0,
                y: 300.0,
                dx: 0.0,
                dy: 50.0,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        let has_pan = events
            .iter()
            .any(|e| matches!(e, InteractionEvent::CameraPanned { .. }));
        assert!(has_pan, "expected CameraPanned on empty canvas scroll");
    }

    #[test]
    fn ctrl_scroll_zooms_camera() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();
        let original_zoom = camera.zoom();

        let events = handler.handle_event(
            &InputEvent::ScrollDelta {
                x: 400.0,
                y: 300.0,
                dx: 0.0,
                dy: -100.0,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            },
            &mut scene,
            &mut camera,
        );

        assert_eq!(events.len(), 1);
        match &events[0] {
            InteractionEvent::CameraZoomed { zoom, .. } => {
                assert!(*zoom > original_zoom, "expected zoom in");
            }
            other => panic!("expected CameraZoomed, got {other:?}"),
        }
    }

    #[test]
    fn click_empty_with_no_selection_no_deselect_event() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();

        // Click on empty canvas with no prior selection.
        handler.handle_event(
            &InputEvent::PointerDown {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );
        let events = handler.handle_event(
            &InputEvent::PointerUp {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        // Should have ClickedCanvas but no SelectionChanged (selection was already empty).
        let has_canvas_click = events
            .iter()
            .any(|e| matches!(e, InteractionEvent::ClickedCanvas { .. }));
        assert!(has_canvas_click);
        let has_selection_changed = events
            .iter()
            .any(|e| matches!(e, InteractionEvent::SelectionChanged { .. }));
        assert!(!has_selection_changed);
    }

    #[test]
    fn pointer_down_on_canvas_no_drag_phase() {
        let (mut scene, mut camera, _) = setup_single_node();
        let mut handler = InputHandler::new();

        // Click on empty canvas.
        handler.handle_event(
            &InputEvent::PointerDown {
                x: 400.0,
                y: 300.0,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
            &mut scene,
            &mut camera,
        );

        assert!(handler.state().drag.is_none());
    }
}
