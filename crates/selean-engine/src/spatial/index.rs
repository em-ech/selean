//! R-tree backed spatial index.
//!
//! The index stores lightweight entries mapping `NodeId` → axis-aligned bounding box.
//! Full node data is looked up from the scene graph store after querying.

use rstar::{AABB, PointDistance, RTree, RTreeObject, SelectionFunction};
use selean_common::types::NodeId;

use crate::scene::BoundingBox;

/// Default overscan margin as a fraction of viewport dimensions.
/// 0.2 = 20% overscan on each side.
const DEFAULT_OVERSCAN: f32 = 0.2;

/// A lightweight entry in the spatial index.
///
/// Contains only the node's ID and its axis-aligned bounding box.
/// Full node data should be looked up from the scene graph store
/// using the returned `NodeId`.
#[derive(Debug, Clone)]
pub struct SpatialEntry {
    /// The node this entry represents.
    pub id: NodeId,
    /// The node's bounding box as an rstar AABB.
    envelope: AABB<[f32; 2]>,
}

impl SpatialEntry {
    /// Creates a new spatial entry from a node ID and bounding box.
    #[must_use]
    pub fn new(id: NodeId, bounds: &BoundingBox) -> Self {
        let lower = [bounds.x, bounds.y];
        let upper = [bounds.right(), bounds.bottom()];
        Self {
            id,
            envelope: AABB::from_corners(lower, upper),
        }
    }

    /// Creates a new spatial entry from raw corner coordinates.
    #[must_use]
    pub fn from_corners(id: NodeId, min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            id,
            envelope: AABB::from_corners([min_x, min_y], [max_x, max_y]),
        }
    }
}

impl RTreeObject for SpatialEntry {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

impl PointDistance for SpatialEntry {
    fn distance_2(&self, point: &[f32; 2]) -> f32 {
        self.envelope.distance_2(point)
    }

    fn contains_point(&self, point: &[f32; 2]) -> bool {
        let lower = self.envelope.lower();
        let upper = self.envelope.upper();
        point[0] >= lower[0] && point[0] <= upper[0] && point[1] >= lower[1] && point[1] <= upper[1]
    }
}

impl PartialEq for SpatialEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for SpatialEntry {}

/// Selection function that matches a spatial entry by its `NodeId`.
///
/// Used internally for ID-based removal from the R-tree.
struct SelectById {
    target: NodeId,
}

impl SelectionFunction<SpatialEntry> for SelectById {
    fn should_unpack_parent(&self, envelope: &AABB<[f32; 2]>) -> bool {
        // Always unpack parent nodes — we don't know which subtree
        // contains our target until we check the leaves.
        // For large trees this could be optimized with a spatial hint,
        // but for our use case the linear leaf scan is fast enough.
        let _ = envelope;
        true
    }

    fn should_unpack_leaf(&self, leaf: &SpatialEntry) -> bool {
        leaf.id == self.target
    }
}

/// R-tree spatial index for efficient spatial queries on scene nodes.
///
/// Provides three query methods:
/// - [`query_viewport`](Self::query_viewport) — Returns nodes visible in the current viewport
///   (with configurable overscan margin for smooth panning).
/// - [`query_point`](Self::query_point) — Returns nodes containing a point (for hit testing).
/// - [`query_region`](Self::query_region) — Returns nodes intersecting an arbitrary rectangle
///   (for marquee selection, AI spatial queries).
///
/// Supports both incremental updates (single node insert/remove/update) and
/// bulk rebuild (for file import or large structural changes).
#[derive(Clone)]
pub struct SpatialIndex {
    /// The R-tree holding all spatial entries.
    tree: RTree<SpatialEntry>,
    /// Overscan margin as a fraction of viewport dimensions (0.0 to 1.0).
    /// Applied on each side, so 0.2 means the query rect is 40% wider and taller.
    overscan: f32,
}

impl SpatialIndex {
    /// Creates a new empty spatial index with default overscan (20%).
    #[must_use]
    pub fn new() -> Self {
        Self {
            tree: RTree::new(),
            overscan: DEFAULT_OVERSCAN,
        }
    }

    /// Creates a spatial index by bulk-loading entries.
    ///
    /// This is significantly faster than inserting entries one-by-one for
    /// large datasets (e.g., importing a .fig file). Uses an optimized
    /// bulk-loading algorithm that produces a well-balanced tree.
    #[must_use]
    pub fn bulk_load(entries: Vec<SpatialEntry>) -> Self {
        Self {
            tree: RTree::bulk_load(entries),
            overscan: DEFAULT_OVERSCAN,
        }
    }

    /// Sets the overscan margin as a fraction of viewport dimensions.
    ///
    /// The overscan is applied on each side of the viewport query rectangle,
    /// so a value of 0.2 (20%) expands the query by 40% total in each axis.
    /// This pre-loads nodes just outside the viewport to prevent pop-in
    /// during panning.
    ///
    /// The value is clamped to [0.0, 1.0].
    pub fn set_overscan(&mut self, overscan: f32) {
        self.overscan = overscan.clamp(0.0, 1.0);
    }

    /// Returns the current overscan fraction.
    #[must_use]
    pub fn overscan(&self) -> f32 {
        self.overscan
    }

    /// Returns the number of entries in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tree.size()
    }

    /// Returns `true` if the index contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    // --- Mutation methods ---

    /// Inserts a node into the spatial index.
    pub fn insert(&mut self, entry: SpatialEntry) {
        self.tree.insert(entry);
    }

    /// Inserts a node using its ID and bounding box.
    pub fn insert_node(&mut self, id: NodeId, bounds: &BoundingBox) {
        self.tree.insert(SpatialEntry::new(id, bounds));
    }

    /// Removes a node from the spatial index by its ID.
    ///
    /// Returns `true` if the node was found and removed, `false` otherwise.
    ///
    /// This performs a linear scan to find the entry by ID, then removes it.
    /// For bulk removals, prefer rebuilding the index.
    pub fn remove(&mut self, id: NodeId) -> bool {
        self.tree
            .remove_with_selection_function(SelectById { target: id })
            .is_some()
    }

    /// Updates a node's bounding box in the index.
    ///
    /// This removes the old entry and inserts a new one. If the node is not
    /// found, the new entry is inserted anyway (handles the case where a node
    /// was just created).
    pub fn update_node(&mut self, id: NodeId, new_bounds: &BoundingBox) {
        self.remove(id);
        self.insert_node(id, new_bounds);
    }

    /// Rebuilds the index from a new set of entries, replacing the existing tree.
    ///
    /// Use this after large structural changes (file import, undo/redo of bulk
    /// operations) rather than many individual insert/remove calls.
    pub fn rebuild(&mut self, entries: Vec<SpatialEntry>) {
        self.tree = RTree::bulk_load(entries);
    }

    /// Removes all entries from the index.
    pub fn clear(&mut self) {
        self.tree = RTree::new();
    }

    // --- Query methods ---

    /// Returns node IDs visible in the given viewport rectangle, with overscan.
    ///
    /// The viewport is defined as `(left, top, right, bottom)` in world coordinates,
    /// typically obtained from `Camera::visible_rect()`. The overscan margin is
    /// applied to expand the query rectangle for smooth panning.
    ///
    /// Returns node IDs in no particular order. The caller is responsible for
    /// sorting by render order if needed.
    #[must_use]
    pub fn query_viewport(&self, left: f32, top: f32, right: f32, bottom: f32) -> Vec<NodeId> {
        let width = right - left;
        let height = bottom - top;
        let margin_x = width * self.overscan;
        let margin_y = height * self.overscan;

        let expanded = AABB::from_corners(
            [left - margin_x, top - margin_y],
            [right + margin_x, bottom + margin_y],
        );

        self.tree
            .locate_in_envelope_intersecting(&expanded)
            .map(|entry| entry.id)
            .collect()
    }

    /// Returns node IDs containing the given point, for hit testing.
    ///
    /// The returned IDs are in **insertion order** (which may not match render order).
    /// The caller should sort by z-order (render order) and pick the topmost node
    /// for click-to-select behavior.
    #[must_use]
    pub fn query_point(&self, x: f32, y: f32) -> Vec<NodeId> {
        let point_envelope = AABB::from_point([x, y]);
        self.tree
            .locate_in_envelope_intersecting(&point_envelope)
            .map(|entry| entry.id)
            .collect()
    }

    /// Returns node IDs intersecting an arbitrary rectangle.
    ///
    /// Use for marquee selection, AI spatial queries ("nodes in the top half"),
    /// or any region-based query. No overscan is applied.
    #[must_use]
    pub fn query_region(&self, left: f32, top: f32, right: f32, bottom: f32) -> Vec<NodeId> {
        let region = AABB::from_corners([left, top], [right, bottom]);
        self.tree
            .locate_in_envelope_intersecting(&region)
            .map(|entry| entry.id)
            .collect()
    }

    /// Returns the nearest node to the given point, if any.
    ///
    /// Useful for snapping and proximity-based interactions.
    #[must_use]
    pub fn nearest_to_point(&self, x: f32, y: f32) -> Option<NodeId> {
        self.tree.nearest_neighbor(&[x, y]).map(|entry| entry.id)
    }
}

impl Default for SpatialIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SpatialIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpatialIndex")
            .field("entries", &self.tree.size())
            .field("overscan", &self.overscan)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    /// Helper: create an entry at a specific position and size.
    fn entry(id: NodeId, x: f32, y: f32, w: f32, h: f32) -> SpatialEntry {
        SpatialEntry::new(id, &BoundingBox::new(x, y, w, h))
    }

    /// Helper: create a new unique entry at a specific position.
    fn entry_at(x: f32, y: f32, w: f32, h: f32) -> SpatialEntry {
        entry(NodeId::new(), x, y, w, h)
    }

    // --- Construction and basic operations ---

    #[test]
    fn new_index_is_empty() {
        let index = SpatialIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn insert_and_len() {
        let mut index = SpatialIndex::new();
        index.insert(entry_at(0.0, 0.0, 10.0, 10.0));
        index.insert(entry_at(20.0, 20.0, 10.0, 10.0));
        assert_eq!(index.len(), 2);
        assert!(!index.is_empty());
    }

    #[test]
    fn insert_node_convenience() {
        let mut index = SpatialIndex::new();
        let id = NodeId::new();
        index.insert_node(id, &BoundingBox::new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn remove_existing() {
        let id = NodeId::new();
        let mut index = SpatialIndex::new();
        index.insert(entry(id, 0.0, 0.0, 10.0, 10.0));
        assert_eq!(index.len(), 1);

        let removed = index.remove(id);
        assert!(removed);
        assert!(index.is_empty());
    }

    #[test]
    fn remove_nonexistent() {
        let mut index = SpatialIndex::new();
        index.insert(entry_at(0.0, 0.0, 10.0, 10.0));
        let removed = index.remove(NodeId::new());
        assert!(!removed);
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn update_node_changes_position() {
        let id = NodeId::new();
        let mut index = SpatialIndex::new();
        index.insert(entry(id, 0.0, 0.0, 10.0, 10.0));

        // Move the node far away.
        index.update_node(id, &BoundingBox::new(1000.0, 1000.0, 10.0, 10.0));

        // Old position should yield no results.
        let at_origin = index.query_point(5.0, 5.0);
        assert!(at_origin.is_empty(), "Node should not be at old position");

        // New position should yield the node.
        let at_new = index.query_point(1005.0, 1005.0);
        assert_eq!(at_new.len(), 1);
        assert_eq!(at_new[0], id);
    }

    #[test]
    fn update_node_for_new_node() {
        // update_node should work even if the node doesn't exist yet.
        let id = NodeId::new();
        let mut index = SpatialIndex::new();
        index.update_node(id, &BoundingBox::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn clear_empties_index() {
        let mut index = SpatialIndex::new();
        for _ in 0..100 {
            index.insert(entry_at(0.0, 0.0, 10.0, 10.0));
        }
        assert_eq!(index.len(), 100);
        index.clear();
        assert!(index.is_empty());
    }

    // --- Bulk loading ---

    #[test]
    fn bulk_load_creates_index() {
        let entries: Vec<_> = (0..1000)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let x = (i % 100) as f32 * 10.0;
                #[allow(clippy::cast_precision_loss)]
                let y = (i / 100) as f32 * 10.0;
                entry_at(x, y, 8.0, 8.0)
            })
            .collect();

        let index = SpatialIndex::bulk_load(entries);
        assert_eq!(index.len(), 1000);
    }

    #[test]
    fn rebuild_replaces_tree() {
        let mut index = SpatialIndex::new();
        index.insert(entry_at(0.0, 0.0, 10.0, 10.0));
        assert_eq!(index.len(), 1);

        let entries: Vec<_> = (0..50).map(|_| entry_at(0.0, 0.0, 5.0, 5.0)).collect();
        index.rebuild(entries);
        assert_eq!(index.len(), 50);
    }

    // --- Viewport queries ---

    #[test]
    fn query_viewport_finds_visible_nodes() {
        let mut index = SpatialIndex::new();
        index.set_overscan(0.0); // Disable overscan for precise testing.

        // Node inside viewport.
        let inside = NodeId::new();
        index.insert(entry(inside, 50.0, 50.0, 100.0, 100.0));

        // Node outside viewport.
        let outside = NodeId::new();
        index.insert(entry(outside, 500.0, 500.0, 100.0, 100.0));

        // Viewport: (0, 0) to (200, 200).
        let results = index.query_viewport(0.0, 0.0, 200.0, 200.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], inside);
    }

    #[test]
    fn query_viewport_includes_partially_visible() {
        let mut index = SpatialIndex::new();
        index.set_overscan(0.0);

        // Node partially overlapping the viewport edge.
        let id = NodeId::new();
        index.insert(entry(id, 180.0, 50.0, 100.0, 50.0));

        let results = index.query_viewport(0.0, 0.0, 200.0, 200.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }

    #[test]
    fn query_viewport_with_overscan() {
        let mut index = SpatialIndex::new();
        index.set_overscan(0.2); // 20% overscan.

        // Node just outside the viewport but within overscan margin.
        // Viewport: (0, 0) to (100, 100). Width/height = 100.
        // Overscan: 20% of 100 = 20px on each side.
        // Expanded viewport: (-20, -20) to (120, 120).
        let id = NodeId::new();
        index.insert(entry(id, 105.0, 50.0, 10.0, 10.0)); // At x=105, within expanded rect.

        let results = index.query_viewport(0.0, 0.0, 100.0, 100.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }

    #[test]
    fn query_viewport_overscan_does_not_reach_far_nodes() {
        let mut index = SpatialIndex::new();
        index.set_overscan(0.2);

        // Node far outside viewport + overscan.
        let id = NodeId::new();
        index.insert(entry(id, 500.0, 500.0, 10.0, 10.0));

        let results = index.query_viewport(0.0, 0.0, 100.0, 100.0);
        assert!(results.is_empty());
    }

    #[test]
    fn query_viewport_empty_index() {
        let index = SpatialIndex::new();
        let results = index.query_viewport(0.0, 0.0, 1000.0, 1000.0);
        assert!(results.is_empty());
    }

    // --- Point queries (hit testing) ---

    #[test]
    fn query_point_finds_containing_node() {
        let mut index = SpatialIndex::new();
        let id = NodeId::new();
        index.insert(entry(id, 10.0, 10.0, 50.0, 50.0));

        let results = index.query_point(30.0, 30.0); // Center of the node.
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }

    #[test]
    fn query_point_at_corner() {
        let mut index = SpatialIndex::new();
        let id = NodeId::new();
        index.insert(entry(id, 0.0, 0.0, 100.0, 100.0));

        // Point at top-left corner (inclusive).
        let results = index.query_point(0.0, 0.0);
        assert_eq!(results.len(), 1);

        // Point at bottom-right corner (inclusive).
        let results = index.query_point(100.0, 100.0);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn query_point_miss() {
        let mut index = SpatialIndex::new();
        index.insert(entry_at(10.0, 10.0, 50.0, 50.0));

        let results = index.query_point(0.0, 0.0); // Outside the node.
        assert!(results.is_empty());
    }

    #[test]
    fn query_point_overlapping_nodes() {
        let mut index = SpatialIndex::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        // Three overlapping nodes.
        index.insert(entry(id1, 0.0, 0.0, 100.0, 100.0));
        index.insert(entry(id2, 25.0, 25.0, 50.0, 50.0));
        index.insert(entry(id3, 40.0, 40.0, 20.0, 20.0));

        // Point inside all three.
        let results = index.query_point(50.0, 50.0);
        assert_eq!(results.len(), 3);
        assert!(results.contains(&id1));
        assert!(results.contains(&id2));
        assert!(results.contains(&id3));
    }

    #[test]
    fn query_point_zero_size_node() {
        let mut index = SpatialIndex::new();
        // Zero-size bounding box: the R-tree entry is a degenerate point.
        let id = NodeId::new();
        index.insert(entry(id, 50.0, 50.0, 0.0, 0.0));

        // Query at that exact point should find it.
        let results = index.query_point(50.0, 50.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);

        // Query elsewhere should miss.
        let results = index.query_point(50.1, 50.0);
        assert!(results.is_empty());
    }

    // --- Region queries ---

    #[test]
    fn query_region_finds_intersecting_nodes() {
        let mut index = SpatialIndex::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        index.insert(entry(id1, 0.0, 0.0, 50.0, 50.0));
        index.insert(entry(id2, 40.0, 40.0, 50.0, 50.0)); // Overlaps region.
        index.insert(entry(id3, 200.0, 200.0, 50.0, 50.0)); // Far away.

        // Region covering (30, 30) to (60, 60).
        let results = index.query_region(30.0, 30.0, 60.0, 60.0);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&id1));
        assert!(results.contains(&id2));
        assert!(!results.contains(&id3));
    }

    #[test]
    fn query_region_no_results() {
        let mut index = SpatialIndex::new();
        index.insert(entry_at(0.0, 0.0, 10.0, 10.0));

        let results = index.query_region(100.0, 100.0, 200.0, 200.0);
        assert!(results.is_empty());
    }

    #[test]
    fn query_region_contains_entire_node() {
        let mut index = SpatialIndex::new();
        let id = NodeId::new();
        index.insert(entry(id, 20.0, 20.0, 10.0, 10.0));

        // Region that fully contains the node.
        let results = index.query_region(0.0, 0.0, 100.0, 100.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], id);
    }

    // --- Nearest neighbor ---

    #[test]
    fn nearest_to_point_finds_closest() {
        let mut index = SpatialIndex::new();
        let near = NodeId::new();
        let far = NodeId::new();

        index.insert(entry(near, 10.0, 10.0, 5.0, 5.0));
        index.insert(entry(far, 100.0, 100.0, 5.0, 5.0));

        let result = index.nearest_to_point(0.0, 0.0);
        assert_eq!(result, Some(near));
    }

    #[test]
    fn nearest_to_point_empty_index() {
        let index = SpatialIndex::new();
        assert_eq!(index.nearest_to_point(0.0, 0.0), None);
    }

    // --- Overscan configuration ---

    #[test]
    fn default_overscan_is_20_percent() {
        let index = SpatialIndex::new();
        assert_eq!(index.overscan(), DEFAULT_OVERSCAN);
        assert_eq!(index.overscan(), 0.2);
    }

    #[test]
    fn set_overscan_clamps_to_range() {
        let mut index = SpatialIndex::new();

        index.set_overscan(-0.5);
        assert_eq!(index.overscan(), 0.0);

        index.set_overscan(2.0);
        assert_eq!(index.overscan(), 1.0);

        index.set_overscan(0.5);
        assert_eq!(index.overscan(), 0.5);
    }

    #[test]
    fn zero_overscan_is_exact_viewport() {
        let mut index = SpatialIndex::new();
        index.set_overscan(0.0);

        // Node just outside viewport.
        let id = NodeId::new();
        index.insert(entry(id, 101.0, 50.0, 10.0, 10.0));

        let results = index.query_viewport(0.0, 0.0, 100.0, 100.0);
        assert!(
            results.is_empty(),
            "Node outside viewport should not be found with 0 overscan"
        );
    }

    // --- Debug formatting ---

    #[test]
    fn debug_format() {
        let index = SpatialIndex::new();
        let debug = format!("{index:?}");
        assert!(debug.contains("SpatialIndex"));
        assert!(debug.contains("entries"));
        assert!(debug.contains("overscan"));
    }

    // --- Large-scale test ---

    #[test]
    fn bulk_load_and_query_10k_nodes() {
        // Simulate a grid of 10K nodes (100x100 grid).
        let entries: Vec<_> = (0..10_000)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let x = (i % 100) as f32 * 20.0;
                #[allow(clippy::cast_precision_loss)]
                let y = (i / 100) as f32 * 20.0;
                entry_at(x, y, 15.0, 15.0)
            })
            .collect();

        let index = SpatialIndex::bulk_load(entries);
        assert_eq!(index.len(), 10_000);

        // Query a viewport that covers roughly the top-left quarter.
        // Grid goes from (0,0) to (1980+15, 1980+15) ≈ (1995, 1995).
        // Query (0, 0) to (500, 500) should find nodes in the first 25 columns
        // and first 25 rows = ~625 nodes.
        let visible = index.query_region(0.0, 0.0, 500.0, 500.0);
        assert!(
            visible.len() > 500 && visible.len() < 800,
            "Expected ~625 visible nodes, got {}",
            visible.len()
        );
    }

    // --- Consistency between incremental and bulk ---

    #[test]
    fn incremental_and_bulk_produce_same_results() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        let id3 = NodeId::new();

        let e1 = entry(id1, 0.0, 0.0, 50.0, 50.0);
        let e2 = entry(id2, 60.0, 60.0, 50.0, 50.0);
        let e3 = entry(id3, 120.0, 120.0, 50.0, 50.0);

        // Build via incremental insert.
        let mut incremental = SpatialIndex::new();
        incremental.set_overscan(0.0);
        incremental.insert(e1.clone());
        incremental.insert(e2.clone());
        incremental.insert(e3.clone());

        // Build via bulk load.
        let mut bulk = SpatialIndex::bulk_load(vec![e1, e2, e3]);
        bulk.set_overscan(0.0);

        // Same queries should return same results.
        let region = (0.0, 0.0, 100.0, 100.0);
        let mut inc_results = incremental.query_region(region.0, region.1, region.2, region.3);
        let mut bulk_results = bulk.query_region(region.0, region.1, region.2, region.3);
        inc_results.sort();
        bulk_results.sort();
        assert_eq!(inc_results, bulk_results);
    }
}
