//! Mutable interaction state tracked across input events.
//!
//! These types are owned by [`super::InputHandler`] and updated as raw
//! events are processed into semantic interactions.

use selean_common::types::NodeId;

/// Set of currently selected node IDs.
#[derive(Debug, Clone, Default)]
pub struct SelectionSet {
    ids: Vec<NodeId>,
}

impl SelectionSet {
    /// Creates an empty selection set.
    #[must_use]
    pub fn new() -> Self {
        Self { ids: Vec::new() }
    }

    /// Replaces the selection with a single node.
    pub fn select_one(&mut self, id: NodeId) {
        self.ids = vec![id];
    }

    /// Toggles a node in/out of the selection.
    ///
    /// If the node is already selected, it is removed. Otherwise it is added.
    pub fn toggle(&mut self, id: NodeId) {
        if let Some(pos) = self.ids.iter().position(|&x| x == id) {
            self.ids.remove(pos);
        } else {
            self.ids.push(id);
        }
    }

    /// Removes all nodes from the selection.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Returns `true` if the given node is selected.
    #[must_use]
    pub fn contains(&self, id: NodeId) -> bool {
        self.ids.contains(&id)
    }

    /// Returns a slice of selected node IDs.
    #[must_use]
    pub fn ids(&self) -> &[NodeId] {
        &self.ids
    }

    /// Returns `true` if no nodes are selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Returns the number of selected nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }
}

/// Tracks an in-progress drag gesture.
#[derive(Debug, Clone)]
pub struct DragPhase {
    /// Node being dragged.
    pub node_id: NodeId,
    /// World-space position where drag started.
    pub start_world: (f32, f32),
    /// Screen-space position where pointer went down (for threshold check).
    pub start_screen: (f32, f32),
    /// Last world-space position (for computing deltas).
    pub last_world: (f32, f32),
    /// Whether the drag threshold has been exceeded.
    pub threshold_exceeded: bool,
}

/// Tracks an in-progress camera pan gesture (middle-button drag).
#[derive(Debug, Clone)]
pub struct CameraPanPhase {
    /// Screen-space position where pan started.
    pub start_screen: (f32, f32),
    /// Camera pan at gesture start.
    pub start_pan: (f32, f32),
}

/// Complete interaction state owned by [`super::InputHandler`].
#[derive(Debug, Clone)]
pub struct InteractionState {
    /// Currently hovered node, if any.
    pub hover_target: Option<NodeId>,
    /// Currently selected nodes.
    pub selection: SelectionSet,
    /// Active drag gesture, if any.
    pub drag: Option<DragPhase>,
    /// Active camera pan gesture, if any.
    pub camera_pan: Option<CameraPanPhase>,
    /// Drag threshold in screen pixels. Movement below this is treated as a click.
    pub drag_threshold: f32,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            hover_target: None,
            selection: SelectionSet::new(),
            drag: None,
            camera_pan: None,
            drag_threshold: 4.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_set_select_one() {
        let mut set = SelectionSet::new();
        let id = NodeId::new();
        set.select_one(id);
        assert_eq!(set.ids(), &[id]);
    }

    #[test]
    fn selection_set_toggle_add() {
        let mut set = SelectionSet::new();
        let id = NodeId::new();
        set.toggle(id);
        assert!(set.contains(id));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn selection_set_toggle_remove() {
        let mut set = SelectionSet::new();
        let id = NodeId::new();
        set.toggle(id);
        set.toggle(id);
        assert!(!set.contains(id));
        assert!(set.is_empty());
    }

    #[test]
    fn selection_set_clear() {
        let mut set = SelectionSet::new();
        set.select_one(NodeId::new());
        set.toggle(NodeId::new());
        assert!(!set.is_empty());
        set.clear();
        assert!(set.is_empty());
    }

    #[test]
    fn selection_set_contains() {
        let mut set = SelectionSet::new();
        let a = NodeId::new();
        let b = NodeId::new();
        set.select_one(a);
        assert!(set.contains(a));
        assert!(!set.contains(b));
    }

    #[test]
    fn selection_set_is_empty_and_len() {
        let mut set = SelectionSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        set.select_one(NodeId::new());
        assert!(!set.is_empty());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn selection_set_select_one_replaces_previous() {
        let mut set = SelectionSet::new();
        let a = NodeId::new();
        let b = NodeId::new();
        set.select_one(a);
        set.select_one(b);
        assert!(!set.contains(a));
        assert!(set.contains(b));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn drag_phase_fields() {
        let id = NodeId::new();
        let drag = DragPhase {
            node_id: id,
            start_world: (10.0, 20.0),
            start_screen: (100.0, 200.0),
            last_world: (10.0, 20.0),
            threshold_exceeded: false,
        };
        assert_eq!(drag.node_id, id);
        assert_eq!(drag.start_world, (10.0, 20.0));
        assert!(!drag.threshold_exceeded);
    }

    #[test]
    fn camera_pan_phase_fields() {
        let phase = CameraPanPhase {
            start_screen: (50.0, 60.0),
            start_pan: (1.0, 2.0),
        };
        assert_eq!(phase.start_screen, (50.0, 60.0));
        assert_eq!(phase.start_pan, (1.0, 2.0));
    }

    #[test]
    fn interaction_state_default() {
        let state = InteractionState::default();
        assert!(state.hover_target.is_none());
        assert!(state.selection.is_empty());
        assert!(state.drag.is_none());
        assert!(state.camera_pan.is_none());
    }

    #[test]
    fn interaction_state_drag_threshold_default_is_4() {
        let state = InteractionState::default();
        assert!((state.drag_threshold - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn selection_set_toggle_duplicate_removes() {
        let mut set = SelectionSet::new();
        let a = NodeId::new();
        let b = NodeId::new();
        set.toggle(a);
        set.toggle(b);
        assert_eq!(set.len(), 2);
        set.toggle(a);
        assert_eq!(set.len(), 1);
        assert!(!set.contains(a));
        assert!(set.contains(b));
    }
}
