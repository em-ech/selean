//! Hierarchy commands for the undo/redo system.
//!
//! Commands that mutate the scene graph tree structure: adding, removing,
//! reparenting, and reordering nodes.

use selean_common::types::NodeId;
use smallvec::SmallVec;

use super::traits::Command;
use crate::scene::{SceneGraph, SceneNode};

// --- AddRootCommand ---

/// Adds a new root node to the scene graph.
///
/// The node is consumed on execute (moved into the scene). On undo, the node
/// is cloned from the scene before removal so it can be re-added on redo.
#[derive(Debug)]
pub struct AddRootCommand {
    /// The node to add. `Some` before first execute, then repopulated on undo.
    node: Option<SceneNode>,
    /// The ID of the node (cached for removal on undo).
    node_id: NodeId,
}

impl AddRootCommand {
    /// Creates a command that will add the given node as a root.
    #[must_use]
    pub fn new(node: SceneNode) -> Self {
        let node_id = node.id;
        Self {
            node: Some(node),
            node_id,
        }
    }
}

impl Command for AddRootCommand {
    fn execute(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(node) = self.node.take() else {
            return false;
        };
        scene.add_root(node);
        true
    }

    fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(node) = scene.get(self.node_id) else {
            return false;
        };
        self.node = Some(node.clone());
        scene.remove(self.node_id)
    }

    fn description(&self) -> &str {
        "Add Root"
    }
}

// --- AddChildCommand ---

/// Adds a new child node under a parent.
///
/// Same ownership pattern as `AddRootCommand`.
#[derive(Debug)]
pub struct AddChildCommand {
    parent_id: NodeId,
    node: Option<SceneNode>,
    node_id: NodeId,
}

impl AddChildCommand {
    /// Creates a command that will add the given node as a child of `parent_id`.
    #[must_use]
    pub fn new(parent_id: NodeId, node: SceneNode) -> Self {
        let node_id = node.id;
        Self {
            parent_id,
            node: Some(node),
            node_id,
        }
    }
}

impl Command for AddChildCommand {
    fn execute(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(node) = self.node.take() else {
            return false;
        };
        scene.add_child(self.parent_id, node)
    }

    fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(node) = scene.get(self.node_id) else {
            return false;
        };
        self.node = Some(node.clone());
        scene.remove(self.node_id)
    }

    fn description(&self) -> &str {
        "Add Child"
    }
}

// --- RemoveNodeCommand ---

/// Snapshot of a single node for subtree restoration.
#[derive(Debug, Clone)]
struct NodeSnapshot {
    /// The node data (with children and parent cleared for re-insertion).
    node: SceneNode,
    /// The parent of this node at the time of removal, or `None` for root.
    parent_id: Option<NodeId>,
    /// The index of this node in its parent's children list (or in roots).
    child_index: usize,
}

/// Removes a node and its entire subtree from the scene graph.
///
/// On execute, performs a DFS to capture every node in the subtree as a
/// `NodeSnapshot` (node clone + position info). On undo, re-inserts top-down
/// using `insert_root_at`/`insert_child_at` to restore the exact positions.
#[derive(Debug)]
pub struct RemoveNodeCommand {
    node_id: NodeId,
    snapshots: Vec<NodeSnapshot>,
}

impl RemoveNodeCommand {
    /// Creates a command that will remove the given node and its subtree.
    #[must_use]
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            snapshots: Vec::new(),
        }
    }

    /// DFS capture of the subtree rooted at `id`.
    fn capture_subtree(scene: &SceneGraph, id: NodeId, snapshots: &mut Vec<NodeSnapshot>) {
        let Some(node) = scene.get(id) else {
            return;
        };

        let parent_id = node.parent;
        let child_index = if let Some(pid) = parent_id {
            scene
                .children(pid)
                .map_or(0, |kids| kids.iter().position(|&c| c == id).unwrap_or(0))
        } else {
            scene.roots().iter().position(|&r| r == id).unwrap_or(0)
        };

        let children = node.children.clone();

        snapshots.push(NodeSnapshot {
            node: node.clone(),
            parent_id,
            child_index,
        });

        for child_id in children {
            Self::capture_subtree(scene, child_id, snapshots);
        }
    }
}

impl Command for RemoveNodeCommand {
    fn execute(&mut self, scene: &mut SceneGraph) -> bool {
        if !scene.contains(self.node_id) {
            return false;
        }

        self.snapshots.clear();
        Self::capture_subtree(scene, self.node_id, &mut self.snapshots);
        scene.remove(self.node_id)
    }

    fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        if self.snapshots.is_empty() {
            return false;
        }

        // Re-insert top-down (the order captured by DFS pre-order).
        for snapshot in &self.snapshots {
            // Clear parent/children on the clone since the scene graph methods
            // will set these during insertion.
            let mut node = snapshot.node.clone();
            node.parent = None;
            node.children.clear();

            if let Some(pid) = snapshot.parent_id {
                if !scene.insert_child_at(pid, node, snapshot.child_index) {
                    return false;
                }
            } else {
                scene.insert_root_at(node, snapshot.child_index);
            }
        }

        true
    }

    fn description(&self) -> &str {
        "Remove Node"
    }
}

// --- ReparentCommand ---

/// Moves a node from one parent to another.
///
/// Captures the old parent ID and the node's index in the old parent's children
/// list so it can be restored on undo.
#[derive(Debug)]
pub struct ReparentCommand {
    node_id: NodeId,
    new_parent_id: NodeId,
    old_parent_id: Option<NodeId>,
    old_child_index: usize,
}

impl ReparentCommand {
    /// Creates a command that will move `node_id` to be a child of `new_parent_id`.
    #[must_use]
    pub fn new(node_id: NodeId, new_parent_id: NodeId) -> Self {
        Self {
            node_id,
            new_parent_id,
            old_parent_id: None,
            old_child_index: 0,
        }
    }
}

impl Command for ReparentCommand {
    fn execute(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(node) = scene.get(self.node_id) else {
            return false;
        };

        self.old_parent_id = node.parent;

        if let Some(pid) = self.old_parent_id {
            self.old_child_index = scene.children(pid).map_or(0, |kids| {
                kids.iter().position(|&c| c == self.node_id).unwrap_or(0)
            });
        } else {
            self.old_child_index = scene
                .roots()
                .iter()
                .position(|&r| r == self.node_id)
                .unwrap_or(0);
        }

        scene.reparent(self.node_id, self.new_parent_id)
    }

    fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        if !scene.contains(self.node_id) {
            return false;
        }

        if let Some(old_pid) = self.old_parent_id {
            // Reparent back to old parent.
            if !scene.reparent(self.node_id, old_pid) {
                return false;
            }
            // Restore original index within parent's children.
            if let Some(parent) = scene.get(old_pid) {
                let current_children = parent.children.clone();
                let current_pos = current_children
                    .iter()
                    .position(|&c| c == self.node_id)
                    .unwrap_or(0);
                if current_pos != self.old_child_index {
                    let mut new_order = current_children;
                    new_order.remove(current_pos);
                    let insert_at = self.old_child_index.min(new_order.len());
                    new_order.insert(insert_at, self.node_id);
                    scene.reorder_children(old_pid, &new_order);
                }
            }
        } else {
            // Was a root node. Detach from current parent and restore as root.
            scene.reparent_to_root(self.node_id, self.old_child_index);
        }

        true
    }

    fn description(&self) -> &str {
        "Reparent"
    }
}

// --- ReorderRootsCommand ---

/// Reorders the root nodes of the scene graph.
///
/// Captures the old root order on execute so it can be restored on undo.
#[derive(Debug)]
pub struct ReorderRootsCommand {
    new_order: Vec<NodeId>,
    old_order: Option<Vec<NodeId>>,
}

impl ReorderRootsCommand {
    /// Creates a command that will reorder the root nodes.
    #[must_use]
    pub fn new(new_order: Vec<NodeId>) -> Self {
        Self {
            new_order,
            old_order: None,
        }
    }
}

impl Command for ReorderRootsCommand {
    fn execute(&mut self, scene: &mut SceneGraph) -> bool {
        self.old_order = Some(scene.roots().to_vec());
        scene.reorder_roots(&self.new_order)
    }

    fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(old) = self.old_order.take() else {
            return false;
        };
        scene.reorder_roots(&old)
    }

    fn description(&self) -> &str {
        "Reorder Roots"
    }
}

// --- ReorderChildrenCommand ---

/// Reorders the children of a node.
///
/// Captures the old order on execute so it can be restored on undo.
#[derive(Debug)]
pub struct ReorderChildrenCommand {
    parent_id: NodeId,
    new_order: Vec<NodeId>,
    old_order: Option<SmallVec<[NodeId; 8]>>,
}

impl ReorderChildrenCommand {
    /// Creates a command that will reorder the children of `parent_id`.
    #[must_use]
    pub fn new(parent_id: NodeId, new_order: Vec<NodeId>) -> Self {
        Self {
            parent_id,
            new_order,
            old_order: None,
        }
    }
}

impl Command for ReorderChildrenCommand {
    fn execute(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(node) = scene.get(self.parent_id) else {
            return false;
        };
        self.old_order = Some(node.children.clone());
        scene.reorder_children(self.parent_id, &self.new_order)
    }

    fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        let Some(old) = self.old_order.take() else {
            return false;
        };
        scene.reorder_children(self.parent_id, &old)
    }

    fn description(&self) -> &str {
        "Reorder Children"
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::scene::{BoundingBox, SceneNodeKind};

    fn make_frame(name: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        )
    }

    // --- AddRootCommand ---

    #[test]
    fn add_root_execute_undo_redo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("Root");
        let id = node.id;

        let mut cmd = AddRootCommand::new(node);

        // Execute: node added.
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.len(), 1);
        assert!(scene.contains(id));

        // Undo: node removed.
        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.len(), 0);
        assert!(!scene.contains(id));

        // Redo: node re-added.
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.len(), 1);
        assert!(scene.contains(id));
    }

    // --- AddChildCommand ---

    #[test]
    fn add_child_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let parent = make_frame("Parent");
        let parent_id = scene.add_root(parent);

        let child = make_frame("Child");
        let child_id = child.id;
        let mut cmd = AddChildCommand::new(parent_id, child);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.len(), 2);
        assert_eq!(scene.children(parent_id).unwrap().len(), 1);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.len(), 1);
        assert!(scene.children(parent_id).unwrap().is_empty());
        assert!(!scene.contains(child_id));
    }

    #[test]
    fn add_child_to_nonexistent_parent_returns_false() {
        let mut scene = SceneGraph::new();
        let child = make_frame("Child");
        let fake_parent = NodeId::new();

        let mut cmd = AddChildCommand::new(fake_parent, child);
        assert!(!cmd.execute(&mut scene));
    }

    // --- RemoveNodeCommand ---

    #[test]
    fn remove_leaf_node() {
        let mut scene = SceneGraph::new();
        let parent = make_frame("Parent");
        let parent_id = scene.add_root(parent);
        let child = make_frame("Child");
        let child_id = child.id;
        scene.add_child(parent_id, child);

        let mut cmd = RemoveNodeCommand::new(child_id);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.len(), 1);
        assert!(!scene.contains(child_id));

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.len(), 2);
        assert!(scene.contains(child_id));
        assert_eq!(scene.parent(child_id), Some(parent_id));
    }

    #[test]
    fn remove_subtree_with_children() {
        let mut scene = SceneGraph::new();
        let root = make_frame("Root");
        let root_id = scene.add_root(root);

        let child = make_frame("Child");
        let child_id = child.id;
        scene.add_child(root_id, child);

        let grandchild = make_frame("Grandchild");
        let grandchild_id = grandchild.id;
        scene.add_child(child_id, grandchild);

        let mut cmd = RemoveNodeCommand::new(child_id);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.len(), 1);
        assert!(!scene.contains(child_id));
        assert!(!scene.contains(grandchild_id));

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.len(), 3);
        assert!(scene.contains(child_id));
        assert!(scene.contains(grandchild_id));
        assert_eq!(scene.parent(child_id), Some(root_id));
        assert_eq!(scene.parent(grandchild_id), Some(child_id));
    }

    #[test]
    fn remove_preserves_child_order() {
        let mut scene = SceneGraph::new();
        let parent = make_frame("Parent");
        let parent_id = scene.add_root(parent);

        let a = make_frame("A");
        let a_id = a.id;
        scene.add_child(parent_id, a);
        let b = make_frame("B");
        let b_id = b.id;
        scene.add_child(parent_id, b);
        let c = make_frame("C");
        let c_id = c.id;
        scene.add_child(parent_id, c);

        // Remove B (middle child).
        let mut cmd = RemoveNodeCommand::new(b_id);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.children(parent_id).unwrap(), &[a_id, c_id]);

        // Undo: B should be back at index 1.
        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.children(parent_id).unwrap(), &[a_id, b_id, c_id]);
    }

    #[test]
    fn remove_root_node() {
        let mut scene = SceneGraph::new();
        let r1 = make_frame("R1");
        let r1_id = scene.add_root(r1);
        let r2 = make_frame("R2");
        let r2_id = scene.add_root(r2);

        let mut cmd = RemoveNodeCommand::new(r1_id);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.roots().len(), 1);
        assert_eq!(scene.roots()[0], r2_id);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.roots().len(), 2);
        assert_eq!(scene.roots()[0], r1_id);
        assert_eq!(scene.roots()[1], r2_id);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut scene = SceneGraph::new();
        let mut cmd = RemoveNodeCommand::new(NodeId::new());
        assert!(!cmd.execute(&mut scene));
    }

    #[test]
    fn remove_subtree_re_redo() {
        let mut scene = SceneGraph::new();
        let root = make_frame("Root");
        let root_id = scene.add_root(root);
        let child = make_frame("Child");
        let child_id = child.id;
        scene.add_child(root_id, child);

        let mut cmd = RemoveNodeCommand::new(child_id);

        // Execute -> Undo -> Execute (redo).
        assert!(cmd.execute(&mut scene));
        assert!(cmd.undo(&mut scene));
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.len(), 1);
        assert!(!scene.contains(child_id));

        // Undo again.
        assert!(cmd.undo(&mut scene));
        assert!(scene.contains(child_id));
    }

    // --- ReparentCommand ---

    #[test]
    fn reparent_and_undo() {
        let mut scene = SceneGraph::new();
        let a = make_frame("A");
        let a_id = scene.add_root(a);
        let b = make_frame("B");
        let b_id = scene.add_root(b);
        let child = make_frame("Child");
        let child_id = child.id;
        scene.add_child(a_id, child);

        let mut cmd = ReparentCommand::new(child_id, b_id);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.parent(child_id), Some(b_id));
        assert!(scene.children(a_id).unwrap().is_empty());
        assert_eq!(scene.children(b_id).unwrap(), &[child_id]);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.parent(child_id), Some(a_id));
        assert_eq!(scene.children(a_id).unwrap(), &[child_id]);
        assert!(scene.children(b_id).unwrap().is_empty());
    }

    // --- ReorderChildrenCommand ---

    #[test]
    fn reorder_children_and_undo() {
        let mut scene = SceneGraph::new();
        let parent = make_frame("Parent");
        let parent_id = scene.add_root(parent);

        let a = make_frame("A");
        let a_id = a.id;
        scene.add_child(parent_id, a);
        let b = make_frame("B");
        let b_id = b.id;
        scene.add_child(parent_id, b);
        let c = make_frame("C");
        let c_id = c.id;
        scene.add_child(parent_id, c);

        let mut cmd = ReorderChildrenCommand::new(parent_id, vec![c_id, a_id, b_id]);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.children(parent_id).unwrap(), &[c_id, a_id, b_id]);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.children(parent_id).unwrap(), &[a_id, b_id, c_id]);
    }

    #[test]
    fn reparent_root_to_child_and_undo() {
        let mut scene = SceneGraph::new();
        let a = make_frame("A");
        let a_id = scene.add_root(a);
        let b = make_frame("B");
        let b_id = scene.add_root(b);

        // Reparent A (root) to become a child of B.
        let mut cmd = ReparentCommand::new(a_id, b_id);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.parent(a_id), Some(b_id));
        assert_eq!(scene.roots().len(), 1);
        assert_eq!(scene.roots()[0], b_id);

        // Undo: A should be a root again at original index (0).
        assert!(cmd.undo(&mut scene));
        assert!(scene.parent(a_id).is_none());
        assert_eq!(scene.roots().len(), 2);
        assert_eq!(scene.roots()[0], a_id);
        assert_eq!(scene.roots()[1], b_id);
    }

    // --- ReorderRootsCommand ---

    #[test]
    fn reorder_roots_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let r1 = make_frame("R1");
        let r1_id = scene.add_root(r1);
        let r2 = make_frame("R2");
        let r2_id = scene.add_root(r2);
        let r3 = make_frame("R3");
        let r3_id = scene.add_root(r3);

        let mut cmd = ReorderRootsCommand::new(vec![r3_id, r1_id, r2_id]);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.roots(), &[r3_id, r1_id, r2_id]);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.roots(), &[r1_id, r2_id, r3_id]);
    }

    #[test]
    fn reorder_roots_invalid_permutation_fails() {
        let mut scene = SceneGraph::new();
        let r1 = make_frame("R1");
        let r1_id = scene.add_root(r1);
        let r2 = make_frame("R2");
        scene.add_root(r2);

        // Wrong length.
        let mut cmd = ReorderRootsCommand::new(vec![r1_id]);
        assert!(!cmd.execute(&mut scene));
    }

    #[test]
    fn reorder_roots_redo_works() {
        let mut scene = SceneGraph::new();
        let r1 = make_frame("R1");
        let r1_id = scene.add_root(r1);
        let r2 = make_frame("R2");
        let r2_id = scene.add_root(r2);

        let mut cmd = ReorderRootsCommand::new(vec![r2_id, r1_id]);
        cmd.execute(&mut scene);
        cmd.undo(&mut scene);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.roots(), &[r2_id, r1_id]);
    }

    #[test]
    fn add_root_at_specific_index_preserved() {
        let mut scene = SceneGraph::new();
        let r1 = make_frame("R1");
        let r1_id = scene.add_root(r1);
        let r2 = make_frame("R2");
        let r2_id = scene.add_root(r2);
        let r3 = make_frame("R3");
        let r3_id = scene.add_root(r3);

        // Remove middle root.
        let mut cmd = RemoveNodeCommand::new(r2_id);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.roots(), &[r1_id, r3_id]);

        // Undo restores at original index.
        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.roots(), &[r1_id, r2_id, r3_id]);
    }
}
