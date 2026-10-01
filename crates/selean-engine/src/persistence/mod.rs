//! Document persistence: save and load scene graphs and documents as JSON.
//!
//! The `save()` / `load()` functions work with a single `SceneGraph` for
//! backward compatibility. The `save_document()` / `load_document()` functions
//! handle the full multi-page `Document` model.
//!
//! All save functions produce v2 format. `load_document()` accepts both v1
//! (single scene) and v2 (multi-page) formats transparently.

pub mod document;
pub mod format;
pub mod page;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use selean_common::types::PageId;

use crate::scene::SceneGraph;

pub use document::Document;
pub use format::{DocumentFormat, FORMAT_VERSION, PageData, SceneGraphData};
pub use page::Page;

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

/// Seconds since the Unix epoch, for the `saved_at` field.
#[cfg(not(target_arch = "wasm32"))]
fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Seconds since the Unix epoch, for the `saved_at` field.
///
/// `std::time::SystemTime::now()` panics on `wasm32-unknown-unknown`, so in
/// the browser the clock is read from JavaScript.
#[cfg(target_arch = "wasm32")]
fn unix_timestamp_secs() -> u64 {
    // `Date.now()` is a non-negative millisecond count far below `u64::MAX`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let secs = (js_sys::Date::now() / 1000.0) as u64;
    secs
}

/// Serializes a scene graph to a JSON string.
///
/// The graph is wrapped in a single-page v2 document for forward compatibility.
/// The output includes a format version and timestamp for compatibility
/// checking on load. The JSON is not pretty-printed; use `save_pretty()`
/// if human readability is needed.
///
/// # Errors
///
/// Returns `PersistenceError::Json` if serialization fails.
pub fn save(graph: &SceneGraph) -> Result<String, PersistenceError> {
    let saved_at = unix_timestamp_secs();

    let doc = DocumentFormat {
        version: FORMAT_VERSION,
        saved_at,
        scene: None,
        pages: Some(vec![PageData {
            id: PageId::new(),
            name: "Page 1".to_string(),
            width: 1920.0,
            height: 1080.0,
            scene: SceneGraphData::from_graph(graph),
        }]),
    };

    serde_json::to_string(&doc).map_err(PersistenceError::Json)
}

/// Serializes a scene graph to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns `PersistenceError::Json` if serialization fails.
pub fn save_pretty(graph: &SceneGraph) -> Result<String, PersistenceError> {
    let saved_at = unix_timestamp_secs();

    let doc = DocumentFormat {
        version: FORMAT_VERSION,
        saved_at,
        scene: None,
        pages: Some(vec![PageData {
            id: PageId::new(),
            name: "Page 1".to_string(),
            width: 1920.0,
            height: 1080.0,
            scene: SceneGraphData::from_graph(graph),
        }]),
    };

    serde_json::to_string_pretty(&doc).map_err(PersistenceError::Json)
}

/// Deserializes a JSON string into a live `SceneGraph`.
///
/// Supports both v1 (single scene) and v2 (multi-page) formats.
/// For v2 documents, returns the active (first) page's scene graph.
///
/// # Errors
///
/// Returns `PersistenceError::Json` on parse failure,
/// `PersistenceError::UnsupportedVersion` if the format version is newer than supported,
/// or `PersistenceError::InvalidScene` if structural validation fails.
pub fn load(json: &str) -> Result<SceneGraph, PersistenceError> {
    let doc = load_document(json)?;
    Ok(doc.active_page().scene.clone())
}

/// Serializes a document to a JSON string.
///
/// # Errors
///
/// Returns `PersistenceError::Json` if serialization fails.
pub fn save_document(doc: &Document) -> Result<String, PersistenceError> {
    let saved_at = unix_timestamp_secs();

    let pages: Vec<PageData> = doc.pages().iter().map(PageData::from_page).collect();

    let doc_format = DocumentFormat {
        version: FORMAT_VERSION,
        saved_at,
        scene: None,
        pages: Some(pages),
    };

    serde_json::to_string(&doc_format).map_err(PersistenceError::Json)
}

/// Deserializes a JSON string into a `Document`.
///
/// Supports both v1 (single scene) and v2 (multi-page) formats.
/// A v1 document is migrated to a single-page `Document` transparently.
///
/// # Errors
///
/// Returns `PersistenceError::Json` on parse failure,
/// `PersistenceError::UnsupportedVersion` if the format version is newer than supported,
/// or `PersistenceError::InvalidScene` if structural validation fails.
pub fn load_document(json: &str) -> Result<Document, PersistenceError> {
    let doc_format: DocumentFormat = serde_json::from_str(json)?;

    if doc_format.version > FORMAT_VERSION {
        return Err(PersistenceError::UnsupportedVersion {
            found: doc_format.version,
            expected: FORMAT_VERSION,
        });
    }

    doc_format
        .validate()
        .map_err(PersistenceError::InvalidScene)?;

    if let Some(pages_data) = doc_format.pages {
        // v2 format
        let pages: Vec<Page> = pages_data.into_iter().map(PageData::into_page).collect();
        if pages.is_empty() {
            return Err(PersistenceError::InvalidScene(
                "document has no pages".to_string(),
            ));
        }
        Ok(Document::from_pages(pages))
    } else if let Some(scene_data) = doc_format.scene {
        // v1 backward compat: wrap single scene in one page
        let graph = scene_data.into_graph();
        let page = Page::with_scene(PageId::new(), "Page 1", 1920.0, 1080.0, graph);
        Ok(Document::from_pages(vec![page]))
    } else {
        Err(PersistenceError::InvalidScene(
            "document has neither pages nor scene".to_string(),
        ))
    }
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
        BlendMode, BoundingBox, ClipMode, Color, FontStyle, SceneGraph, SceneNode, SceneNodeKind,
        TextAlign, Transform2D,
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
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
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
                    expected: 2
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
        // Valid JSON, valid version, but orphan node in a page.
        let mut nodes_map = std::collections::HashMap::new();
        let orphan = make_node("Orphan");
        let oid = orphan.id;
        nodes_map.insert(oid, orphan);

        let doc = DocumentFormat {
            version: FORMAT_VERSION,
            saved_at: 0,
            scene: None,
            pages: Some(vec![PageData {
                id: PageId::new(),
                name: "Bad Page".to_string(),
                width: 1920.0,
                height: 1080.0,
                scene: SceneGraphData {
                    nodes: nodes_map,
                    roots: vec![],
                },
            }]),
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
    fn load_rejects_missing_scene_and_pages() {
        // v2 allows scene and pages to be absent, but load should reject
        // a document that has neither.
        let json = r#"{"version":2,"saved_at":0}"#;
        let err = load(json).unwrap_err();
        assert!(
            matches!(err, PersistenceError::InvalidScene(_)),
            "got: {err}"
        );
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

    // --- Document save/load tests ---

    #[test]
    fn save_load_document_roundtrip() {
        let mut doc = Document::new();
        doc.active_page_mut().scene.add_root(make_node("Node1"));
        let page2_id = doc.add_page("Page 2", 800.0, 600.0);
        doc.page_mut(page2_id)
            .unwrap()
            .scene
            .add_root(make_node("Node2"));

        let json = save_document(&doc).expect("save");
        let restored = load_document(&json).expect("load");

        assert_eq!(restored.page_count(), 2);
        assert_eq!(restored.active_page().scene.len(), 1);
        assert_eq!(restored.page(page2_id).unwrap().scene.len(), 1);
    }

    #[test]
    fn load_v1_format_as_single_page() {
        // Simulate a v1 format document.
        let mut graph = SceneGraph::new();
        graph.add_root(make_node("OldNode"));

        let v1_json = serde_json::json!({
            "version": 1,
            "saved_at": 0,
            "scene": SceneGraphData::from_graph(&graph),
        })
        .to_string();

        let doc = load_document(&v1_json).expect("load v1");
        assert_eq!(doc.page_count(), 1);
        assert_eq!(doc.active_page().scene.len(), 1);
        assert_eq!(doc.active_page().name, "Page 1");
    }

    #[test]
    fn save_document_produces_v2_format() {
        let doc = Document::new();
        let json = save_document(&doc).expect("save");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed["version"], FORMAT_VERSION);
        assert!(parsed["pages"].is_array());
        assert_eq!(parsed["pages"].as_array().unwrap().len(), 1);
        // v2 should not have a top-level scene field.
        assert!(parsed.get("scene").is_none());
    }

    #[test]
    fn load_document_rejects_unsupported_version() {
        let json = r#"{"version":999,"saved_at":0}"#;
        let err = load_document(json).unwrap_err();
        assert!(
            matches!(
                err,
                PersistenceError::UnsupportedVersion {
                    found: 999,
                    expected: 2
                }
            ),
            "got: {err}"
        );
    }

    #[test]
    fn load_preserves_page_dimensions() {
        let mut doc = Document::new();
        let id = doc.add_page("Wide", 3840.0, 2160.0);

        let json = save_document(&doc).expect("save");
        let restored = load_document(&json).expect("load");

        let page = restored.page(id).unwrap();
        assert_eq!(page.width, 3840.0);
        assert_eq!(page.height, 2160.0);
        assert_eq!(page.name, "Wide");
    }

    #[test]
    fn load_via_save_graph_roundtrip() {
        // save() wraps in v2 format, load() should still work.
        let mut graph = SceneGraph::new();
        let id = graph.add_root(make_node("Test"));

        let json = save(&graph).expect("save");
        let restored = load(&json).expect("load");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored.get(id).unwrap().name, "Test");
    }
}
