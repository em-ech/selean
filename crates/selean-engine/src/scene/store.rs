//! Scene graph store — the central data structure for the design node tree.
//!
//! `SceneGraph` owns all scene nodes (in a `HashMap`), their spatial index,
//! and their hierarchical relationships. All mutations go through this struct,
//! which automatically:
//! - Keeps the spatial index in sync with node bounds
//! - Propagates `DIRTY_CHILDREN` up the ancestor chain
//! - Maintains a pre-computed `z_index` for render ordering

use std::collections::HashMap;

use selean_common::types::NodeId;

use super::dirty::DirtyFlags;
use super::node::{BoundingBox, Color, SceneNode, SceneNodeKind};
use crate::spatial::SpatialIndex;

/// The central scene graph store.
///
/// Contains all scene nodes indexed by `NodeId`, with an integrated spatial
/// index for viewport culling and hit testing. All mutations go through
/// methods on this struct to ensure consistency between the node map,
/// parent-child links, spatial index, and dirty flags.
#[derive(Clone)]
pub struct SceneGraph {
    /// All nodes indexed by their unique ID.
    nodes: HashMap<NodeId, SceneNode>,
    /// Spatial index for viewport culling and hit testing.
    spatial: SpatialIndex,
    /// IDs of root nodes (nodes with no parent), in insertion order.
    roots: Vec<NodeId>,
    /// Pre-computed render order index for each node.
    /// Updated on tree structure changes (add, remove, reparent, reorder).
    z_indices: HashMap<NodeId, u32>,
    /// Whether the z-index map needs recomputation.
    z_dirty: bool,
    /// Reusable buffer for viewport query results, avoiding per-frame allocation.
    visible_ids_buf: Vec<NodeId>,
}

impl SceneGraph {
    /// Creates a new empty scene graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            spatial: SpatialIndex::new(),
            roots: Vec::new(),
            z_indices: HashMap::new(),
            z_dirty: false,
            visible_ids_buf: Vec::new(),
        }
    }

    // --- Accessors ---

    /// Returns the number of nodes in the graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the graph has no nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns a reference to a node by its ID, or `None` if not found.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&SceneNode> {
        self.nodes.get(&id)
    }

    /// Returns a mutable reference to a node by its ID, or `None` if not found.
    ///
    /// **Warning:** Direct mutation bypasses dirty propagation and spatial index
    /// sync. Prefer using the typed mutation methods (`set_bounds`, `set_fill`,
    /// etc.) which handle these automatically. Use this only when you need to
    /// modify properties that don't affect the spatial index or render state,
    /// or when you'll call `mark_dirty` and `sync_spatial` yourself.
    #[must_use]
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut SceneNode> {
        self.nodes.get_mut(&id)
    }

    /// Returns `true` if a node with the given ID exists.
    #[must_use]
    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// Returns the IDs of root nodes (nodes with no parent).
    #[must_use]
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Returns a reference to the spatial index.
    #[must_use]
    pub fn spatial(&self) -> &SpatialIndex {
        &self.spatial
    }

    /// Returns a mutable reference to the spatial index (for configuring overscan, etc.).
    #[must_use]
    pub fn spatial_mut(&mut self) -> &mut SpatialIndex {
        &mut self.spatial
    }

    /// Returns the children of a node in render order (back to front), or `None`
    /// if the node doesn't exist.
    #[must_use]
    pub fn children(&self, id: NodeId) -> Option<&[NodeId]> {
        self.nodes.get(&id).map(|n| n.children.as_slice())
    }

    /// Returns the parent of a node, or `None` if the node doesn't exist or is a root.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(&id).and_then(|n| n.parent)
    }

    /// Returns the z-index (render order) of a node.
    /// Lower values are rendered first (behind higher values).
    /// Returns `None` if the node doesn't exist.
    #[must_use]
    pub fn z_index(&self, id: NodeId) -> Option<u32> {
        self.z_indices.get(&id).copied()
    }

    // --- Node lifecycle ---

    /// Adds a new node to the graph as a root node.
    ///
    /// The node starts with all dirty flags set. The spatial index is updated
    /// immediately. Returns the node's ID.
    ///
    /// If a node with the same ID already exists, it is replaced.
    pub fn add_root(&mut self, node: SceneNode) -> NodeId {
        let id = node.id;
        self.spatial.insert_node(id, &node.bounds);
        self.nodes.insert(id, node);
        self.roots.push(id);
        self.z_dirty = true;
        id
    }

    /// Adds a new node as a child of the given parent.
    ///
    /// The child is appended to the end of the parent's children list
    /// (rendered on top of existing children). The parent gets `DIRTY_CHILDREN`
    /// propagated up.
    ///
    /// Returns `true` if the parent was found and the child was added.
    /// Returns `false` if the parent doesn't exist (the child is NOT added).
    pub fn add_child(&mut self, parent_id: NodeId, mut child: SceneNode) -> bool {
        if !self.nodes.contains_key(&parent_id) {
            return false;
        }

        let child_id = child.id;
        child.parent = Some(parent_id);

        self.spatial.insert_node(child_id, &child.bounds);
        self.nodes.insert(child_id, child);

        // Add to parent's children list.
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }

        self.propagate_dirty_up(parent_id, DirtyFlags::CHILDREN);
        self.z_dirty = true;
        true
    }

    /// Removes a node and all its descendants from the graph.
    ///
    /// The node is removed from its parent's children list. All descendants
    /// are recursively removed. The spatial index is updated.
    ///
    /// Returns `true` if the node was found and removed.
    pub fn remove(&mut self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get(&id) else {
            return false;
        };

        let parent_id = node.parent;
        let children: Vec<NodeId> = node.children.clone();

        // Recursively remove descendants first.
        for child_id in children {
            self.remove(child_id);
        }

        // Remove from parent's children list.
        if let Some(pid) = parent_id {
            if let Some(parent) = self.nodes.get_mut(&pid) {
                parent.children.retain(|c| *c != id);
            }
            self.propagate_dirty_up(pid, DirtyFlags::CHILDREN);
        } else {
            // It's a root node.
            self.roots.retain(|r| *r != id);
        }

        // Remove node and spatial entry.
        self.nodes.remove(&id);
        self.spatial.remove(id);
        self.z_indices.remove(&id);
        self.z_dirty = true;
        true
    }

    /// Moves a node to be a child of a new parent.
    ///
    /// The node is removed from its current parent (or roots) and added as
    /// the last child of `new_parent_id`. Both the old and new parents get
    /// dirty propagation.
    ///
    /// Returns `false` if either the node or the new parent doesn't exist,
    /// or if the node would become its own ancestor (cycle detection).
    pub fn reparent(&mut self, node_id: NodeId, new_parent_id: NodeId) -> bool {
        // Validate both exist.
        if !self.nodes.contains_key(&node_id) || !self.nodes.contains_key(&new_parent_id) {
            return false;
        }

        // Prevent cycles: new_parent must not be a descendant of node_id.
        if self.is_ancestor_of(node_id, new_parent_id) {
            return false;
        }

        // Prevent no-op reparent to same parent.
        if self.parent(node_id) == Some(new_parent_id) {
            return true; // Already in the right place.
        }

        // Remove from old parent's children list (or roots).
        let old_parent_id = self.parent(node_id);
        if let Some(pid) = old_parent_id {
            if let Some(parent) = self.nodes.get_mut(&pid) {
                parent.children.retain(|c| *c != node_id);
            }
            self.propagate_dirty_up(pid, DirtyFlags::CHILDREN);
        } else {
            self.roots.retain(|r| *r != node_id);
        }

        // Set new parent on node.
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.parent = Some(new_parent_id);
        }

        // Add to new parent's children list.
        if let Some(parent) = self.nodes.get_mut(&new_parent_id) {
            parent.children.push(node_id);
        }

        self.propagate_dirty_up(new_parent_id, DirtyFlags::CHILDREN);
        self.z_dirty = true;
        true
    }

    /// Reorders the children of a node.
    ///
    /// `new_order` must contain exactly the same IDs as the current children
    /// (a permutation). Returns `false` if the node doesn't exist or if
    /// `new_order` doesn't match the current children set.
    pub fn reorder_children(&mut self, parent_id: NodeId, new_order: &[NodeId]) -> bool {
        let Some(node) = self.nodes.get(&parent_id) else {
            return false;
        };

        // Validate the new order is a permutation of current children.
        if new_order.len() != node.children.len() {
            return false;
        }
        let mut sorted_current = node.children.clone();
        sorted_current.sort();
        let mut sorted_new: Vec<NodeId> = new_order.to_vec();
        sorted_new.sort();
        if sorted_current != sorted_new {
            return false;
        }

        // Apply the new order.
        if let Some(node) = self.nodes.get_mut(&parent_id) {
            node.children = new_order.to_vec();
        }

        self.propagate_dirty_up(parent_id, DirtyFlags::CHILDREN);
        self.z_dirty = true;
        true
    }

    // --- Property mutations (auto-sync spatial index + dirty propagation) ---

    /// Updates a node's bounding box.
    ///
    /// Syncs the spatial index and marks the node with `DIRTY_GEOMETRY`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_bounds(&mut self, id: NodeId, bounds: BoundingBox) -> bool {
        let found = self.mutate_node(id, DirtyFlags::GEOMETRY, |node| {
            node.bounds = bounds;
            true
        });
        if found {
            self.spatial.update_node(id, &bounds);
        }
        found
    }

    /// Updates a node's fill color.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_fill(&mut self, id: NodeId, fill: Option<Color>) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.fill = fill;
            true
        })
    }

    /// Updates a node's stroke color.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_stroke(&mut self, id: NodeId, stroke: Option<Color>) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.stroke = stroke;
            true
        })
    }

    /// Updates a node's stroke width.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_stroke_width(&mut self, id: NodeId, width: f32) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.stroke_width = width;
            true
        })
    }

    /// Updates a node's opacity.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_opacity(&mut self, id: NodeId, opacity: f32) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.opacity = opacity.clamp(0.0, 1.0);
            true
        })
    }

    /// Updates a node's visibility.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_visible(&mut self, id: NodeId, visible: bool) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.visible = visible;
            true
        })
    }

    /// Updates a node's name.
    ///
    /// Does not set any dirty flags (name is not a visual property).
    /// Returns `false` if the node doesn't exist.
    pub fn set_name(&mut self, id: NodeId, name: String) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.name = name;
            true
        } else {
            false
        }
    }

    /// Updates the text content of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_text_content(&mut self, id: NodeId, content: String) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                content: ref mut c, ..
            } = node.kind
            {
                *c = content;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font size of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_size(&mut self, id: NodeId, font_size: f32) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_size: ref mut fs,
                ..
            } = node.kind
            {
                *fs = font_size;
                true
            } else {
                false
            }
        })
    }

    /// Updates the path data of a Vector node.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist or is not a Vector node.
    pub fn set_path_data(&mut self, id: NodeId, new_path_data: String) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            if let SceneNodeKind::Vector {
                path_data: ref mut pd,
            } = node.kind
            {
                *pd = new_path_data;
                true
            } else {
                false
            }
        })
    }

    /// Updates the asset reference of an Image node.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist or is not an Image node.
    pub fn set_asset_ref(&mut self, id: NodeId, new_asset_ref: String) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            if let SceneNodeKind::Image {
                asset_ref: ref mut ar,
            } = node.kind
            {
                *ar = new_asset_ref;
                true
            } else {
                false
            }
        })
    }

    // --- Render order ---

    /// Returns visible nodes for rendering, sorted in back-to-front render order.
    ///
    /// Queries the spatial index with the given viewport, then sorts results
    /// by pre-computed z-index. Only visible nodes are returned.
    ///
    /// This is the primary method used by the renderer each frame.
    #[must_use]
    pub fn visible_nodes_sorted(
        &mut self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) -> Vec<&SceneNode> {
        // Ensure z-indices are up to date.
        if self.z_dirty {
            self.recompute_z_indices();
        }

        // Reuse the internal buffer to avoid allocating a new Vec<NodeId> each frame.
        self.spatial
            .query_viewport_into(left, top, right, bottom, &mut self.visible_ids_buf);

        // Sort by z-index (lower = further back = rendered first).
        self.visible_ids_buf
            .sort_by_key(|id| self.z_indices.get(id).copied().unwrap_or(u32::MAX));

        // Filter to only visible nodes and collect references.
        self.visible_ids_buf
            .iter()
            .filter_map(|id| {
                let node = self.nodes.get(id)?;
                if node.visible { Some(node) } else { None }
            })
            .collect()
    }

    // --- Dirty management ---

    /// Clears dirty flags on all nodes.
    ///
    /// Call this after the renderer has processed all dirty nodes in a frame.
    pub fn clear_all_dirty(&mut self) {
        for node in self.nodes.values_mut() {
            node.clear_dirty();
        }
    }

    /// Returns an iterator over all nodes with non-zero dirty flags.
    pub fn dirty_nodes(&self) -> impl Iterator<Item = &SceneNode> {
        self.nodes.values().filter(|n| n.dirty.is_dirty())
    }

    // --- Tree queries ---

    /// Returns `true` if `ancestor_id` is an ancestor of `descendant_id`.
    ///
    /// A node is NOT considered an ancestor of itself.
    #[must_use]
    pub fn is_ancestor_of(&self, ancestor_id: NodeId, descendant_id: NodeId) -> bool {
        let mut current = descendant_id;
        while let Some(parent_id) = self.parent(current) {
            if parent_id == ancestor_id {
                return true;
            }
            current = parent_id;
        }
        false
    }

    /// Returns the depth of a node in the tree (0 for root nodes).
    #[must_use]
    pub fn depth(&self, id: NodeId) -> Option<u32> {
        if !self.contains(id) {
            return None;
        }
        let mut d = 0;
        let mut current = id;
        while let Some(parent_id) = self.parent(current) {
            d += 1;
            current = parent_id;
        }
        Some(d)
    }

    /// Returns all ancestor IDs from the node up to the root (exclusive of the node itself).
    #[must_use]
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut current = id;
        while let Some(parent_id) = self.parent(current) {
            result.push(parent_id);
            current = parent_id;
        }
        result
    }

    /// Returns all descendant IDs (depth-first, pre-order).
    #[must_use]
    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        self.collect_descendants(id, &mut result);
        result
    }

    /// Returns an iterator over all nodes in the graph.
    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &SceneNode)> {
        self.nodes.iter()
    }

    // --- Bulk operations ---

    /// Rebuilds the spatial index from all current nodes.
    ///
    /// Use after bulk modifications (e.g., file import) rather than
    /// relying on incremental updates.
    pub fn rebuild_spatial_index(&mut self) {
        use crate::spatial::SpatialIndex as SI;
        let entries: Vec<_> = self
            .nodes
            .values()
            .map(|n| crate::spatial::SpatialEntry::new(n.id, &n.bounds))
            .collect();
        self.spatial = SI::bulk_load(entries);
    }

    // --- Internal helpers ---

    /// Mutates a node's property via the closure, sets dirty flags, and propagates
    /// `DIRTY_CHILDREN` up to ancestors.
    ///
    /// The closure receives the node and returns `true` if the mutation was applied
    /// (e.g., returns `false` if a text-specific setter is called on a non-text node).
    /// Dirty flags and propagation only happen when the closure returns `true`.
    ///
    /// Returns `true` if the node was found and the closure returned `true`.
    fn mutate_node(
        &mut self,
        id: NodeId,
        flags: DirtyFlags,
        f: impl FnOnce(&mut SceneNode) -> bool,
    ) -> bool {
        let mutated = if let Some(node) = self.nodes.get_mut(&id) {
            if f(node) {
                node.dirty |= flags;
                true
            } else {
                false
            }
        } else {
            false
        };

        if mutated {
            self.propagate_dirty_up_from_child(id);
        }
        mutated
    }

    /// Propagates `DIRTY_CHILDREN` from a node up to its ancestors.
    fn propagate_dirty_up(&mut self, start_id: NodeId, flags: DirtyFlags) {
        let mut current = Some(start_id);
        while let Some(id) = current {
            if let Some(node) = self.nodes.get_mut(&id) {
                // If this flag is already set, ancestors already know — stop early.
                if node.dirty.contains(flags) {
                    break;
                }
                node.dirty |= flags;
                current = node.parent;
            } else {
                break;
            }
        }
    }

    /// Propagates `DIRTY_CHILDREN` from a child node's parent upward.
    fn propagate_dirty_up_from_child(&mut self, child_id: NodeId) {
        if let Some(parent_id) = self.parent(child_id) {
            self.propagate_dirty_up(parent_id, DirtyFlags::CHILDREN);
        }
    }

    /// Recursively collects descendant IDs in depth-first pre-order.
    fn collect_descendants(&self, id: NodeId, result: &mut Vec<NodeId>) {
        if let Some(node) = self.nodes.get(&id) {
            for &child_id in &node.children {
                result.push(child_id);
                self.collect_descendants(child_id, result);
            }
        }
    }

    /// Recomputes z-indices for all nodes via depth-first traversal.
    ///
    /// z-index 0 = furthest back (first root's first recursive child),
    /// increasing values = further forward.
    fn recompute_z_indices(&mut self) {
        self.z_indices.clear();
        let mut counter: u32 = 0;
        let roots = self.roots.clone();
        for root_id in &roots {
            self.assign_z_index(*root_id, &mut counter);
        }
        self.z_dirty = false;
    }

    /// Recursively assigns z-index in depth-first pre-order.
    fn assign_z_index(&mut self, id: NodeId, counter: &mut u32) {
        self.z_indices.insert(id, *counter);
        *counter += 1;

        // Clone children to avoid borrow conflict.
        let children = match self.nodes.get(&id) {
            Some(node) => node.children.clone(),
            None => return,
        };

        for child_id in children {
            self.assign_z_index(child_id, counter);
        }
    }
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SceneGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneGraph")
            .field("nodes", &self.nodes.len())
            .field("roots", &self.roots.len())
            .field("spatial_entries", &self.spatial.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::scene::SceneNodeKind;

    /// Helper: creates a simple frame node with the given name and bounds.
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

    /// Helper: creates a frame node with a specific ID.
    fn frame_node_with_id(id: NodeId, name: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        SceneNode::new(
            id,
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(x, y, w, h),
        )
    }

    // --- Construction ---

    #[test]
    fn new_graph_is_empty() {
        let graph = SceneGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
        assert!(graph.roots().is_empty());
    }

    // --- Adding nodes ---

    #[test]
    fn add_root_increases_count() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Root", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        assert_eq!(graph.len(), 1);
        assert_eq!(graph.roots(), &[id]);
        assert!(graph.contains(id));
    }

    #[test]
    fn add_root_syncs_spatial_index() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Root", 50.0, 50.0, 100.0, 100.0);
        let id = graph.add_root(node);

        let hits = graph.spatial().query_point(75.0, 75.0);
        assert_eq!(hits, vec![id]);
    }

    #[test]
    fn add_child_sets_parent() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        assert!(graph.add_child(parent_id, child));

        assert_eq!(graph.parent(child_id), Some(parent_id));
        assert_eq!(graph.children(parent_id), Some(&[child_id][..]));
        assert_eq!(graph.len(), 2);
    }

    #[test]
    fn add_child_to_nonexistent_parent_fails() {
        let mut graph = SceneGraph::new();
        let child = frame_node("Orphan", 0.0, 0.0, 50.0, 50.0);
        let fake_parent = NodeId::new();

        assert!(!graph.add_child(fake_parent, child));
        assert!(graph.is_empty());
    }

    #[test]
    fn add_child_propagates_dirty() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);
        graph.clear_all_dirty();

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        graph.add_child(parent_id, child);

        let parent_node = graph.get(parent_id).expect("parent should exist");
        assert!(
            parent_node.dirty.contains(DirtyFlags::CHILDREN),
            "Parent should have DIRTY_CHILDREN after child added"
        );
    }

    // --- Removing nodes ---

    #[test]
    fn remove_root() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Root", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        assert!(graph.remove(id));
        assert!(graph.is_empty());
        assert!(graph.roots().is_empty());
        assert!(graph.spatial().query_point(50.0, 50.0).is_empty());
    }

    #[test]
    fn remove_child() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);
        graph.clear_all_dirty();

        assert!(graph.remove(child_id));
        assert_eq!(graph.len(), 1);
        assert_eq!(graph.children(parent_id), Some(&[][..]));

        // Parent should be dirty.
        let parent_node = graph.get(parent_id).expect("parent should exist");
        assert!(parent_node.dirty.contains(DirtyFlags::CHILDREN));
    }

    #[test]
    fn remove_subtree() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let child = frame_node("Child", 10.0, 10.0, 200.0, 200.0);
        let child_id = child.id;
        graph.add_child(root_id, child);

        let grandchild = frame_node("Grandchild", 20.0, 20.0, 50.0, 50.0);
        let grandchild_id = grandchild.id;
        graph.add_child(child_id, grandchild);

        assert_eq!(graph.len(), 3);

        // Remove child — should also remove grandchild.
        assert!(graph.remove(child_id));
        assert_eq!(graph.len(), 1);
        assert!(!graph.contains(child_id));
        assert!(!graph.contains(grandchild_id));
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut graph = SceneGraph::new();
        assert!(!graph.remove(NodeId::new()));
    }

    // --- Reparenting ---

    #[test]
    fn reparent_moves_child() {
        let mut graph = SceneGraph::new();
        let first_parent = frame_node("ParentA", 0.0, 0.0, 200.0, 200.0);
        let first_parent_id = graph.add_root(first_parent);

        let second_parent = frame_node("ParentB", 300.0, 0.0, 200.0, 200.0);
        let second_parent_id = graph.add_root(second_parent);

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(first_parent_id, child);

        assert!(graph.reparent(child_id, second_parent_id));
        assert_eq!(graph.parent(child_id), Some(second_parent_id));
        assert_eq!(graph.children(first_parent_id), Some(&[][..]));
        assert_eq!(graph.children(second_parent_id), Some(&[child_id][..]));
    }

    #[test]
    fn reparent_prevents_cycle() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        // Trying to make parent a child of its own child should fail.
        assert!(!graph.reparent(parent_id, child_id));
    }

    #[test]
    fn reparent_same_parent_is_noop() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        assert!(graph.reparent(child_id, parent_id));
        assert_eq!(graph.children(parent_id), Some(&[child_id][..]));
    }

    #[test]
    fn reparent_nonexistent_node_fails() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        assert!(!graph.reparent(NodeId::new(), parent_id));
    }

    #[test]
    fn reparent_to_nonexistent_parent_fails() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let node_id = graph.add_root(node);

        assert!(!graph.reparent(node_id, NodeId::new()));
    }

    // --- Reorder children ---

    #[test]
    fn reorder_children_changes_order() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let c1 = frame_node("C1", 0.0, 0.0, 50.0, 50.0);
        let c1_id = c1.id;
        graph.add_child(parent_id, c1);

        let c2 = frame_node("C2", 60.0, 0.0, 50.0, 50.0);
        let c2_id = c2.id;
        graph.add_child(parent_id, c2);

        let c3 = frame_node("C3", 120.0, 0.0, 50.0, 50.0);
        let c3_id = c3.id;
        graph.add_child(parent_id, c3);

        // Reverse order.
        assert!(graph.reorder_children(parent_id, &[c3_id, c2_id, c1_id]));
        assert_eq!(graph.children(parent_id), Some(&[c3_id, c2_id, c1_id][..]));
    }

    #[test]
    fn reorder_children_rejects_wrong_ids() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let c1 = frame_node("C1", 0.0, 0.0, 50.0, 50.0);
        graph.add_child(parent_id, c1);

        // Wrong IDs.
        assert!(!graph.reorder_children(parent_id, &[NodeId::new()]));
    }

    #[test]
    fn reorder_children_rejects_wrong_length() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let c1 = frame_node("C1", 0.0, 0.0, 50.0, 50.0);
        graph.add_child(parent_id, c1);

        // Empty order for a parent with one child.
        assert!(!graph.reorder_children(parent_id, &[]));
    }

    // --- Property mutations ---

    #[test]
    fn set_bounds_updates_spatial_index() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 50.0, 50.0);
        let id = graph.add_root(node);

        // Move far away.
        assert!(graph.set_bounds(id, BoundingBox::new(1000.0, 1000.0, 50.0, 50.0)));

        // Old position should miss.
        assert!(graph.spatial().query_point(25.0, 25.0).is_empty());
        // New position should hit.
        assert_eq!(graph.spatial().query_point(1025.0, 1025.0), vec![id]);
    }

    #[test]
    fn set_bounds_marks_geometry_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 50.0, 50.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        graph.set_bounds(id, BoundingBox::new(10.0, 10.0, 50.0, 50.0));
        let node = graph.get(id).expect("node should exist");
        assert!(node.dirty.contains(DirtyFlags::GEOMETRY));
    }

    #[test]
    fn set_fill_marks_style_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 50.0, 50.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        graph.set_fill(id, Some(Color::new(1.0, 0.0, 0.0, 1.0)));
        let node = graph.get(id).expect("node should exist");
        assert!(node.dirty.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn set_opacity_clamps() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 50.0, 50.0);
        let id = graph.add_root(node);

        graph.set_opacity(id, 2.0);
        assert!((graph.get(id).expect("exists").opacity - 1.0).abs() < f32::EPSILON);

        graph.set_opacity(id, -1.0);
        assert!(graph.get(id).expect("exists").opacity.abs() < f32::EPSILON);
    }

    #[test]
    fn set_name_no_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Old", 0.0, 0.0, 50.0, 50.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        graph.set_name(id, "New".to_string());
        let node = graph.get(id).expect("exists");
        assert_eq!(node.name, "New");
        assert!(node.dirty.is_clean());
    }

    #[test]
    fn mutation_on_nonexistent_returns_false() {
        let mut graph = SceneGraph::new();
        let fake = NodeId::new();
        assert!(!graph.set_bounds(fake, BoundingBox::new(0.0, 0.0, 10.0, 10.0)));
        assert!(!graph.set_fill(fake, None));
        assert!(!graph.set_stroke(fake, None));
        assert!(!graph.set_stroke_width(fake, 1.0));
        assert!(!graph.set_opacity(fake, 1.0));
        assert!(!graph.set_visible(fake, true));
        assert!(!graph.set_name(fake, "x".to_string()));
        assert!(!graph.set_text_content(fake, "hello".to_string()));
        assert!(!graph.set_font_size(fake, 16.0));
    }

    // --- Text node mutations ---

    #[test]
    fn set_text_content_updates_text_node() {
        let mut graph = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 16.0,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 30.0),
        );
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_text_content(id, "World".to_string()));

        let node = graph.get(id).expect("node should exist");
        if let SceneNodeKind::Text { content, .. } = &node.kind {
            assert_eq!(content, "World");
        } else {
            panic!("expected Text node");
        }
        assert!(node.dirty.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn set_text_content_on_frame_returns_false() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Rect", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(!graph.set_text_content(id, "nope".to_string()));
        assert!(
            graph.get(id).expect("exists").dirty.is_clean(),
            "non-text node should not be dirtied"
        );
    }

    #[test]
    fn set_text_content_propagates_dirty() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 400.0, 400.0);
        let parent_id = graph.add_root(parent);

        let text_node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Old".to_string(),
                font_size: 14.0,
            },
            BoundingBox::new(10.0, 10.0, 100.0, 20.0),
        );
        let text_id = text_node.id;
        graph.add_child(parent_id, text_node);
        graph.clear_all_dirty();

        graph.set_text_content(text_id, "New".to_string());

        assert!(
            graph
                .get(parent_id)
                .expect("parent")
                .dirty
                .contains(DirtyFlags::CHILDREN),
            "parent should have CHILDREN dirty after child text mutation"
        );
    }

    #[test]
    fn set_font_size_updates_text_node() {
        let mut graph = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 16.0,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 30.0),
        );
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_font_size(id, 24.0));

        let node = graph.get(id).expect("node should exist");
        if let SceneNodeKind::Text { font_size, .. } = &node.kind {
            assert!((*font_size - 24.0).abs() < f32::EPSILON);
        } else {
            panic!("expected Text node");
        }
        assert!(node.dirty.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn set_font_size_on_frame_returns_false() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Rect", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(!graph.set_font_size(id, 24.0));
        assert!(
            graph.get(id).expect("exists").dirty.is_clean(),
            "non-text node should not be dirtied"
        );
    }

    // --- Image/Vector node mutations ---

    #[test]
    fn set_path_data_updates_vector_node() {
        let mut graph = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Icon".to_string(),
            SceneNodeKind::Vector {
                path_data: "M 0 0 L 10 10".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 24.0, 24.0),
        );
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_path_data(id, "M 0 0 L 20 20".to_string()));

        let node = graph.get(id).expect("node should exist");
        if let SceneNodeKind::Vector { path_data } = &node.kind {
            assert_eq!(path_data, "M 0 0 L 20 20");
        } else {
            panic!("expected Vector node");
        }
        assert!(node.dirty.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn set_path_data_on_frame_returns_false() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Rect", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(!graph.set_path_data(id, "M 0 0 L 10 10".to_string()));
        assert!(graph.get(id).expect("exists").dirty.is_clean());
    }

    #[test]
    fn set_path_data_propagates_dirty() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 400.0, 400.0);
        let parent_id = graph.add_root(parent);

        let vector_node = SceneNode::new(
            NodeId::new(),
            "Icon".to_string(),
            SceneNodeKind::Vector {
                path_data: "M 0 0 L 10 10".to_string(),
            },
            BoundingBox::new(10.0, 10.0, 24.0, 24.0),
        );
        let vector_id = vector_node.id;
        graph.add_child(parent_id, vector_node);
        graph.clear_all_dirty();

        graph.set_path_data(vector_id, "M 0 0 L 20 20".to_string());

        assert!(
            graph
                .get(parent_id)
                .expect("parent")
                .dirty
                .contains(DirtyFlags::CHILDREN),
        );
    }

    #[test]
    fn set_asset_ref_updates_image_node() {
        let mut graph = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Photo".to_string(),
            SceneNodeKind::Image {
                asset_ref: "old.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_asset_ref(id, "new.png".to_string()));

        let node = graph.get(id).expect("node should exist");
        if let SceneNodeKind::Image { asset_ref } = &node.kind {
            assert_eq!(asset_ref, "new.png");
        } else {
            panic!("expected Image node");
        }
        assert!(node.dirty.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn set_asset_ref_on_frame_returns_false() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Rect", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(!graph.set_asset_ref(id, "img.png".to_string()));
        assert!(graph.get(id).expect("exists").dirty.is_clean());
    }

    #[test]
    fn set_asset_ref_propagates_dirty() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 400.0, 400.0);
        let parent_id = graph.add_root(parent);

        let image_node = SceneNode::new(
            NodeId::new(),
            "Photo".to_string(),
            SceneNodeKind::Image {
                asset_ref: "old.png".to_string(),
            },
            BoundingBox::new(10.0, 10.0, 100.0, 100.0),
        );
        let image_id = image_node.id;
        graph.add_child(parent_id, image_node);
        graph.clear_all_dirty();

        graph.set_asset_ref(image_id, "new.png".to_string());

        assert!(
            graph
                .get(parent_id)
                .expect("parent")
                .dirty
                .contains(DirtyFlags::CHILDREN),
        );
    }

    #[test]
    fn set_font_size_propagates_dirty() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 400.0, 400.0);
        let parent_id = graph.add_root(parent);

        let text_node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 14.0,
            },
            BoundingBox::new(10.0, 10.0, 100.0, 20.0),
        );
        let text_id = text_node.id;
        graph.add_child(parent_id, text_node);
        graph.clear_all_dirty();

        graph.set_font_size(text_id, 32.0);

        assert!(
            graph
                .get(parent_id)
                .expect("parent")
                .dirty
                .contains(DirtyFlags::CHILDREN),
            "parent should have CHILDREN dirty after child font_size mutation"
        );
    }

    // --- Dirty propagation ---

    #[test]
    fn dirty_propagates_to_root() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let child = frame_node("Child", 10.0, 10.0, 200.0, 200.0);
        let child_id = child.id;
        graph.add_child(root_id, child);

        let grandchild = frame_node("Grandchild", 20.0, 20.0, 50.0, 50.0);
        let gc_id = grandchild.id;
        graph.add_child(child_id, grandchild);

        graph.clear_all_dirty();

        // Mutate the grandchild.
        graph.set_fill(gc_id, Some(Color::new(1.0, 0.0, 0.0, 1.0)));

        // Grandchild has STYLE.
        assert!(
            graph
                .get(gc_id)
                .expect("gc")
                .dirty
                .contains(DirtyFlags::STYLE)
        );
        // Child and root should have CHILDREN.
        assert!(
            graph
                .get(child_id)
                .expect("child")
                .dirty
                .contains(DirtyFlags::CHILDREN)
        );
        assert!(
            graph
                .get(root_id)
                .expect("root")
                .dirty
                .contains(DirtyFlags::CHILDREN)
        );
    }

    #[test]
    fn dirty_propagation_stops_early_if_already_set() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let child = frame_node("Child", 10.0, 10.0, 200.0, 200.0);
        let child_id = child.id;
        graph.add_child(root_id, child);

        graph.clear_all_dirty();

        // First mutation propagates to root.
        graph.set_fill(child_id, Some(Color::BLACK));
        assert!(
            graph
                .get(root_id)
                .expect("root")
                .dirty
                .contains(DirtyFlags::CHILDREN)
        );

        // Second mutation on same child — root already has CHILDREN,
        // propagation should stop early (no panic, no infinite loop).
        graph.set_stroke(child_id, Some(Color::WHITE));
        // Still works fine.
        assert!(
            graph
                .get(root_id)
                .expect("root")
                .dirty
                .contains(DirtyFlags::CHILDREN)
        );
    }

    // --- Z-index and render order ---

    #[test]
    fn z_indices_follow_dfs_order() {
        let mut graph = SceneGraph::new();
        let r1 = frame_node("R1", 0.0, 0.0, 100.0, 100.0);
        let r1_id = graph.add_root(r1);

        let c1 = frame_node("C1", 10.0, 10.0, 30.0, 30.0);
        let c1_id = c1.id;
        graph.add_child(r1_id, c1);

        let c2 = frame_node("C2", 50.0, 10.0, 30.0, 30.0);
        let c2_id = c2.id;
        graph.add_child(r1_id, c2);

        let r2 = frame_node("R2", 200.0, 0.0, 100.0, 100.0);
        let r2_id = graph.add_root(r2);

        // Force z-index computation.
        let _ = graph.visible_nodes_sorted(-1000.0, -1000.0, 2000.0, 2000.0);

        // DFS order: R1(0), C1(1), C2(2), R2(3).
        assert_eq!(graph.z_index(r1_id), Some(0));
        assert_eq!(graph.z_index(c1_id), Some(1));
        assert_eq!(graph.z_index(c2_id), Some(2));
        assert_eq!(graph.z_index(r2_id), Some(3));
    }

    #[test]
    fn visible_nodes_sorted_returns_back_to_front() {
        let mut graph = SceneGraph::new();
        graph.spatial_mut().set_overscan(0.0);

        // All nodes overlap so spatial query returns all.
        let back = frame_node("Back", 0.0, 0.0, 100.0, 100.0);
        let back_id = graph.add_root(back);

        let front = frame_node("Front", 0.0, 0.0, 100.0, 100.0);
        let front_id = graph.add_root(front);

        let sorted = graph.visible_nodes_sorted(0.0, 0.0, 100.0, 100.0);
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].id, back_id); // Lower z-index first.
        assert_eq!(sorted[1].id, front_id);
    }

    #[test]
    fn visible_nodes_excludes_invisible() {
        let mut graph = SceneGraph::new();
        graph.spatial_mut().set_overscan(0.0);

        let visible = frame_node("Visible", 0.0, 0.0, 100.0, 100.0);
        let _visible_id = graph.add_root(visible);

        let mut hidden = frame_node("Hidden", 0.0, 0.0, 100.0, 100.0);
        hidden.visible = false;
        let hidden_id = hidden.id;
        graph.add_root(hidden);

        let sorted = graph.visible_nodes_sorted(0.0, 0.0, 100.0, 100.0);
        assert_eq!(sorted.len(), 1);
        assert_ne!(sorted[0].id, hidden_id);
    }

    // --- Tree queries ---

    #[test]
    fn is_ancestor_of_direct_parent() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        assert!(graph.is_ancestor_of(parent_id, child_id));
        assert!(!graph.is_ancestor_of(child_id, parent_id));
    }

    #[test]
    fn is_ancestor_of_grandparent() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let child = frame_node("Child", 10.0, 10.0, 200.0, 200.0);
        let child_id = child.id;
        graph.add_child(root_id, child);

        let gc = frame_node("GC", 20.0, 20.0, 50.0, 50.0);
        let gc_id = gc.id;
        graph.add_child(child_id, gc);

        assert!(graph.is_ancestor_of(root_id, gc_id));
    }

    #[test]
    fn is_ancestor_of_self_is_false() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        assert!(!graph.is_ancestor_of(id, id));
    }

    #[test]
    fn depth_of_root_is_zero() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Root", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        assert_eq!(graph.depth(id), Some(0));
    }

    #[test]
    fn depth_increases_with_nesting() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let child = frame_node("Child", 10.0, 10.0, 200.0, 200.0);
        let child_id = child.id;
        graph.add_child(root_id, child);

        let gc = frame_node("GC", 20.0, 20.0, 50.0, 50.0);
        let gc_id = gc.id;
        graph.add_child(child_id, gc);

        assert_eq!(graph.depth(root_id), Some(0));
        assert_eq!(graph.depth(child_id), Some(1));
        assert_eq!(graph.depth(gc_id), Some(2));
    }

    #[test]
    fn ancestors_returns_path_to_root() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let child = frame_node("Child", 10.0, 10.0, 200.0, 200.0);
        let child_id = child.id;
        graph.add_child(root_id, child);

        let gc = frame_node("GC", 20.0, 20.0, 50.0, 50.0);
        let gc_id = gc.id;
        graph.add_child(child_id, gc);

        assert_eq!(graph.ancestors(gc_id), vec![child_id, root_id]);
        assert_eq!(graph.ancestors(child_id), vec![root_id]);
        assert!(graph.ancestors(root_id).is_empty());
    }

    #[test]
    fn descendants_returns_subtree() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let c1 = frame_node("C1", 10.0, 10.0, 100.0, 100.0);
        let c1_id = c1.id;
        graph.add_child(root_id, c1);

        let c2 = frame_node("C2", 120.0, 10.0, 100.0, 100.0);
        let c2_id = c2.id;
        graph.add_child(root_id, c2);

        let gc = frame_node("GC", 20.0, 20.0, 30.0, 30.0);
        let gc_id = gc.id;
        graph.add_child(c1_id, gc);

        let descs = graph.descendants(root_id);
        assert_eq!(descs.len(), 3);
        assert_eq!(descs, vec![c1_id, gc_id, c2_id]); // DFS pre-order.
    }

    // --- Bulk operations ---

    #[test]
    fn rebuild_spatial_index_consistency() {
        let mut graph = SceneGraph::new();
        for i in 0..100 {
            #[allow(clippy::cast_precision_loss)]
            let x = (i as f32) * 20.0;
            let node = frame_node(&format!("N{i}"), x, 0.0, 15.0, 15.0);
            graph.add_root(node);
        }

        graph.rebuild_spatial_index();

        // Spatial index should match node count.
        assert_eq!(graph.spatial().len(), 100);

        // Query should still work.
        let region = graph.spatial().query_region(0.0, 0.0, 100.0, 100.0);
        assert!(
            !region.is_empty(),
            "Spatial query should find nodes after rebuild"
        );
    }

    // --- Clear dirty ---

    #[test]
    fn clear_all_dirty_resets_all_nodes() {
        let mut graph = SceneGraph::new();
        let n1 = frame_node("N1", 0.0, 0.0, 50.0, 50.0);
        let n1_id = graph.add_root(n1);
        let n2 = frame_node("N2", 60.0, 0.0, 50.0, 50.0);
        let n2_id = graph.add_root(n2);

        // Both start dirty (ALL flags).
        assert!(graph.get(n1_id).expect("n1").dirty.is_dirty());

        graph.clear_all_dirty();

        assert!(graph.get(n1_id).expect("n1").dirty.is_clean());
        assert!(graph.get(n2_id).expect("n2").dirty.is_clean());
    }

    #[test]
    fn dirty_nodes_iterator() {
        let mut graph = SceneGraph::new();
        let n1 = frame_node("N1", 0.0, 0.0, 50.0, 50.0);
        let _n1_id = graph.add_root(n1);
        let n2 = frame_node("N2", 60.0, 0.0, 50.0, 50.0);
        let _n2_id = graph.add_root(n2);

        // Both are dirty (newly created).
        assert_eq!(graph.dirty_nodes().count(), 2);

        graph.clear_all_dirty();
        assert_eq!(graph.dirty_nodes().count(), 0);
    }

    // --- Debug ---

    #[test]
    fn debug_format() {
        let graph = SceneGraph::new();
        let debug = format!("{graph:?}");
        assert!(debug.contains("SceneGraph"));
        assert!(debug.contains("nodes"));
    }

    // --- Proptest: SceneGraph + SpatialIndex consistency ---

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_bounds() -> impl Strategy<Value = BoundingBox> {
            (0.0_f32..1000.0, 0.0_f32..1000.0, 1.0_f32..200.0, 1.0_f32..200.0)
                .prop_map(|(x, y, w, h)| BoundingBox::new(x, y, w, h))
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(50))]

            #[test]
            fn spatial_index_matches_node_count(count in 1_usize..200) {
                let mut graph = SceneGraph::new();
                for i in 0..count {
                    #[allow(clippy::cast_precision_loss)]
                    let x = (i as f32) * 10.0;
                    let node = frame_node(&format!("N{i}"), x, 0.0, 8.0, 8.0);
                    graph.add_root(node);
                }
                prop_assert_eq!(graph.spatial().len(), count);
            }

            #[test]
            fn node_always_findable_at_its_own_center(bounds in arb_bounds()) {
                let mut graph = SceneGraph::new();
                let node = SceneNode::new(
                    NodeId::new(),
                    "N".to_string(),
                    SceneNodeKind::Frame { corner_radius: [0.0; 4] },
                    bounds,
                );
                let id = graph.add_root(node);

                let cx = bounds.x + bounds.width / 2.0;
                let cy = bounds.y + bounds.height / 2.0;
                let hits = graph.spatial().query_point(cx, cy);
                prop_assert!(hits.contains(&id), "node not found at its own center");
            }

            #[test]
            fn set_bounds_keeps_spatial_index_consistent(
                initial in arb_bounds(),
                updated in arb_bounds(),
            ) {
                let mut graph = SceneGraph::new();
                let node = SceneNode::new(
                    NodeId::new(),
                    "N".to_string(),
                    SceneNodeKind::Frame { corner_radius: [0.0; 4] },
                    initial,
                );
                let id = graph.add_root(node);

                graph.set_bounds(id, updated);

                let cx = updated.x + updated.width / 2.0;
                let cy = updated.y + updated.height / 2.0;
                let hits = graph.spatial().query_point(cx, cy);
                prop_assert!(hits.contains(&id), "node not found after set_bounds");
            }

            #[test]
            fn remove_removes_from_spatial_index(bounds in arb_bounds()) {
                let mut graph = SceneGraph::new();
                let node = SceneNode::new(
                    NodeId::new(),
                    "N".to_string(),
                    SceneNodeKind::Frame { corner_radius: [0.0; 4] },
                    bounds,
                );
                let id = graph.add_root(node);
                graph.remove(id);

                prop_assert_eq!(graph.spatial().len(), 0);
            }
        }
    }

    // --- Edge case: duplicate add_root replaces ---

    #[test]
    fn add_root_with_same_id_replaces() {
        let mut graph = SceneGraph::new();
        let id = NodeId::new();

        let node1 = frame_node_with_id(id, "First", 0.0, 0.0, 50.0, 50.0);
        graph.add_root(node1);

        let node2 = frame_node_with_id(id, "Second", 100.0, 100.0, 50.0, 50.0);
        graph.add_root(node2);

        // Should have the second node's name.
        assert_eq!(graph.get(id).expect("exists").name, "Second");
        // Note: roots will have the ID twice. This is acceptable for now;
        // production code should check for duplicates.
    }
}
