//! Alignment and distribution computations for selected nodes.
//!
//! [`compute_alignment`] is a pure function that calculates new bounding boxes
//! without mutating the scene. The caller applies the results via commands.

use super::{BoundingBox, SceneGraph};
use selean_common::types::NodeId;

/// The kind of alignment or distribution to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentKind {
    /// Align left edges to the leftmost node.
    Left,
    /// Align right edges to the rightmost node.
    Right,
    /// Align top edges to the topmost node.
    Top,
    /// Align bottom edges to the bottommost node.
    Bottom,
    /// Align horizontal centers.
    CenterH,
    /// Align vertical centers.
    CenterV,
    /// Distribute nodes evenly along the horizontal axis.
    DistributeH,
    /// Distribute nodes evenly along the vertical axis.
    DistributeV,
}

impl AlignmentKind {
    /// Parses an alignment kind from a string label.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Left" => Some(Self::Left),
            "Right" => Some(Self::Right),
            "Top" => Some(Self::Top),
            "Bottom" => Some(Self::Bottom),
            "CenterH" => Some(Self::CenterH),
            "CenterV" => Some(Self::CenterV),
            "DistributeH" => Some(Self::DistributeH),
            "DistributeV" => Some(Self::DistributeV),
            _ => None,
        }
    }
}

/// The result of an alignment computation for a single node.
#[derive(Debug, Clone)]
pub struct AlignResult {
    /// The node whose bounds should change.
    pub node_id: NodeId,
    /// The new bounding box.
    pub new_bounds: BoundingBox,
}

/// Computes new bounding boxes for the given nodes after alignment.
///
/// Returns an empty vec if fewer than 2 nodes are provided (alignment
/// requires at least 2 nodes), or if any node ID is not found in the scene.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
pub fn compute_alignment(
    scene: &SceneGraph,
    node_ids: &[NodeId],
    kind: AlignmentKind,
) -> Vec<AlignResult> {
    if node_ids.len() < 2 {
        return vec![];
    }

    // Collect current bounds.
    let mut entries: Vec<(NodeId, BoundingBox)> = Vec::with_capacity(node_ids.len());
    for &id in node_ids {
        let Some(node) = scene.get(id) else {
            return vec![];
        };
        entries.push((id, node.bounds));
    }

    match kind {
        AlignmentKind::Left => {
            let min_x = entries
                .iter()
                .map(|(_, b)| b.x)
                .fold(f32::INFINITY, f32::min);
            entries
                .iter()
                .map(|(id, b)| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(min_x, b.y, b.width, b.height),
                })
                .collect()
        }
        AlignmentKind::Right => {
            let max_right = entries
                .iter()
                .map(|(_, b)| b.x + b.width)
                .fold(f32::NEG_INFINITY, f32::max);
            entries
                .iter()
                .map(|(id, b)| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(max_right - b.width, b.y, b.width, b.height),
                })
                .collect()
        }
        AlignmentKind::Top => {
            let min_y = entries
                .iter()
                .map(|(_, b)| b.y)
                .fold(f32::INFINITY, f32::min);
            entries
                .iter()
                .map(|(id, b)| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(b.x, min_y, b.width, b.height),
                })
                .collect()
        }
        AlignmentKind::Bottom => {
            let max_bottom = entries
                .iter()
                .map(|(_, b)| b.y + b.height)
                .fold(f32::NEG_INFINITY, f32::max);
            entries
                .iter()
                .map(|(id, b)| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(b.x, max_bottom - b.height, b.width, b.height),
                })
                .collect()
        }
        AlignmentKind::CenterH => {
            let min_x = entries
                .iter()
                .map(|(_, b)| b.x)
                .fold(f32::INFINITY, f32::min);
            let max_right = entries
                .iter()
                .map(|(_, b)| b.x + b.width)
                .fold(f32::NEG_INFINITY, f32::max);
            let center = f32::midpoint(min_x, max_right);
            entries
                .iter()
                .map(|(id, b)| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(center - b.width / 2.0, b.y, b.width, b.height),
                })
                .collect()
        }
        AlignmentKind::CenterV => {
            let min_y = entries
                .iter()
                .map(|(_, b)| b.y)
                .fold(f32::INFINITY, f32::min);
            let max_bottom = entries
                .iter()
                .map(|(_, b)| b.y + b.height)
                .fold(f32::NEG_INFINITY, f32::max);
            let center = f32::midpoint(min_y, max_bottom);
            entries
                .iter()
                .map(|(id, b)| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(b.x, center - b.height / 2.0, b.width, b.height),
                })
                .collect()
        }
        AlignmentKind::DistributeH => {
            if entries.len() < 3 {
                // With 2 nodes, distribution is a no-op.
                return entries
                    .iter()
                    .map(|(id, b)| AlignResult {
                        node_id: *id,
                        new_bounds: *b,
                    })
                    .collect();
            }
            // Sort by x position.
            entries.sort_by(|a, b| {
                a.1.x
                    .partial_cmp(&b.1.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let (Some(first), Some(last)) = (entries.first(), entries.last()) else {
                return vec![];
            };
            let first_x = first.1.x;
            let total_span = last.1.x - first_x;
            let step = total_span / (entries.len() - 1) as f32;
            entries
                .iter()
                .enumerate()
                .map(|(i, (id, b))| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(first_x + step * i as f32, b.y, b.width, b.height),
                })
                .collect()
        }
        AlignmentKind::DistributeV => {
            if entries.len() < 3 {
                return entries
                    .iter()
                    .map(|(id, b)| AlignResult {
                        node_id: *id,
                        new_bounds: *b,
                    })
                    .collect();
            }
            entries.sort_by(|a, b| {
                a.1.y
                    .partial_cmp(&b.1.y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let (Some(first), Some(last)) = (entries.first(), entries.last()) else {
                return vec![];
            };
            let first_y = first.1.y;
            let total_span = last.1.y - first_y;
            let step = total_span / (entries.len() - 1) as f32;
            entries
                .iter()
                .enumerate()
                .map(|(i, (id, b))| AlignResult {
                    node_id: *id,
                    new_bounds: BoundingBox::new(b.x, first_y + step * i as f32, b.width, b.height),
                })
                .collect()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::scene::{SceneNode, SceneNodeKind};

    fn make_scene_with_nodes(bounds_list: &[(f32, f32, f32, f32)]) -> (SceneGraph, Vec<NodeId>) {
        let mut scene = SceneGraph::new();
        let mut ids = Vec::new();
        for (i, &(x, y, w, h)) in bounds_list.iter().enumerate() {
            let node = SceneNode::new(
                NodeId::new(),
                format!("Node {i}"),
                SceneNodeKind::Frame {
                    corner_radius: [0.0; 4],
                },
                BoundingBox::new(x, y, w, h),
            );
            let id = node.id;
            scene.add_root(node);
            ids.push(id);
        }
        (scene, ids)
    }

    #[test]
    fn align_left() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (200.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::Left);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].new_bounds.x, 100.0);
        assert_eq!(results[1].new_bounds.x, 100.0);
        // Y and dimensions preserved
        assert_eq!(results[0].new_bounds.y, 50.0);
        assert_eq!(results[1].new_bounds.y, 80.0);
        assert_eq!(results[1].new_bounds.width, 60.0);
    }

    #[test]
    fn align_right() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (200.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::Right);
        assert_eq!(results.len(), 2);
        // max right = 200 + 60 = 260
        assert_eq!(results[0].new_bounds.x, 210.0); // 260 - 50
        assert_eq!(results[1].new_bounds.x, 200.0); // 260 - 60
    }

    #[test]
    fn align_top() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (200.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::Top);
        assert_eq!(results[0].new_bounds.y, 50.0);
        assert_eq!(results[1].new_bounds.y, 50.0);
    }

    #[test]
    fn align_bottom() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (200.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::Bottom);
        // max bottom = max(50+50, 80+40) = 120
        assert_eq!(results[0].new_bounds.y, 70.0); // 120 - 50
        assert_eq!(results[1].new_bounds.y, 80.0); // 120 - 40
    }

    #[test]
    fn align_center_h() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (200.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::CenterH);
        // center = (100 + 260) / 2 = 180
        assert_eq!(results[0].new_bounds.x, 155.0); // 180 - 25
        assert_eq!(results[1].new_bounds.x, 150.0); // 180 - 30
    }

    #[test]
    fn align_center_v() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (200.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::CenterV);
        // center = (50 + 120) / 2 = 85
        assert_eq!(results[0].new_bounds.y, 60.0); // 85 - 25
        assert_eq!(results[1].new_bounds.y, 65.0); // 85 - 20
    }

    #[test]
    fn distribute_h_three_nodes() {
        let (scene, ids) = make_scene_with_nodes(&[
            (100.0, 50.0, 50.0, 50.0),
            (300.0, 50.0, 50.0, 50.0),
            (500.0, 50.0, 50.0, 50.0),
        ]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::DistributeH);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].new_bounds.x, 100.0);
        assert_eq!(results[1].new_bounds.x, 300.0);
        assert_eq!(results[2].new_bounds.x, 500.0);
    }

    #[test]
    fn distribute_v_three_nodes() {
        let (scene, ids) = make_scene_with_nodes(&[
            (50.0, 100.0, 50.0, 50.0),
            (50.0, 300.0, 50.0, 50.0),
            (50.0, 500.0, 50.0, 50.0),
        ]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::DistributeV);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].new_bounds.y, 100.0);
        assert_eq!(results[1].new_bounds.y, 300.0);
        assert_eq!(results[2].new_bounds.y, 500.0);
    }

    #[test]
    fn single_node_returns_empty() {
        let (scene, ids) = make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::Left);
        assert!(results.is_empty());
    }

    #[test]
    fn zero_nodes_returns_empty() {
        let (scene, _) = make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0)]);
        let results = compute_alignment(&scene, &[], AlignmentKind::Left);
        assert!(results.is_empty());
    }

    #[test]
    fn already_aligned_left() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (100.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::Left);
        assert_eq!(results[0].new_bounds.x, 100.0);
        assert_eq!(results[1].new_bounds.x, 100.0);
    }

    #[test]
    fn dimensions_preserved_after_alignment() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 70.0), (200.0, 80.0, 60.0, 40.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::Left);
        assert_eq!(results[0].new_bounds.width, 50.0);
        assert_eq!(results[0].new_bounds.height, 70.0);
        assert_eq!(results[1].new_bounds.width, 60.0);
        assert_eq!(results[1].new_bounds.height, 40.0);
    }

    #[test]
    fn distribute_h_two_nodes_is_noop() {
        let (scene, ids) =
            make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0), (300.0, 50.0, 50.0, 50.0)]);
        let results = compute_alignment(&scene, &ids, AlignmentKind::DistributeH);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].new_bounds.x, 100.0);
        assert_eq!(results[1].new_bounds.x, 300.0);
    }

    #[test]
    fn missing_node_id_returns_empty() {
        let (scene, _) = make_scene_with_nodes(&[(100.0, 50.0, 50.0, 50.0)]);
        let fake_id = NodeId::new();
        let results = compute_alignment(&scene, &[fake_id, NodeId::new()], AlignmentKind::Left);
        assert!(results.is_empty());
    }

    #[test]
    fn alignment_kind_from_str() {
        assert_eq!(AlignmentKind::parse("Left"), Some(AlignmentKind::Left));
        assert_eq!(
            AlignmentKind::parse("DistributeV"),
            Some(AlignmentKind::DistributeV)
        );
        assert_eq!(AlignmentKind::parse("Invalid"), None);
    }
}
