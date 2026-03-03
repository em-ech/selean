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

use super::clip::ClipMode;
use super::dirty::DirtyFlags;
use super::node::{
    BlendMode, BoundingBox, Color, Effect, FontStyle, Gradient, SceneNode, SceneNodeKind, TextAlign,
};
use super::transform::Transform2D;
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
    /// Whether any node has a dirty `TRANSFORM` flag (gates world transform recomputation).
    has_any_transform_dirty: bool,
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
            has_any_transform_dirty: false,
            visible_ids_buf: Vec::new(),
        }
    }

    /// Reconstructs a scene graph from deserialized document state.
    ///
    /// Takes ownership of the node map and root list. After construction,
    /// rebuilds the spatial index from scratch and recomputes all world
    /// transforms so the graph is ready for rendering.
    #[must_use]
    pub fn from_document_state(nodes: HashMap<NodeId, SceneNode>, roots: Vec<NodeId>) -> Self {
        let mut graph = Self {
            nodes,
            spatial: SpatialIndex::new(),
            roots,
            z_indices: HashMap::new(),
            z_dirty: true,
            has_any_transform_dirty: true,
            visible_ids_buf: Vec::new(),
        };
        graph.rebuild_spatial_index();
        graph.recompute_world_transforms();
        graph
    }

    // --- Accessors ---

    /// Returns a reference to the internal node map.
    #[must_use]
    pub fn nodes(&self) -> &HashMap<NodeId, SceneNode> {
        &self.nodes
    }

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
        // Root nodes have world_transform == local_transform (no parent).
        let world_aabb = node.world_transform.transform_aabb(&node.bounds);
        self.spatial.insert_node(id, &world_aabb);
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

        // Insert with identity AABB for now; propagate_transform_dirty_down
        // will cause recompute_world_transforms to fix it.
        self.spatial.insert_node(child_id, &child.bounds);
        self.nodes.insert(child_id, child);

        // Add to parent's children list.
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }

        // The child's world transform depends on the new parent.
        self.propagate_transform_dirty_down(child_id);
        self.propagate_dirty_up(parent_id, DirtyFlags::CHILDREN);
        self.z_dirty = true;
        true
    }

    /// Adds a new root node at a specific index in the roots list.
    ///
    /// Same as [`add_root`] but inserts at `index` instead of appending.
    /// The index is clamped to `roots.len()`.
    pub fn insert_root_at(&mut self, node: SceneNode, index: usize) -> NodeId {
        let id = node.id;
        let world_aabb = node.world_transform.transform_aabb(&node.bounds);
        self.spatial.insert_node(id, &world_aabb);
        self.nodes.insert(id, node);
        let clamped = index.min(self.roots.len());
        self.roots.insert(clamped, id);
        self.z_dirty = true;
        id
    }

    /// Adds a child node at a specific index in the parent's children list.
    ///
    /// Same as [`add_child`] but inserts at `index` instead of appending.
    /// The index is clamped to `children.len()`.
    /// Returns `false` if the parent doesn't exist.
    pub fn insert_child_at(
        &mut self,
        parent_id: NodeId,
        mut child: SceneNode,
        index: usize,
    ) -> bool {
        if !self.nodes.contains_key(&parent_id) {
            return false;
        }

        let child_id = child.id;
        child.parent = Some(parent_id);

        self.spatial.insert_node(child_id, &child.bounds);
        self.nodes.insert(child_id, child);

        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            let clamped = index.min(parent.children.len());
            parent.children.insert(clamped, child_id);
        }

        self.propagate_transform_dirty_down(child_id);
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

        // New parent means new world transform for the reparented node and its descendants.
        self.propagate_transform_dirty_down(node_id);
        self.propagate_dirty_up(new_parent_id, DirtyFlags::CHILDREN);
        self.z_dirty = true;
        true
    }

    /// Moves a child node to be a root at a specific index.
    ///
    /// Detaches the node from its current parent and inserts it into the roots
    /// list at `index` (clamped to `roots.len()`). The node keeps its children.
    /// Returns `false` if the node doesn't exist or is already a root.
    pub fn reparent_to_root(&mut self, node_id: NodeId, index: usize) -> bool {
        let Some(node) = self.nodes.get(&node_id) else {
            return false;
        };

        let Some(old_parent_id) = node.parent else {
            return false; // Already a root.
        };

        // Remove from old parent's children list.
        if let Some(parent) = self.nodes.get_mut(&old_parent_id) {
            parent.children.retain(|c| *c != node_id);
        }
        self.propagate_dirty_up(old_parent_id, DirtyFlags::CHILDREN);

        // Clear parent on node.
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.parent = None;
        }

        // Insert into roots at index.
        let clamped = index.min(self.roots.len());
        self.roots.insert(clamped, node_id);

        self.propagate_transform_dirty_down(node_id);
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

    /// Reorders root nodes. `new_order` must be a permutation of the current roots.
    ///
    /// Returns `false` if the new order is not a valid permutation.
    pub fn reorder_roots(&mut self, new_order: &[NodeId]) -> bool {
        if new_order.len() != self.roots.len() {
            return false;
        }
        let mut sorted_current = self.roots.clone();
        sorted_current.sort();
        let mut sorted_new: Vec<NodeId> = new_order.to_vec();
        sorted_new.sort();
        if sorted_current != sorted_new {
            return false;
        }
        self.roots = new_order.to_vec();
        self.z_dirty = true;
        true
    }

    // --- Property mutations (auto-sync spatial index + dirty propagation) ---

    /// Updates a node's bounding box.
    ///
    /// Syncs the spatial index (using the cached world transform for AABB) and
    /// marks the node with `DIRTY_GEOMETRY`. Returns `false` if the node doesn't exist.
    pub fn set_bounds(&mut self, id: NodeId, bounds: BoundingBox) -> bool {
        let found = self.mutate_node(id, DirtyFlags::GEOMETRY, |node| {
            node.bounds = bounds;
            true
        });
        if found {
            // Use the cached world transform to compute the spatial AABB.
            // If the transform is also dirty this frame, recompute_world_transforms()
            // will correct the spatial AABB during the prepare phase.
            if let Some(node) = self.nodes.get(&id) {
                let world_aabb = node.world_transform.transform_aabb(&bounds);
                self.spatial.update_node(id, &world_aabb);
            }
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

    /// Updates a node's gradient fill.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_fill_gradient(&mut self, id: NodeId, fill_gradient: Option<Gradient>) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.fill_gradient = fill_gradient;
            true
        })
    }

    /// Updates a node's visual effects list.
    ///
    /// Marks the node with `DIRTY_EFFECTS`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_effects(&mut self, id: NodeId, effects: Vec<Effect>) -> bool {
        self.mutate_node(id, DirtyFlags::EFFECTS, |node| {
            node.effects = effects;
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

    /// Updates the corner radius of a Frame node.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist or is not a Frame node.
    pub fn set_corner_radius(&mut self, id: NodeId, corner_radius: [f32; 4]) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            if let SceneNodeKind::Frame {
                corner_radius: ref mut cr,
            } = node.kind
            {
                *cr = corner_radius;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font family of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_family(&mut self, id: NodeId, font_family: String) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_family: ref mut ff,
                ..
            } = node.kind
            {
                *ff = font_family;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font weight of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_weight(&mut self, id: NodeId, font_weight: u16) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_weight: ref mut fw,
                ..
            } = node.kind
            {
                *fw = font_weight;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font style of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_style(&mut self, id: NodeId, font_style: FontStyle) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_style: ref mut fs,
                ..
            } = node.kind
            {
                *fs = font_style;
                true
            } else {
                false
            }
        })
    }

    /// Updates the text alignment of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_text_align(&mut self, id: NodeId, text_align: TextAlign) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                text_align: ref mut ta,
                ..
            } = node.kind
            {
                *ta = text_align;
                true
            } else {
                false
            }
        })
    }

    /// Updates the line height of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_line_height(&mut self, id: NodeId, line_height: f32) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                line_height: ref mut lh,
                ..
            } = node.kind
            {
                *lh = line_height;
                true
            } else {
                false
            }
        })
    }

    /// Updates the text color of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_text_color(&mut self, id: NodeId, text_color: Option<Color>) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                text_color: ref mut tc,
                ..
            } = node.kind
            {
                *tc = text_color;
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

    // --- Transform mutations ---

    /// Sets the local transform for a node.
    ///
    /// Marks the node and all descendants with `TRANSFORM` dirty.
    /// The world transform is recomputed lazily during the prepare phase.
    /// Returns `false` if the node doesn't exist.
    pub fn set_transform(&mut self, id: NodeId, transform: Transform2D) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.local_transform = transform;
            node.dirty |= DirtyFlags::TRANSFORM;
        } else {
            return false;
        }

        self.propagate_transform_dirty_down(id);
        self.propagate_dirty_up_from_child(id);
        true
    }

    /// Sets the local transform to a pure rotation around the node's center.
    ///
    /// This **replaces** the entire local transform (it does not compose with
    /// any existing scale or skew). Use [`set_transform`] for composed transforms.
    ///
    /// Returns `false` if the node doesn't exist.
    pub fn set_rotation(&mut self, id: NodeId, angle_rad: f32) -> bool {
        let Some(node) = self.nodes.get(&id) else {
            return false;
        };
        let cx = node.bounds.x + node.bounds.width * 0.5;
        let cy = node.bounds.y + node.bounds.height * 0.5;
        let transform = Transform2D::from_rotation_around(angle_rad, cx, cy);
        self.set_transform(id, transform)
    }

    /// Sets the local transform to a pure scale around the node's center.
    ///
    /// This **replaces** the entire local transform (it does not compose with
    /// any existing rotation or skew). Use [`set_transform`] for composed transforms.
    ///
    /// Returns `false` if the node doesn't exist.
    pub fn set_scale(&mut self, id: NodeId, sx: f32, sy: f32) -> bool {
        let Some(node) = self.nodes.get(&id) else {
            return false;
        };
        let cx = node.bounds.x + node.bounds.width * 0.5;
        let cy = node.bounds.y + node.bounds.height * 0.5;
        let transform = Transform2D::from_scale_around(sx, sy, cx, cy);
        self.set_transform(id, transform)
    }

    /// Sets the blend mode for a node.
    ///
    /// Marks the node with `STYLE` dirty.
    /// Returns `false` if the node doesn't exist.
    pub fn set_blend_mode(&mut self, id: NodeId, mode: BlendMode) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.blend_mode = mode;
            true
        })
    }

    /// Sets the clip mode for a node.
    ///
    /// When set to a mode other than `None`, all descendants are clipped to
    /// this node's bounds. Marks the node with `CLIP` dirty and propagates
    /// `CHILDREN` up to ancestors.
    /// Returns `false` if the node doesn't exist.
    pub fn set_clip_mode(&mut self, id: NodeId, mode: ClipMode) -> bool {
        self.mutate_node(id, DirtyFlags::CLIP, |node| {
            node.clip_mode = mode;
            true
        })
    }

    // --- Scroll offset ---

    /// Sets the scroll offset for a node.
    ///
    /// Children of this node will be translated by `(-x, -y)` in the node's
    /// coordinate space. The node itself renders at its normal position.
    /// Marks the node and all descendants with `TRANSFORM` dirty (same as
    /// `set_transform`) and propagates `CHILDREN` up.
    /// Returns `false` if the node does not exist.
    pub fn set_scroll_offset(&mut self, id: NodeId, x: f32, y: f32) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.scroll_offset = [x, y];
            node.dirty |= DirtyFlags::TRANSFORM;
        } else {
            return false;
        }

        self.propagate_transform_dirty_down(id);
        self.propagate_dirty_up_from_child(id);
        true
    }

    /// Returns the scroll offset for a node, or `None` if the node does not exist.
    #[must_use]
    pub fn scroll_offset(&self, id: NodeId) -> Option<[f32; 2]> {
        self.nodes.get(&id).map(|n| n.scroll_offset)
    }

    /// Computes the bounding box enclosing all direct children of a node.
    ///
    /// Each child's bounds are transformed by its `local_transform` before
    /// being unioned. Returns `None` if the node does not exist. Returns an
    /// empty bounding box if the node has no children.
    #[must_use]
    pub fn compute_content_bounds(&self, id: NodeId) -> Option<BoundingBox> {
        let node = self.nodes.get(&id)?;
        let mut result = BoundingBox::new(0.0, 0.0, 0.0, 0.0);
        for &child_id in &node.children {
            if let Some(child) = self.nodes.get(&child_id) {
                let transformed = child.local_transform.transform_aabb(&child.bounds);
                result = result.union(&transformed);
            }
        }
        Some(result)
    }

    /// Returns the maximum scroll offset for a node.
    ///
    /// This is `[max(0, content_right - node_width), max(0, content_bottom - node_height)]`.
    /// Content smaller than the container yields `[0.0, 0.0]`.
    /// Returns `None` if the node does not exist.
    #[must_use]
    pub fn max_scroll(&self, id: NodeId) -> Option<[f32; 2]> {
        let node = self.nodes.get(&id)?;
        let content = self.compute_content_bounds(id)?;
        let max_x = (content.right() - node.bounds.width).max(0.0);
        let max_y = (content.bottom() - node.bounds.height).max(0.0);
        Some([max_x, max_y])
    }

    /// Sets the scroll offset, clamped to `[0..max_x, 0..max_y]`.
    ///
    /// Computes `max_scroll` and clamps the requested offset. Negative values
    /// are clamped to zero. Returns `false` if the node does not exist.
    pub fn set_scroll_offset_clamped(&mut self, id: NodeId, x: f32, y: f32) -> bool {
        let Some(max) = self.max_scroll(id) else {
            return false;
        };
        let clamped_x = x.clamp(0.0, max[0]);
        let clamped_y = y.clamp(0.0, max[1]);
        self.set_scroll_offset(id, clamped_x, clamped_y)
    }

    // --- World transform recomputation ---

    /// Recomputes world transforms for all dirty nodes.
    ///
    /// Walks the full tree (DFS from all roots), only recomputing nodes that
    /// have the `TRANSFORM` dirty flag set. Gated by `has_any_transform_dirty`
    /// so zero-cost on frames with no transform changes.
    ///
    /// After recomputation, the spatial index AABBs are updated to reflect
    /// the new world-space bounds. Call this at the start of the prepare phase.
    pub fn recompute_world_transforms(&mut self) {
        if !self.has_any_transform_dirty {
            return;
        }

        let roots: Vec<NodeId> = self.roots.clone();
        for &root_id in &roots {
            self.recompute_world_transform_recursive(root_id, Transform2D::identity());
        }

        self.has_any_transform_dirty = false;
    }

    // --- Hit testing ---

    /// Returns the ancestor clip chain for a node: all ancestors (from immediate
    /// parent up to root) that have a clip mode other than `None`.
    ///
    /// The returned vec is ordered from nearest ancestor to most distant.
    #[must_use]
    pub fn ancestor_clip_chain(&self, id: NodeId) -> Vec<(NodeId, ClipMode)> {
        let mut result = Vec::new();
        let mut current = id;
        while let Some(parent_id) = self.parent(current) {
            if let Some(node) = self.nodes.get(&parent_id) {
                let effective = if node.clip_mode != ClipMode::None {
                    Some(node.clip_mode)
                } else if node.scroll_offset != [0.0, 0.0] {
                    Some(ClipMode::Scissor)
                } else {
                    None
                };
                if let Some(mode) = effective {
                    result.push((parent_id, mode));
                }
            }
            current = parent_id;
        }
        result
    }

    /// Tests whether a world-space point passes the clip region of a clip node.
    ///
    /// The `effective_mode` parameter is the clip mode to test against, which may
    /// differ from `node.clip_mode` for implicit scroll clips.
    ///
    /// For `Scissor` and `ShaderRect` clips, tests against the world-space AABB.
    /// For `Stencil` clips on `Frame` nodes with corner radii, tests using the
    /// SDF distance to the rounded rect in local space.
    #[must_use]
    pub fn point_passes_clip(
        &self,
        x: f32,
        y: f32,
        clip_node_id: NodeId,
        effective_mode: ClipMode,
    ) -> bool {
        let Some(node) = self.nodes.get(&clip_node_id) else {
            return true;
        };

        match effective_mode {
            ClipMode::None => true,
            ClipMode::Scissor | ClipMode::ShaderRect => {
                // Test against world-space AABB.
                let world_aabb = node.world_transform.transform_aabb(&node.bounds);
                x >= world_aabb.x
                    && x <= world_aabb.right()
                    && y >= world_aabb.y
                    && y <= world_aabb.bottom()
            }
            ClipMode::Stencil => {
                // For stencil, test in local space for precision.
                let Some(inv) = node.world_transform.inverse() else {
                    return false;
                };
                let (lx, ly) = inv.transform_point(x, y);

                // First check AABB.
                if !node.bounds.contains_point(lx, ly) {
                    return false;
                }

                // If this is a Frame with corner radii, do SDF check.
                if let SceneNodeKind::Frame { corner_radius } = &node.kind {
                    let has_radii = corner_radius.iter().any(|&r| r > 0.0);
                    if has_radii {
                        return Self::point_inside_rounded_rect(
                            lx,
                            ly,
                            &node.bounds,
                            corner_radius,
                        );
                    }
                }

                true
            }
        }
    }

    /// SDF-based point-in-rounded-rect test.
    ///
    /// Returns `true` if the point (in local space) is inside the rounded rectangle.
    fn point_inside_rounded_rect(
        lx: f32,
        ly: f32,
        bounds: &BoundingBox,
        corner_radius: &[f32; 4],
    ) -> bool {
        let half_w = bounds.width * 0.5;
        let half_h = bounds.height * 0.5;
        let cx = bounds.x + half_w;
        let cy = bounds.y + half_h;

        // Position relative to rect center.
        let px = lx - cx;
        let py = ly - cy;

        // Select corner radius based on quadrant.
        // corner_radius: [top_left, top_right, bottom_right, bottom_left]
        let radius = if py < 0.0 {
            if px < 0.0 {
                corner_radius[0] // top-left
            } else {
                corner_radius[1] // top-right
            }
        } else if px >= 0.0 {
            corner_radius[2] // bottom-right
        } else {
            corner_radius[3] // bottom-left
        };

        let max_radius = half_w.min(half_h);
        let r = radius.min(max_radius);

        // SDF for rounded rect.
        let qx = px.abs() - half_w + r;
        let qy = py.abs() - half_h + r;
        let dist = qx.max(qy).min(0.0)
            + (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt()
            - r;

        dist <= 0.0
    }

    /// Returns node IDs at the given point, sorted front-to-back (topmost first).
    ///
    /// Uses the R-tree spatial index for broad-phase AABB filtering, then
    /// inverse-transforms the point into each node's local space for precise
    /// narrow-phase checking. Invisible nodes are excluded.
    ///
    /// Clip-aware: nodes whose ancestors have clip regions are excluded if the
    /// point falls outside any ancestor's clip region.
    ///
    /// Ensure z-indices are up-to-date before calling (e.g., via `visible_nodes_sorted`
    /// or after any tree structure changes).
    #[must_use]
    pub fn hit_test(&mut self, x: f32, y: f32) -> Vec<NodeId> {
        // Ensure z-indices are up to date.
        if self.z_dirty {
            self.recompute_z_indices();
        }

        let candidates = self.spatial.query_point(x, y);
        let mut hits: Vec<NodeId> = candidates
            .into_iter()
            .filter(|&id| {
                let Some(node) = self.nodes.get(&id) else {
                    return false;
                };

                // Skip invisible nodes.
                if !node.visible {
                    return false;
                }

                // Inverse-transform the click point into local space.
                let Some(inv) = node.world_transform.inverse() else {
                    // Non-invertible transform (zero-scale) — cannot be hit.
                    return false;
                };
                let (lx, ly) = inv.transform_point(x, y);
                if !node.bounds.contains_point(lx, ly) {
                    return false;
                }

                // Check ancestor clip chain.
                let clip_chain = self.ancestor_clip_chain(id);
                clip_chain
                    .iter()
                    .all(|&(clip_id, mode)| self.point_passes_clip(x, y, clip_id, mode))
            })
            .collect();

        // Sort front-to-back (highest z-index first).
        hits.sort_by(|a, b| {
            let za = self.z_indices.get(a).copied().unwrap_or(0);
            let zb = self.z_indices.get(b).copied().unwrap_or(0);
            zb.cmp(&za)
        });

        hits
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
            .map(|n| {
                let world_aabb = n.world_transform.transform_aabb(&n.bounds);
                crate::spatial::SpatialEntry::new(n.id, &world_aabb)
            })
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

    /// Marks a node and all its descendants with `TRANSFORM` dirty.
    ///
    /// Uses the clone-children-per-level pattern to avoid borrow conflicts.
    fn propagate_transform_dirty_down(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.dirty |= DirtyFlags::TRANSFORM;
        }
        self.has_any_transform_dirty = true;

        let children = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child_id in children {
            self.propagate_transform_dirty_down(child_id);
        }
    }

    /// Recursively recomputes world transforms for dirty nodes.
    ///
    /// Always recurses into children (they may be dirty even if the parent isn't).
    /// Only recomputes nodes that have the `TRANSFORM` flag set.
    fn recompute_world_transform_recursive(&mut self, id: NodeId, parent_world: Transform2D) {
        // Compute new world transform and collect data needed for spatial update.
        let (world_transform, scroll_offset, children, spatial_update) = {
            let Some(node) = self.nodes.get_mut(&id) else {
                return;
            };

            if node.dirty.contains(DirtyFlags::TRANSFORM) {
                node.world_transform = parent_world.compose(&node.local_transform);
                let world_aabb = node.world_transform.transform_aabb(&node.bounds);
                let wt = node.world_transform;
                let so = node.scroll_offset;
                node.dirty = node.dirty.without(DirtyFlags::TRANSFORM);
                (wt, so, node.children.clone(), Some(world_aabb))
            } else {
                (
                    node.world_transform,
                    node.scroll_offset,
                    node.children.clone(),
                    None,
                )
            }
        };

        // Update spatial index outside the nodes borrow.
        if let Some(world_aabb) = spatial_update {
            self.spatial.update_node(id, &world_aabb);
        }

        // If this node has a scroll offset, children see a shifted parent transform.
        let children_parent = if scroll_offset == [0.0, 0.0] {
            world_transform
        } else {
            world_transform.compose(&Transform2D::translation(
                -scroll_offset[0],
                -scroll_offset[1],
            ))
        };

        for child_id in children {
            self.recompute_world_transform_recursive(child_id, children_parent);
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
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::scene::{GradientStop, SceneNodeKind};

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
    fn set_fill_gradient_linear() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        let grad = Some(Gradient::Linear {
            start: [0.0, 0.0],
            end: [1.0, 1.0],
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: Color::new(1.0, 0.0, 0.0, 1.0),
                },
                GradientStop {
                    position: 1.0,
                    color: Color::new(0.0, 0.0, 1.0, 1.0),
                },
            ],
        });
        assert!(graph.set_fill_gradient(id, grad.clone()));
        assert_eq!(graph.get(id).expect("exists").fill_gradient, grad);
    }

    #[test]
    fn set_fill_gradient_clear() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        let grad = Some(Gradient::Radial {
            center: [0.5, 0.5],
            radius: 1.0,
            stops: vec![],
        });
        graph.set_fill_gradient(id, grad);
        assert!(graph.get(id).expect("exists").fill_gradient.is_some());

        assert!(graph.set_fill_gradient(id, None));
        assert!(graph.get(id).expect("exists").fill_gradient.is_none());
    }

    #[test]
    fn set_fill_gradient_missing_node() {
        let mut graph = SceneGraph::new();
        let missing = NodeId::new();
        assert!(!graph.set_fill_gradient(missing, None));
    }

    #[test]
    fn set_fill_gradient_marks_style_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        let grad = Some(Gradient::Linear {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![],
        });
        graph.set_fill_gradient(id, grad);
        let node = graph.get(id).expect("exists");
        assert!(node.dirty.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn set_effects_drop_shadow() {
        let mut graph = SceneGraph::new();
        let node = frame_node("FX", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        let effects = vec![Effect::DropShadow {
            color: Color::new(0.0, 0.0, 0.0, 0.5),
            offset_x: 4.0,
            offset_y: 4.0,
            blur_radius: 8.0,
        }];
        assert!(graph.set_effects(id, effects));
        assert_eq!(graph.get(id).expect("exists").effects.len(), 1);
    }

    #[test]
    fn set_effects_clear() {
        let mut graph = SceneGraph::new();
        let node = frame_node("FX", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        let effects = vec![Effect::Blur { radius: 5.0 }];
        graph.set_effects(id, effects);
        assert_eq!(graph.get(id).expect("exists").effects.len(), 1);

        assert!(graph.set_effects(id, Vec::new()));
        assert!(graph.get(id).expect("exists").effects.is_empty());
    }

    #[test]
    fn set_effects_missing_node() {
        let mut graph = SceneGraph::new();
        let missing = NodeId::new();
        assert!(!graph.set_effects(missing, Vec::new()));
    }

    #[test]
    fn set_effects_marks_effects_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("FX", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        graph.set_effects(id, vec![Effect::Blur { radius: 3.0 }]);
        let node = graph.get(id).expect("exists");
        assert!(node.dirty.contains(DirtyFlags::EFFECTS));
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
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
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
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
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
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
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
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
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

    // --- Transform mutations ---

    #[test]
    fn set_transform_marks_node_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        graph.set_transform(id, Transform2D::rotation(0.5));
        let node = graph.get(id).expect("exists");
        assert!(node.dirty.contains(DirtyFlags::TRANSFORM));
    }

    #[test]
    fn set_transform_on_nonexistent_returns_false() {
        let mut graph = SceneGraph::new();
        assert!(!graph.set_transform(NodeId::new(), Transform2D::identity()));
    }

    #[test]
    fn set_transform_propagates_to_descendants() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);
        let child = frame_node("Child", 10.0, 10.0, 100.0, 100.0);
        let child_id = child.id;
        graph.add_child(root_id, child);
        let gc = frame_node("GC", 20.0, 20.0, 50.0, 50.0);
        let gc_id = gc.id;
        graph.add_child(child_id, gc);
        graph.clear_all_dirty();

        // Set transform on root — child and grandchild should be dirty.
        graph.set_transform(root_id, Transform2D::rotation(0.5));
        assert!(
            graph
                .get(child_id)
                .expect("child")
                .dirty
                .contains(DirtyFlags::TRANSFORM)
        );
        assert!(
            graph
                .get(gc_id)
                .expect("gc")
                .dirty
                .contains(DirtyFlags::TRANSFORM)
        );
    }

    #[test]
    fn set_rotation_replaces_transform() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_rotation(id, std::f32::consts::FRAC_PI_4));

        let node = graph.get(id).expect("exists");
        assert!(!node.local_transform.is_identity());
        assert!(node.dirty.contains(DirtyFlags::TRANSFORM));
    }

    #[test]
    fn set_rotation_on_nonexistent_returns_false() {
        let mut graph = SceneGraph::new();
        assert!(!graph.set_rotation(NodeId::new(), 0.5));
    }

    #[test]
    fn set_scale_replaces_transform() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_scale(id, 2.0, 3.0));

        let node = graph.get(id).expect("exists");
        assert!(!node.local_transform.is_identity());
        assert!(node.dirty.contains(DirtyFlags::TRANSFORM));
    }

    #[test]
    fn set_scale_on_nonexistent_returns_false() {
        let mut graph = SceneGraph::new();
        assert!(!graph.set_scale(NodeId::new(), 1.0, 1.0));
    }

    #[test]
    fn set_blend_mode_marks_style_dirty() {
        use crate::scene::BlendMode;

        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_blend_mode(id, BlendMode::Add));
        let node = graph.get(id).expect("exists");
        assert_eq!(node.blend_mode, BlendMode::Add);
        assert!(node.dirty.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn set_blend_mode_on_nonexistent_returns_false() {
        use crate::scene::BlendMode;
        let mut graph = SceneGraph::new();
        assert!(!graph.set_blend_mode(NodeId::new(), BlendMode::Add));
    }

    // --- World transform recomputation ---

    #[test]
    fn recompute_world_transforms_root_only() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 10.0, 20.0, 100.0, 50.0);
        let id = graph.add_root(node);

        graph.set_transform(id, Transform2D::translation(100.0, 200.0));
        graph.recompute_world_transforms();

        let node = graph.get(id).expect("exists");
        assert!(!node.dirty.contains(DirtyFlags::TRANSFORM));
        // World = identity (no parent) * translation(100, 200) = translation(100, 200)
        let (x, y) = node.world_transform.transform_point(0.0, 0.0);
        assert!((x - 100.0).abs() < 1e-5);
        assert!((y - 200.0).abs() < 1e-5);
    }

    #[test]
    fn recompute_world_transforms_parent_child() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);
        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        graph.set_transform(parent_id, Transform2D::translation(100.0, 0.0));
        graph.recompute_world_transforms();

        // Child world = parent_world * child_local = translate(100,0) * identity
        let child_node = graph.get(child_id).expect("child");
        let (x, y) = child_node.world_transform.transform_point(0.0, 0.0);
        assert!((x - 100.0).abs() < 1e-5);
        assert!((y - 0.0).abs() < 1e-5);
    }

    #[test]
    fn recompute_world_transforms_cascades_parent_child() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);
        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        // Set transforms on both parent and child.
        graph.set_transform(parent_id, Transform2D::translation(100.0, 0.0));
        graph.set_transform(child_id, Transform2D::translation(0.0, 50.0));
        graph.recompute_world_transforms();

        // Child world = parent_world * child_local = translate(100,0) * translate(0,50)
        let child_node = graph.get(child_id).expect("child");
        let (x, y) = child_node.world_transform.transform_point(0.0, 0.0);
        assert!((x - 100.0).abs() < 1e-5);
        assert!((y - 50.0).abs() < 1e-5);
    }

    #[test]
    fn recompute_noop_on_clean_graph() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        graph.add_root(node);
        graph.clear_all_dirty();
        // This should be a no-op and not panic.
        graph.recompute_world_transforms();
    }

    // --- Hit testing ---

    #[test]
    fn hit_test_unrotated_rect() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 100.0, 100.0, 50.0, 50.0);
        let id = graph.add_root(node);
        graph.recompute_world_transforms();

        // Hit at center.
        let hits = graph.hit_test(125.0, 125.0);
        assert_eq!(hits, vec![id]);

        // Miss outside.
        let hits = graph.hit_test(200.0, 200.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn hit_test_translated_rect() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 50.0, 50.0);
        let id = graph.add_root(node);

        graph.set_transform(id, Transform2D::translation(100.0, 100.0));
        graph.recompute_world_transforms();

        // Original location should miss.
        let hits = graph.hit_test(25.0, 25.0);
        assert!(hits.is_empty());

        // Translated location should hit.
        let hits = graph.hit_test(125.0, 125.0);
        assert_eq!(hits, vec![id]);
    }

    #[test]
    fn hit_test_rotated_rect() {
        let mut graph = SceneGraph::new();
        // 100x100 rect at (0, 0).
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        // Rotate 45 degrees around center (50, 50).
        graph.set_rotation(id, std::f32::consts::FRAC_PI_4);
        graph.recompute_world_transforms();

        // Center should always hit.
        let hits = graph.hit_test(50.0, 50.0);
        assert_eq!(hits, vec![id]);
    }

    #[test]
    fn hit_test_skips_invisible() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.set_visible(id, false);
        graph.recompute_world_transforms();

        let hits = graph.hit_test(50.0, 50.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn hit_test_skips_zero_scale() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        graph.set_transform(id, Transform2D::scale(0.0, 0.0));
        graph.recompute_world_transforms();

        let hits = graph.hit_test(50.0, 50.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn hit_test_front_to_back_order() {
        let mut graph = SceneGraph::new();
        graph.spatial_mut().set_overscan(0.0);

        let back = frame_node("Back", 0.0, 0.0, 100.0, 100.0);
        let back_id = graph.add_root(back);
        let front = frame_node("Front", 0.0, 0.0, 100.0, 100.0);
        let front_id = graph.add_root(front);
        graph.recompute_world_transforms();

        let hits = graph.hit_test(50.0, 50.0);
        assert_eq!(hits.len(), 2);
        // Front (higher z-index) should come first.
        assert_eq!(hits[0], front_id);
        assert_eq!(hits[1], back_id);
    }

    // --- add_child / reparent transform dirty propagation (Issue 10) ---

    #[test]
    fn add_child_marks_child_transform_dirty() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);
        graph.clear_all_dirty();

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        let child_node = graph.get(child_id).expect("child");
        assert!(
            child_node.dirty.contains(DirtyFlags::TRANSFORM),
            "newly added child should have TRANSFORM dirty"
        );
    }

    #[test]
    fn reparent_marks_node_and_descendants_transform_dirty() {
        let mut graph = SceneGraph::new();
        let first_parent = frame_node("ParentA", 0.0, 0.0, 200.0, 200.0);
        let first_parent_id = graph.add_root(first_parent);
        let second_parent = frame_node("ParentB", 300.0, 0.0, 200.0, 200.0);
        let second_parent_id = graph.add_root(second_parent);

        let child = frame_node("Child", 10.0, 10.0, 100.0, 100.0);
        let child_id = child.id;
        graph.add_child(first_parent_id, child);
        let gc = frame_node("GC", 20.0, 20.0, 30.0, 30.0);
        let gc_id = gc.id;
        graph.add_child(child_id, gc);

        graph.clear_all_dirty();

        // Reparent child (with grandchild) to second_parent.
        graph.reparent(child_id, second_parent_id);

        let child_node = graph.get(child_id).expect("child");
        assert!(
            child_node.dirty.contains(DirtyFlags::TRANSFORM),
            "reparented child should have TRANSFORM dirty"
        );
        let gc_node = graph.get(gc_id).expect("gc");
        assert!(
            gc_node.dirty.contains(DirtyFlags::TRANSFORM),
            "reparented grandchild should have TRANSFORM dirty"
        );
    }

    #[test]
    fn reparent_same_parent_does_not_dirty_transform() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);
        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        graph.clear_all_dirty();

        // Reparent to same parent — should be a no-op.
        graph.reparent(child_id, parent_id);

        let child_node = graph.get(child_id).expect("child");
        assert!(
            !child_node.dirty.contains(DirtyFlags::TRANSFORM),
            "same-parent reparent should not dirty TRANSFORM"
        );
    }

    // --- Proptest: SceneGraph + SpatialIndex consistency ---

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_bounds() -> impl Strategy<Value = BoundingBox> {
            (
                0.0_f32..1000.0,
                0.0_f32..1000.0,
                1.0_f32..200.0,
                1.0_f32..200.0,
            )
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

    // --- Clip mode tests ---

    #[test]
    fn set_clip_mode_marks_clip_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Clip", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        assert!(graph.set_clip_mode(id, ClipMode::Scissor));

        let n = graph.get(id).expect("exists");
        assert_eq!(n.clip_mode, ClipMode::Scissor);
        assert!(n.dirty.contains(DirtyFlags::CLIP));
    }

    #[test]
    fn set_clip_mode_propagates_children_up() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);
        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);
        graph.clear_all_dirty();

        graph.set_clip_mode(child_id, ClipMode::Stencil);

        let p = graph.get(parent_id).expect("exists");
        assert!(p.dirty.contains(DirtyFlags::CHILDREN));
    }

    #[test]
    fn set_clip_mode_nonexistent_returns_false() {
        let mut graph = SceneGraph::new();
        assert!(!graph.set_clip_mode(NodeId::new(), ClipMode::Scissor));
    }

    #[test]
    fn clip_mode_default_is_none() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        assert_eq!(graph.get(id).expect("exists").clip_mode, ClipMode::None);
    }

    #[test]
    fn set_clip_mode_all_modes() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        for mode in [
            ClipMode::None,
            ClipMode::Scissor,
            ClipMode::Stencil,
            ClipMode::ShaderRect,
        ] {
            graph.set_clip_mode(id, mode);
            assert_eq!(graph.get(id).expect("exists").clip_mode, mode);
        }
    }

    // --- Clip-aware hit testing tests ---

    #[test]
    fn scissor_clip_blocks_hit_outside_parent_bounds() {
        let mut graph = SceneGraph::new();
        // Parent at (0,0) 100x100 with Scissor clip.
        let parent = frame_node("ClipParent", 0.0, 0.0, 100.0, 100.0);
        let parent_id = graph.add_root(parent);
        graph.set_clip_mode(parent_id, ClipMode::Scissor);

        // Child extends beyond parent bounds: (50, 50) to (200, 200).
        let child = frame_node("Child", 50.0, 50.0, 150.0, 150.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        graph.recompute_world_transforms();

        // Point inside both parent and child: should hit.
        let hits = graph.hit_test(75.0, 75.0);
        assert!(hits.contains(&child_id));

        // Point inside child but outside parent clip: should NOT hit child.
        let hits = graph.hit_test(150.0, 150.0);
        assert!(!hits.contains(&child_id));
    }

    #[test]
    fn stencil_clip_with_rounded_corners_blocks_corner_hit() {
        let mut graph = SceneGraph::new();
        // Parent with large corner radius (50 = half of 100px).
        let mut parent = SceneNode::new(
            NodeId::new(),
            "RoundedClip".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [50.0, 50.0, 50.0, 50.0],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        parent.fill = Some(Color::WHITE);
        let parent_id = graph.add_root(parent);
        graph.set_clip_mode(parent_id, ClipMode::Stencil);

        // Child fills the entire parent.
        let child = frame_node("Child", 0.0, 0.0, 100.0, 100.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        graph.recompute_world_transforms();

        // Center of rounded rect: should hit.
        let hits = graph.hit_test(50.0, 50.0);
        assert!(hits.contains(&child_id));

        // Corner of the AABB (outside the rounded rect): should NOT hit.
        // With radius 50 (fully rounded = circle), point (1, 1) is outside.
        let hits = graph.hit_test(1.0, 1.0);
        assert!(!hits.contains(&child_id));
    }

    #[test]
    fn nested_clips_both_must_pass() {
        let mut graph = SceneGraph::new();
        // Outer clip at (0,0) 200x200.
        let outer = frame_node("Outer", 0.0, 0.0, 200.0, 200.0);
        let outer_id = graph.add_root(outer);
        graph.set_clip_mode(outer_id, ClipMode::Scissor);

        // Inner clip at (50,50) 100x100.
        let inner = frame_node("Inner", 50.0, 50.0, 100.0, 100.0);
        let inner_id = inner.id;
        graph.add_child(outer_id, inner);
        graph.set_clip_mode(inner_id, ClipMode::Scissor);

        // Leaf covers a wide area.
        let leaf = frame_node("Leaf", 0.0, 0.0, 300.0, 300.0);
        let leaf_id = leaf.id;
        graph.add_child(inner_id, leaf);

        graph.recompute_world_transforms();

        // Inside both clips.
        let hits = graph.hit_test(75.0, 75.0);
        assert!(hits.contains(&leaf_id));

        // Inside outer but outside inner.
        let hits = graph.hit_test(25.0, 25.0);
        assert!(!hits.contains(&leaf_id));

        // Outside both.
        let hits = graph.hit_test(250.0, 250.0);
        assert!(!hits.contains(&leaf_id));
    }

    #[test]
    fn no_clip_regression_passes() {
        // Ensure nodes without clip ancestors still work.
        let mut graph = SceneGraph::new();
        let node = frame_node("Normal", 10.0, 10.0, 50.0, 50.0);
        let id = graph.add_root(node);
        graph.recompute_world_transforms();

        let hits = graph.hit_test(30.0, 30.0);
        assert!(hits.contains(&id));
    }

    #[test]
    fn ancestor_clip_chain_returns_correct_ancestors() {
        let mut graph = SceneGraph::new();
        let a = frame_node("A", 0.0, 0.0, 200.0, 200.0);
        let a_id = graph.add_root(a);
        graph.set_clip_mode(a_id, ClipMode::Scissor);

        let b = frame_node("B", 10.0, 10.0, 150.0, 150.0);
        let b_id = b.id;
        graph.add_child(a_id, b);
        // B has no clip mode.

        let c = frame_node("C", 20.0, 20.0, 100.0, 100.0);
        let c_id = c.id;
        graph.add_child(b_id, c);
        graph.set_clip_mode(c_id, ClipMode::Stencil);

        let d = frame_node("D", 30.0, 30.0, 50.0, 50.0);
        let d_id = d.id;
        graph.add_child(c_id, d);

        let chain = graph.ancestor_clip_chain(d_id);
        // Should contain C (Stencil) and A (Scissor), but not B (None).
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0], (c_id, ClipMode::Stencil));
        assert_eq!(chain[1], (a_id, ClipMode::Scissor));
    }

    #[test]
    fn shader_rect_clip_blocks_hit() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("ShaderClip", 0.0, 0.0, 100.0, 100.0);
        let parent_id = graph.add_root(parent);
        graph.set_clip_mode(parent_id, ClipMode::ShaderRect);

        let child = frame_node("Child", 80.0, 80.0, 100.0, 100.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        graph.recompute_world_transforms();

        // Inside parent clip and child.
        let hits = graph.hit_test(90.0, 90.0);
        assert!(hits.contains(&child_id));

        // Inside child but outside parent clip.
        let hits = graph.hit_test(120.0, 120.0);
        assert!(!hits.contains(&child_id));
    }

    // --- Scroll offset tests ---

    #[test]
    fn default_scroll_offset_is_zero() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        assert_eq!(graph.scroll_offset(id), Some([0.0, 0.0]));
    }

    #[test]
    fn set_scroll_offset_stores_value() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        assert!(graph.set_scroll_offset(id, 50.0, 30.0));
        assert_eq!(graph.scroll_offset(id), Some([50.0, 30.0]));
    }

    #[test]
    fn set_scroll_offset_marks_transform_dirty() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Node", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);
        graph.clear_all_dirty();

        graph.set_scroll_offset(id, 10.0, 20.0);
        let node = graph.get(id).expect("node exists");
        assert!(node.dirty.contains(DirtyFlags::TRANSFORM));
    }

    #[test]
    fn set_scroll_offset_propagates_to_descendants() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 10.0, 10.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        let grandchild = frame_node("Grandchild", 5.0, 5.0, 20.0, 20.0);
        let grandchild_id = grandchild.id;
        graph.add_child(child_id, grandchild);

        graph.recompute_world_transforms();
        graph.clear_all_dirty();

        graph.set_scroll_offset(parent_id, 10.0, 10.0);

        let child_node = graph.get(child_id).expect("child exists");
        assert!(child_node.dirty.contains(DirtyFlags::TRANSFORM));

        let gc_node = graph.get(grandchild_id).expect("grandchild exists");
        assert!(gc_node.dirty.contains(DirtyFlags::TRANSFORM));
    }

    #[test]
    fn set_scroll_offset_on_nonexistent_returns_false() {
        let mut graph = SceneGraph::new();
        assert!(!graph.set_scroll_offset(NodeId::new(), 10.0, 10.0));
    }

    // --- Scroll transform injection tests ---

    #[test]
    fn scroll_offset_translates_children_world_transform() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 50.0, 50.0, 40.0, 40.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        graph.set_scroll_offset(parent_id, 20.0, 10.0);
        graph.recompute_world_transforms();

        let child_wt = graph.get(child_id).expect("child").world_transform;
        // Child's world_transform should include the scroll translation.
        // With identity parent transform and scroll (20, 10), child position becomes
        // local (50, 50) shifted by (-20, -10) = world (30, 40).
        let (wx, wy) = child_wt.transform_point(50.0, 50.0);
        assert!((wx - 30.0).abs() < 0.001);
        assert!((wy - 40.0).abs() < 0.001);
    }

    #[test]
    fn scroll_offset_does_not_affect_parent_world_transform() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 100.0, 100.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        graph.set_scroll_offset(parent_id, 50.0, 50.0);
        graph.recompute_world_transforms();

        // Parent's own world transform should be unaffected by scroll.
        let parent_wt = graph.get(parent_id).expect("parent").world_transform;
        assert!(parent_wt.is_identity());
    }

    #[test]
    fn scroll_offset_cascades_to_grandchildren() {
        let mut graph = SceneGraph::new();
        let root = frame_node("Root", 0.0, 0.0, 500.0, 500.0);
        let root_id = graph.add_root(root);

        let child = frame_node("Child", 100.0, 100.0, 200.0, 200.0);
        let child_id = child.id;
        graph.add_child(root_id, child);

        let grandchild = frame_node("Grandchild", 10.0, 10.0, 30.0, 30.0);
        let grandchild_id = grandchild.id;
        graph.add_child(child_id, grandchild);

        graph.set_scroll_offset(root_id, 30.0, 20.0);
        graph.recompute_world_transforms();

        // Grandchild should also be shifted by root's scroll offset.
        let gc_wt = graph.get(grandchild_id).expect("gc").world_transform;
        let (wx, wy) = gc_wt.transform_point(10.0, 10.0);
        // grandchild local (10,10) => child local (10,10) with identity child transform
        // child is shifted by root scroll (-30, -20)
        assert!((wx - (-20.0)).abs() < 0.001);
        assert!((wy - (-10.0)).abs() < 0.001);
    }

    #[test]
    fn scroll_offset_composes_with_parent_transform() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);
        // Set a translation transform on the parent.
        graph.set_transform(parent_id, Transform2D::translation(100.0, 50.0));

        let child = frame_node("Child", 0.0, 0.0, 40.0, 40.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        graph.set_scroll_offset(parent_id, 10.0, 5.0);
        graph.recompute_world_transforms();

        let child_wt = graph.get(child_id).expect("child").world_transform;
        // Parent world = identity * translation(100,50) = translation(100,50)
        // Scroll shifts children by (-10, -5) in parent's space.
        // So child world = translation(100,50) * translation(-10,-5) = translation(90,45)
        let (wx, wy) = child_wt.transform_point(0.0, 0.0);
        assert!((wx - 90.0).abs() < 0.001);
        assert!((wy - 45.0).abs() < 0.001);
    }

    #[test]
    fn nested_scroll_containers_compose() {
        let mut graph = SceneGraph::new();
        let outer = frame_node("Outer", 0.0, 0.0, 400.0, 400.0);
        let outer_id = graph.add_root(outer);

        let inner = frame_node("Inner", 0.0, 0.0, 200.0, 200.0);
        let inner_id = inner.id;
        graph.add_child(outer_id, inner);

        let leaf = frame_node("Leaf", 0.0, 0.0, 50.0, 50.0);
        let leaf_id = leaf.id;
        graph.add_child(inner_id, leaf);

        graph.set_scroll_offset(outer_id, 10.0, 20.0);
        graph.set_scroll_offset(inner_id, 5.0, 3.0);
        graph.recompute_world_transforms();

        let leaf_wt = graph.get(leaf_id).expect("leaf").world_transform;
        // Outer scroll shifts inner by (-10, -20).
        // Inner scroll shifts leaf by (-5, -3).
        // Total shift on leaf: (-15, -23).
        let (wx, wy) = leaf_wt.transform_point(0.0, 0.0);
        assert!((wx - (-15.0)).abs() < 0.001);
        assert!((wy - (-23.0)).abs() < 0.001);
    }

    #[test]
    fn scroll_offset_updates_spatial_index() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 300.0, 300.0);
        let parent_id = graph.add_root(parent);

        // Child at (200, 200, 50, 50) — initially visible in spatial index.
        let child = frame_node("Child", 200.0, 200.0, 50.0, 50.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);
        graph.recompute_world_transforms();

        // Child should be at (200,200)-(250,250) in world space.
        let hits = graph.spatial().query_point(225.0, 225.0);
        assert!(hits.contains(&child_id));

        // Scroll right by 200. Child world position becomes (0, 200).
        graph.set_scroll_offset(parent_id, 200.0, 0.0);
        graph.recompute_world_transforms();

        // Old position should miss.
        let hits = graph.spatial().query_point(225.0, 225.0);
        assert!(!hits.contains(&child_id));

        // New position (0, 200)-(50, 250) should hit.
        let hits = graph.spatial().query_point(25.0, 225.0);
        assert!(hits.contains(&child_id));
    }

    #[test]
    fn zero_scroll_offset_is_noop() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 50.0, 50.0, 40.0, 40.0);
        let child_id = child.id;
        graph.add_child(parent_id, child);

        // Explicitly set zero scroll offset.
        graph.set_scroll_offset(parent_id, 0.0, 0.0);
        graph.recompute_world_transforms();

        let child_wt = graph.get(child_id).expect("child").world_transform;
        // Child should have identity world transform (no parent transform, no scroll).
        assert!(child_wt.is_identity());
    }

    // --- Implicit scroll clip tests ---

    #[test]
    fn scroll_container_blocks_hit_outside_bounds() {
        let mut graph = SceneGraph::new();
        // Scroll container at (0,0,100,100).
        let container = frame_node("Container", 0.0, 0.0, 100.0, 100.0);
        let container_id = graph.add_root(container);

        // Child at (0,0,100,50) — fully inside container before scroll.
        let child = frame_node("Child", 0.0, 0.0, 100.0, 50.0);
        let child_id = child.id;
        graph.add_child(container_id, child);

        // Scroll down by 60. Child shifts from (0,0) to (0,-60) in world space.
        // The implicit scissor clip should hide the portion outside container bounds.
        graph.set_scroll_offset(container_id, 0.0, 60.0);
        graph.recompute_world_transforms();

        // Point at (50, -30) is where the child moved but outside container bounds (y=0..100).
        // This point is outside the container, so it should not hit.
        let hits = graph.hit_test(50.0, -30.0);
        assert!(!hits.contains(&child_id));
    }

    #[test]
    fn scroll_container_hit_inside_bounds() {
        let mut graph = SceneGraph::new();
        // Scroll container at (0,0,200,200).
        let container = frame_node("Container", 0.0, 0.0, 200.0, 200.0);
        let container_id = graph.add_root(container);

        // Child at (50,150,80,80) — partially visible.
        let child = frame_node("Child", 50.0, 150.0, 80.0, 80.0);
        let child_id = child.id;
        graph.add_child(container_id, child);

        // Scroll down by 100. Child shifts to (50, 50) in world space.
        graph.set_scroll_offset(container_id, 0.0, 100.0);
        graph.recompute_world_transforms();

        // Point at (90, 90) should be inside both child and container bounds.
        let hits = graph.hit_test(90.0, 90.0);
        assert!(hits.contains(&child_id));
    }

    #[test]
    fn scroll_container_with_explicit_clip_uses_explicit() {
        let mut graph = SceneGraph::new();
        let container = frame_node("Container", 0.0, 0.0, 100.0, 100.0);
        let container_id = graph.add_root(container);
        // Set explicit Stencil clip mode.
        graph.set_clip_mode(container_id, ClipMode::Stencil);
        graph.set_scroll_offset(container_id, 10.0, 10.0);

        let child = frame_node("Child", 5.0, 5.0, 20.0, 20.0);
        let child_id = child.id;
        graph.add_child(container_id, child);

        graph.recompute_world_transforms();

        // The ancestor_clip_chain should report Stencil (explicit), not Scissor (implicit).
        let chain = graph.ancestor_clip_chain(child_id);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].1, ClipMode::Stencil);
    }

    // --- Content bounds + scroll clamping tests ---

    #[test]
    fn compute_content_bounds_no_children() {
        let mut graph = SceneGraph::new();
        let node = frame_node("Empty", 0.0, 0.0, 100.0, 100.0);
        let id = graph.add_root(node);

        let bounds = graph.compute_content_bounds(id).expect("node exists");
        assert!(bounds.is_empty());
    }

    #[test]
    fn compute_content_bounds_single_child() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let child = frame_node("Child", 10.0, 20.0, 80.0, 60.0);
        graph.add_child(parent_id, child);

        let bounds = graph.compute_content_bounds(parent_id).expect("exists");
        assert_eq!(bounds.x, 10.0);
        assert_eq!(bounds.y, 20.0);
        assert_eq!(bounds.width, 80.0);
        assert_eq!(bounds.height, 60.0);
    }

    #[test]
    fn compute_content_bounds_multiple_children() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let c1 = frame_node("C1", 0.0, 0.0, 50.0, 50.0);
        graph.add_child(parent_id, c1);

        let c2 = frame_node("C2", 100.0, 100.0, 150.0, 80.0);
        graph.add_child(parent_id, c2);

        let bounds = graph.compute_content_bounds(parent_id).expect("exists");
        assert_eq!(bounds.x, 0.0);
        assert_eq!(bounds.y, 0.0);
        assert_eq!(bounds.width, 250.0);
        assert_eq!(bounds.height, 180.0);
    }

    #[test]
    fn compute_content_bounds_nonexistent() {
        let graph = SceneGraph::new();
        assert!(graph.compute_content_bounds(NodeId::new()).is_none());
    }

    #[test]
    fn compute_content_bounds_with_transformed_child() {
        let mut graph = SceneGraph::new();
        let parent = frame_node("Parent", 0.0, 0.0, 200.0, 200.0);
        let parent_id = graph.add_root(parent);

        let mut child = frame_node("Child", 0.0, 0.0, 50.0, 50.0);
        child.local_transform = Transform2D::translation(100.0, 100.0);
        graph.add_child(parent_id, child);

        let bounds = graph.compute_content_bounds(parent_id).expect("exists");
        // Child at (0,0,50,50) translated by (100,100) => AABB at (100,100,50,50).
        assert_eq!(bounds.x, 100.0);
        assert_eq!(bounds.y, 100.0);
        assert_eq!(bounds.width, 50.0);
        assert_eq!(bounds.height, 50.0);
    }

    #[test]
    fn max_scroll_basic() {
        let mut graph = SceneGraph::new();
        // Container is 100x100.
        let container = frame_node("Container", 0.0, 0.0, 100.0, 100.0);
        let container_id = graph.add_root(container);

        // Content extends to (200, 300).
        let child = frame_node("Child", 0.0, 0.0, 200.0, 300.0);
        graph.add_child(container_id, child);

        let max = graph.max_scroll(container_id).expect("exists");
        assert_eq!(max[0], 100.0); // 200 - 100
        assert_eq!(max[1], 200.0); // 300 - 100
    }

    #[test]
    fn max_scroll_content_smaller_than_container() {
        let mut graph = SceneGraph::new();
        let container = frame_node("Container", 0.0, 0.0, 200.0, 200.0);
        let container_id = graph.add_root(container);

        let child = frame_node("Child", 0.0, 0.0, 50.0, 50.0);
        graph.add_child(container_id, child);

        let max = graph.max_scroll(container_id).expect("exists");
        assert_eq!(max, [0.0, 0.0]);
    }

    #[test]
    fn set_scroll_offset_clamped_basic() {
        let mut graph = SceneGraph::new();
        let container = frame_node("Container", 0.0, 0.0, 100.0, 100.0);
        let container_id = graph.add_root(container);

        let child = frame_node("Child", 0.0, 0.0, 300.0, 250.0);
        graph.add_child(container_id, child);

        // Request scroll beyond max.
        assert!(graph.set_scroll_offset_clamped(container_id, 500.0, 400.0));
        let offset = graph.scroll_offset(container_id).expect("exists");
        assert_eq!(offset[0], 200.0); // max = 300 - 100
        assert_eq!(offset[1], 150.0); // max = 250 - 100
    }

    #[test]
    fn set_scroll_offset_clamped_negative_clamped_to_zero() {
        let mut graph = SceneGraph::new();
        let container = frame_node("Container", 0.0, 0.0, 100.0, 100.0);
        let container_id = graph.add_root(container);

        let child = frame_node("Child", 0.0, 0.0, 200.0, 200.0);
        graph.add_child(container_id, child);

        assert!(graph.set_scroll_offset_clamped(container_id, -50.0, -30.0));
        let offset = graph.scroll_offset(container_id).expect("exists");
        assert_eq!(offset, [0.0, 0.0]);
    }
}
