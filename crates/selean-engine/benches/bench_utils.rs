//! Deterministic synthetic scene generator for benchmarks.
//!
//! Produces reproducible scene graphs with configurable node counts and type
//! distributions. Uses a seeded RNG so benchmark runs are comparable.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names,
    missing_docs
)]

use rand::prelude::*;
use rand::rngs::StdRng;

use selean_common::types::NodeId;
use selean_engine::scene::{BoundingBox, Color, SceneGraph, SceneNode, SceneNodeKind};

/// Distribution of node types in the synthetic scene.
#[derive(Debug, Clone, Copy)]
pub struct TypeDistribution {
    /// Fraction of nodes that are frames (0.0–1.0).
    pub frames: f32,
    /// Fraction of nodes that are text (0.0–1.0).
    pub text: f32,
    /// Fraction of nodes that are groups (0.0–1.0).
    pub groups: f32,
    /// Fraction of nodes that are images (0.0–1.0).
    pub images: f32,
}

impl Default for TypeDistribution {
    fn default() -> Self {
        Self {
            frames: 0.60,
            text: 0.25,
            groups: 0.10,
            images: 0.05,
        }
    }
}

/// Configuration for synthetic scene generation.
#[derive(Debug, Clone)]
pub struct SceneConfig {
    /// Total number of nodes to generate.
    pub node_count: usize,
    /// Type distribution (fractions must sum to ~1.0).
    pub distribution: TypeDistribution,
    /// RNG seed for reproducibility.
    pub seed: u64,
    /// Canvas width in logical pixels. Nodes are placed within this area.
    pub canvas_width: f32,
    /// Canvas height in logical pixels.
    pub canvas_height: f32,
    /// Maximum nesting depth for groups.
    pub max_depth: u32,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            node_count: 10_000,
            distribution: TypeDistribution::default(),
            seed: 42,
            canvas_width: 10_000.0,
            canvas_height: 10_000.0,
            max_depth: 4,
        }
    }
}

impl SceneConfig {
    /// Creates a config with a specific node count, using defaults for everything else.
    pub fn with_count(node_count: usize) -> Self {
        Self {
            node_count,
            ..Self::default()
        }
    }
}

/// Generates a deterministic synthetic scene graph.
///
/// Groups are created first as root nodes, then leaf nodes are distributed
/// among them (and some as additional roots) to create a realistic hierarchy
/// 3-4 levels deep.
pub fn generate_scene(config: &SceneConfig) -> SceneGraph {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut graph = SceneGraph::new();
    let dist = &config.distribution;

    // Compute node counts per type.
    let group_count = (config.node_count as f32 * dist.groups).round() as usize;
    let frame_count = (config.node_count as f32 * dist.frames).round() as usize;
    let text_count = (config.node_count as f32 * dist.text).round() as usize;
    let image_count = (config.node_count as f32 * dist.images).round() as usize;

    // Phase 1: Create group hierarchy (nesting up to max_depth).
    let mut group_ids: Vec<(NodeId, u32)> = Vec::with_capacity(group_count);
    for i in 0..group_count {
        let bounds = random_bounds(&mut rng, config.canvas_width, config.canvas_height, true);
        let node = SceneNode::new(
            NodeId::new(),
            format!("Group-{i}"),
            SceneNodeKind::Group,
            bounds,
        );
        let node_id = node.id;

        // First group is always a root. Others may nest under an existing group.
        if group_ids.is_empty() || rng.gen_bool(0.3) {
            graph.add_root(node);
            group_ids.push((node_id, 0));
        } else {
            // Pick a random existing group that isn't too deep.
            let eligible: Vec<usize> = group_ids
                .iter()
                .enumerate()
                .filter(|(_, (_, depth))| *depth < config.max_depth)
                .map(|(idx, _)| idx)
                .collect();

            if eligible.is_empty() {
                graph.add_root(node);
                group_ids.push((node_id, 0));
            } else {
                let parent_idx = eligible[rng.gen_range(0..eligible.len())];
                let parent_depth = group_ids[parent_idx].1;
                let parent_id = group_ids[parent_idx].0;
                graph.add_child(parent_id, node);
                group_ids.push((node_id, parent_depth + 1));
            }
        }
    }

    // Phase 2: Create leaf nodes (frames, text, images) and distribute among groups.
    let mut leaf_kinds: Vec<SceneNodeKind> =
        Vec::with_capacity(frame_count + text_count + image_count);

    for _ in 0..frame_count {
        let radii: [f32; 4] = if rng.gen_bool(0.4) {
            let r = rng.gen_range(0.0_f32..20.0);
            [r, r, r, r]
        } else {
            [0.0; 4]
        };
        leaf_kinds.push(SceneNodeKind::Frame {
            corner_radius: radii,
        });
    }

    for i in 0..text_count {
        leaf_kinds.push(SceneNodeKind::Text {
            content: format!("Label {i}"),
            font_size: rng.gen_range(10.0_f32..48.0),
        });
    }

    for i in 0..image_count {
        leaf_kinds.push(SceneNodeKind::Image {
            asset_ref: format!("asset_{i}.png"),
        });
    }

    // Shuffle leaf kinds for realistic interleaving.
    leaf_kinds.shuffle(&mut rng);

    for (i, kind) in leaf_kinds.into_iter().enumerate() {
        let is_group_like = matches!(kind, SceneNodeKind::Frame { .. });
        let bounds = random_bounds(
            &mut rng,
            config.canvas_width,
            config.canvas_height,
            is_group_like,
        );

        let mut node = SceneNode::new(NodeId::new(), format!("Node-{i}"), kind, bounds);
        node.fill = Some(random_color(&mut rng));
        node.opacity = rng.gen_range(0.5_f32..1.0);

        if rng.gen_bool(0.3) {
            node.stroke = Some(random_color(&mut rng));
            node.stroke_width = rng.gen_range(1.0_f32..4.0);
        }

        // 80% of leaves go into a group, 20% are root-level.
        if !group_ids.is_empty() && rng.gen_bool(0.8) {
            let group_idx = rng.gen_range(0..group_ids.len());
            let parent_id = group_ids[group_idx].0;
            graph.add_child(parent_id, node);
        } else {
            graph.add_root(node);
        }
    }

    // Clear dirty flags so benchmarks start from a clean state.
    graph.clear_all_dirty();

    graph
}

/// Generates a random bounding box within the canvas.
fn random_bounds(rng: &mut StdRng, canvas_w: f32, canvas_h: f32, large: bool) -> BoundingBox {
    let (min_size, max_size) = if large { (100.0, 800.0) } else { (20.0, 200.0) };

    let w = rng.gen_range(min_size..max_size);
    let h = rng.gen_range(min_size..max_size);
    let x = rng.gen_range(0.0..(canvas_w - w).max(1.0));
    let y = rng.gen_range(0.0..(canvas_h - h).max(1.0));

    BoundingBox::new(x, y, w, h)
}

/// Generates a random RGBA color.
fn random_color(rng: &mut StdRng) -> Color {
    Color::new(
        rng.gen_range(0.0_f32..1.0),
        rng.gen_range(0.0_f32..1.0),
        rng.gen_range(0.0_f32..1.0),
        rng.gen_range(0.5_f32..1.0),
    )
}
