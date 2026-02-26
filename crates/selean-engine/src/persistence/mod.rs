//! Document persistence: save and load scene graphs as JSON.
//!
//! The `save()` function serializes a `SceneGraph` to a JSON string.
//! The `load()` function deserializes and validates the JSON, rebuilding
//! transient state (spatial index, world transforms, dirty flags).

pub mod format;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::scene::SceneGraph;

pub use format::{DocumentFormat, FORMAT_VERSION, SceneGraphData};

/// Errors that can occur during save or load.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    /// The document was saved with a newer format version than we support.
    #[error("unsupported format version {found}, expected {expected}")]
    UnsupportedVersion {
        /// The version found in the document.
        found: u32,
        /// The version this build supports.
        expected: u32,
    },

    /// The scene graph data failed structural validation.
    #[error("invalid scene: {0}")]
    InvalidScene(String),

    /// JSON serialization or deserialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Serializes a scene graph to a JSON string.
///
/// The output includes a format version and timestamp for compatibility
/// checking on load. The JSON is not pretty-printed; use `save_pretty()`
/// if human readability is needed.
///
/// # Errors
///
/// Returns `PersistenceError::Json` if serialization fails.
pub fn save(graph: &SceneGraph) -> Result<String, PersistenceError> {
    let saved_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let doc = DocumentFormat {
        version: FORMAT_VERSION,
        saved_at,
        scene: SceneGraphData::from_graph(graph),
    };

    serde_json::to_string(&doc).map_err(PersistenceError::Json)
}

/// Serializes a scene graph to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns `PersistenceError::Json` if serialization fails.
pub fn save_pretty(graph: &SceneGraph) -> Result<String, PersistenceError> {
    let saved_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let doc = DocumentFormat {
        version: FORMAT_VERSION,
        saved_at,
        scene: SceneGraphData::from_graph(graph),
    };

    serde_json::to_string_pretty(&doc).map_err(PersistenceError::Json)
}

/// Deserializes a JSON string into a live `SceneGraph`.
///
/// Validates the format version and structural integrity before rebuilding
/// transient state (spatial index, world transforms).
///
/// # Errors
///
/// Returns `PersistenceError::Json` on parse failure,
/// `PersistenceError::UnsupportedVersion` if the format version doesn't match,
/// or `PersistenceError::InvalidScene` if structural validation fails.
pub fn load(json: &str) -> Result<SceneGraph, PersistenceError> {
    let doc: DocumentFormat = serde_json::from_str(json)?;

    if doc.version != FORMAT_VERSION {
        return Err(PersistenceError::UnsupportedVersion {
            found: doc.version,
            expected: FORMAT_VERSION,
        });
    }

    doc.scene
        .validate()
        .map_err(PersistenceError::InvalidScene)?;

    Ok(doc.scene.into_graph())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::float_cmp,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::cast_precision_loss,
        clippy::similar_names,
        clippy::items_after_statements,
        clippy::unreadable_literal
    )]

    use super::*;
    use crate::scene::{
        BlendMode, BoundingBox, ClipMode, Color, SceneGraph, SceneNode, SceneNodeKind, Transform2D,
    };
    use selean_common::types::NodeId;

    fn make_node(name: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        )
    }

    // --- Save/load roundtrip tests ---

    #[test]
    fn save_load_empty_graph() {
        let graph = SceneGraph::new();
        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");
        assert!(restored.is_empty());
        assert!(restored.roots().is_empty());
    }

    #[test]
    fn save_load_single_node() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("Solo");
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        node.opacity = 0.75;
        let id = graph.add_root(node);

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        assert_eq!(restored.len(), 1);
        let n = restored.get(id).unwrap();
        assert_eq!(n.name, "Solo");
        assert_eq!(n.fill.unwrap().r, 1.0);
        assert_eq!(n.opacity, 0.75);
    }

    #[test]
    fn save_load_hierarchy() {
        let mut graph = SceneGraph::new();
        let root = make_node("Root");
        let rid = graph.add_root(root);
        let child = make_node("Child");
        let cid = child.id;
        graph.add_child(rid, child);
        let grandchild = make_node("Grandchild");
        let gcid = grandchild.id;
        graph.add_child(cid, grandchild);

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        assert_eq!(restored.len(), 3);
        assert_eq!(restored.roots(), &[rid]);
        assert_eq!(restored.children(rid).unwrap(), &[cid]);
        assert_eq!(restored.children(cid).unwrap(), &[gcid]);
        assert_eq!(restored.parent(cid), Some(rid));
        assert_eq!(restored.parent(gcid), Some(cid));
    }

    #[test]
    fn save_load_multiple_roots() {
        let mut graph = SceneGraph::new();
        let r1 = graph.add_root(make_node("R1"));
        let r2 = graph.add_root(make_node("R2"));
        let r3 = graph.add_root(make_node("R3"));

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        assert_eq!(restored.roots(), &[r1, r2, r3]);
    }

    #[test]
    fn save_load_all_node_kinds() {
        let mut graph = SceneGraph::new();

        let frame = make_node("Frame");
        let fid = graph.add_root(frame);

        let text = SceneNode::new(
            NodeId::new(),
            "Text".to_string(),
            SceneNodeKind::Text {
                content: "hello".to_string(),
                font_size: 14.0,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 20.0),
        );
        let tid = graph.add_root(text);

        let image = SceneNode::new(
            NodeId::new(),
            "Image".to_string(),
            SceneNodeKind::Image {
                asset_ref: "img.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 64.0, 64.0),
        );
        let iid = graph.add_root(image);

        let vector = SceneNode::new(
            NodeId::new(),
            "Vector".to_string(),
            SceneNodeKind::Vector {
                path_data: "M0 0 L10 10".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 10.0, 10.0),
        );
        let vid = graph.add_root(vector);

        let group = SceneNode::new(
            NodeId::new(),
            "Group".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 0.0, 0.0),
        );
        let gid = graph.add_root(group);

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        assert_eq!(restored.len(), 5);
        assert!(matches!(
            restored.get(fid).unwrap().kind,
            SceneNodeKind::Frame { .. }
        ));
        assert!(matches!(
            restored.get(tid).unwrap().kind,
            SceneNodeKind::Text { .. }
        ));
        assert!(matches!(
            restored.get(iid).unwrap().kind,
            SceneNodeKind::Image { .. }
        ));
        assert!(matches!(
            restored.get(vid).unwrap().kind,
            SceneNodeKind::Vector { .. }
        ));
        assert!(matches!(
            restored.get(gid).unwrap().kind,
            SceneNodeKind::Group
        ));
    }

    #[test]
    fn save_load_transform() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("Transformed");
        node.local_transform = Transform2D::from_raw([0.5, 0.1, -0.1, 0.5, 100.0, 200.0]);
        let id = graph.add_root(node);

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        let n = restored.get(id).unwrap();
        assert_eq!(
            *n.local_transform.raw(),
            [0.5, 0.1, -0.1, 0.5, 100.0, 200.0]
        );
    }

    #[test]
    fn save_load_blend_mode_and_clip() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("BlendClip");
        node.blend_mode = BlendMode::Multiply;
        node.clip_mode = ClipMode::Stencil;
        let id = graph.add_root(node);

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        let n = restored.get(id).unwrap();
        assert_eq!(n.blend_mode, BlendMode::Multiply);
        assert_eq!(n.clip_mode, ClipMode::Stencil);
    }

    #[test]
    fn save_load_scroll_offset() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("Scroller");
        node.scroll_offset = [55.5, 77.3];
        let id = graph.add_root(node);

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        assert_eq!(restored.get(id).unwrap().scroll_offset, [55.5, 77.3]);
    }

    #[test]
    fn save_load_stroke_properties() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("Stroked");
        node.stroke = Some(Color::new(0.0, 0.0, 1.0, 1.0));
        node.stroke_width = 3.5;
        let id = graph.add_root(node);

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        let n = restored.get(id).unwrap();
        assert_eq!(n.stroke.unwrap().b, 1.0);
        assert_eq!(n.stroke_width, 3.5);
    }

    // --- Field omission checks (serde skip) ---

    #[test]
    fn dirty_flags_reset_on_load() {
        let mut graph = SceneGraph::new();
        let node = make_node("Clean");
        let id = graph.add_root(node);
        // Clear dirty on original to verify load resets to ALL.
        graph.get_mut(id).unwrap().clear_dirty();

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        use crate::scene::DirtyFlags;
        let n = restored.get(id).unwrap();
        // Serde default sets ALL, but from_document_state calls
        // recompute_world_transforms which clears TRANSFORM.
        let expected = DirtyFlags::ALL.without(DirtyFlags::TRANSFORM);
        assert!(
            n.dirty.contains(expected),
            "expected all flags except TRANSFORM on load, got {:?}",
            n.dirty
        );
        assert!(
            !n.dirty.contains(DirtyFlags::TRANSFORM),
            "TRANSFORM should be cleared after world transform recomputation"
        );
    }

    #[test]
    fn world_transform_recomputed_on_load() {
        let mut graph = SceneGraph::new();
        let mut parent = make_node("Parent");
        parent.local_transform = Transform2D::from_raw([1.0, 0.0, 0.0, 1.0, 50.0, 0.0]);
        let pid = graph.add_root(parent);

        let mut child = make_node("Child");
        child.local_transform = Transform2D::from_raw([1.0, 0.0, 0.0, 1.0, 0.0, 30.0]);
        let cid = child.id;
        graph.add_child(pid, child);
        graph.recompute_world_transforms();

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        // Child world transform should be parent * child = translate(50, 30).
        let wt = restored.get(cid).unwrap().world_transform.raw();
        assert_eq!(wt[4], 50.0, "tx");
        assert_eq!(wt[5], 30.0, "ty");
    }

    #[test]
    fn spatial_index_rebuilt_on_load() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("Spatial");
        node.bounds = BoundingBox::new(10.0, 20.0, 100.0, 50.0);
        let id = graph.add_root(node);
        graph.recompute_world_transforms();

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");

        // The spatial index should contain the node at its bounds.
        let hits = restored.spatial().query_viewport(0.0, 0.0, 200.0, 200.0);
        assert!(hits.contains(&id), "spatial index should contain the node");
    }

    // --- Rejection tests ---

    #[test]
    fn load_rejects_unsupported_version() {
        let json = r#"{"version":999,"saved_at":0,"scene":{"nodes":{},"roots":[]}}"#;
        let err = load(json).unwrap_err();
        assert!(
            matches!(
                err,
                PersistenceError::UnsupportedVersion {
                    found: 999,
                    expected: 1
                }
            ),
            "got: {err}"
        );
    }

    #[test]
    fn load_rejects_invalid_json() {
        let err = load("not json").unwrap_err();
        assert!(matches!(err, PersistenceError::Json(_)), "got: {err}");
    }

    #[test]
    fn load_rejects_invalid_scene() {
        // Valid JSON, valid version, but orphan node.
        let mut nodes_map = std::collections::HashMap::new();
        let orphan = make_node("Orphan");
        let oid = orphan.id;
        nodes_map.insert(oid, orphan);

        let doc = DocumentFormat {
            version: FORMAT_VERSION,
            saved_at: 0,
            scene: SceneGraphData {
                nodes: nodes_map,
                roots: vec![],
            },
        };
        let json = serde_json::to_string(&doc).expect("serialize");
        let err = load(&json).unwrap_err();
        assert!(
            matches!(err, PersistenceError::InvalidScene(_)),
            "got: {err}"
        );
    }

    #[test]
    fn save_produces_valid_json() {
        let mut graph = SceneGraph::new();
        graph.add_root(make_node("A"));
        let json = save(&graph).expect("save");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed["version"], FORMAT_VERSION);
        assert!(parsed["saved_at"].as_u64().unwrap() > 0);
    }

    #[test]
    fn save_pretty_is_multiline() {
        let mut graph = SceneGraph::new();
        graph.add_root(make_node("A"));
        let json = save_pretty(&graph).expect("save_pretty");
        assert!(json.contains('\n'), "pretty output should be multiline");
    }

    #[test]
    fn load_rejects_missing_fields() {
        let json = r#"{"version":1,"saved_at":0}"#;
        let err = load(json).unwrap_err();
        assert!(matches!(err, PersistenceError::Json(_)), "got: {err}");
    }

    #[test]
    fn roundtrip_large_scene() {
        let mut graph = SceneGraph::new();
        let root = make_node("Root");
        let rid = graph.add_root(root);

        for i in 0..50 {
            let mut child = make_node(&format!("Child{i}"));
            child.bounds = BoundingBox::new(i as f32 * 10.0, 0.0, 50.0, 50.0);
            child.fill = Some(Color::new(0.5, 0.5, 0.5, 1.0));
            graph.add_child(rid, child);
        }

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");
        assert_eq!(restored.len(), 51);
    }
}
