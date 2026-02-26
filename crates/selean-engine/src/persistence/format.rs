//! Document format types and validation for scene graph serialization.
//!
//! `DocumentFormat` is the top-level container written to disk, wrapping
//! `SceneGraphData` which holds the serializable subset of the scene graph.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use selean_common::types::NodeId;

use crate::scene::{SceneGraph, SceneNode};

/// Current format version. Incremented on breaking schema changes.
pub const FORMAT_VERSION: u32 = 1;

/// Top-level document format written to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFormat {
    /// Schema version for forward/backward compatibility checks.
    pub version: u32,
    /// Timestamp (seconds since UNIX epoch) when the document was saved.
    pub saved_at: u64,
    /// The scene graph data.
    pub scene: SceneGraphData,
}

/// Serializable subset of the scene graph: the node map and root ordering.
///
/// Transient state (world transforms, dirty flags, spatial index) is excluded
/// and rebuilt on load via `SceneGraph::from_document_state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneGraphData {
    /// All nodes keyed by their ID.
    pub nodes: HashMap<NodeId, SceneNode>,
    /// Root node IDs in render order.
    pub roots: Vec<NodeId>,
}

impl SceneGraphData {
    /// Extracts the serializable data from a live scene graph.
    #[must_use]
    pub fn from_graph(graph: &SceneGraph) -> Self {
        Self {
            nodes: graph.nodes().clone(),
            roots: graph.roots().to_vec(),
        }
    }

    /// Validates structural integrity of the deserialized data.
    ///
    /// Returns `Ok(())` if all invariants hold, or `Err` with a description
    /// of the first violation found.
    ///
    /// # Errors
    ///
    /// Returns a `String` describing the violation. Checks (in order):
    ///
    /// 1. Every root ID exists in the node map.
    /// 2. Every child ID referenced by a node exists in the node map.
    /// 3. Parent pointers are consistent with children lists.
    /// 4. Root nodes have `parent == None`.
    /// 5. No root ID appears in any node's children list.
    /// 6. No cycles (DFS from roots).
    /// 7. No orphans (every node is reachable from a root).
    pub fn validate(&self) -> Result<(), String> {
        let root_set: HashSet<NodeId> = self.roots.iter().copied().collect();

        // 1. Roots exist in node map.
        for &root_id in &self.roots {
            if !self.nodes.contains_key(&root_id) {
                return Err(format!("root {root_id} not found in node map"));
            }
        }

        // 2. Children exist in node map.
        for (id, node) in &self.nodes {
            for &child_id in &node.children {
                if !self.nodes.contains_key(&child_id) {
                    return Err(format!(
                        "node {id} references child {child_id} not in node map"
                    ));
                }
            }
        }

        // 3. Parent consistency: if node.parent == Some(pid), then pid.children contains node.id.
        for (&id, node) in &self.nodes {
            if let Some(pid) = node.parent {
                match self.nodes.get(&pid) {
                    Some(parent_node) => {
                        if !parent_node.children.contains(&id) {
                            return Err(format!(
                                "node {id} claims parent {pid}, but {pid}'s children list does not contain {id}"
                            ));
                        }
                    }
                    None => {
                        return Err(format!(
                            "node {id} claims parent {pid}, but {pid} not in node map"
                        ));
                    }
                }
            }
        }

        // 4. Root nodes must have parent == None.
        for &root_id in &self.roots {
            if let Some(node) = self.nodes.get(&root_id) {
                if node.parent.is_some() {
                    return Err(format!(
                        "root {root_id} has parent {:?}, expected None",
                        node.parent
                    ));
                }
            }
        }

        // 5. No root appears in any children list.
        for (id, node) in &self.nodes {
            for &child_id in &node.children {
                if root_set.contains(&child_id) {
                    return Err(format!("root {child_id} appears in children list of {id}"));
                }
            }
        }

        // 6. No cycles (DFS from each root, tracking visited and in-stack).
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        for &root_id in &self.roots {
            self.dfs_cycle_check(root_id, &mut visited, &mut stack)?;
        }

        // 7. No orphans: every node must have been visited.
        if visited.len() != self.nodes.len() {
            let orphans: Vec<_> = self
                .nodes
                .keys()
                .filter(|id| !visited.contains(id))
                .collect();
            return Err(format!(
                "orphan nodes not reachable from roots: {orphans:?}"
            ));
        }

        Ok(())
    }

    /// Converts validated data into a live `SceneGraph`.
    ///
    /// Call `validate()` before this to ensure structural integrity.
    /// The returned graph has a rebuilt spatial index and recomputed world transforms.
    #[must_use]
    pub fn into_graph(self) -> SceneGraph {
        SceneGraph::from_document_state(self.nodes, self.roots)
    }

    /// DFS cycle detection. Returns `Err` if a back-edge is found.
    fn dfs_cycle_check(
        &self,
        node_id: NodeId,
        visited: &mut HashSet<NodeId>,
        stack: &mut HashSet<NodeId>,
    ) -> Result<(), String> {
        if stack.contains(&node_id) {
            return Err(format!("cycle detected involving node {node_id}"));
        }
        if visited.contains(&node_id) {
            return Ok(());
        }

        visited.insert(node_id);
        stack.insert(node_id);

        if let Some(node) = self.nodes.get(&node_id) {
            for &child_id in &node.children {
                self.dfs_cycle_check(child_id, visited, stack)?;
            }
        }

        stack.remove(&node_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{BoundingBox, ClipMode, SceneNodeKind, Transform2D};

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

    fn make_text_node(name: &str, content: &str, font_size: f32) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Text {
                content: content.to_string(),
                font_size,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 30.0),
        )
    }

    // --- Roundtrip tests ---

    #[test]
    fn roundtrip_empty_graph() {
        let graph = SceneGraph::new();
        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();
        assert!(restored.is_empty());
        assert!(restored.roots().is_empty());
    }

    #[test]
    fn roundtrip_single_root() {
        let mut graph = SceneGraph::new();
        let node = make_node("Root");
        let id = graph.add_root(node);

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        assert_eq!(restored.len(), 1);
        assert_eq!(restored.roots(), &[id]);
        assert_eq!(restored.get(id).unwrap().name, "Root");
    }

    #[test]
    fn roundtrip_parent_child() {
        let mut graph = SceneGraph::new();
        let parent = make_node("Parent");
        let pid = graph.add_root(parent);
        let child = make_node("Child");
        let cid = child.id;
        graph.add_child(pid, child);

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        assert_eq!(restored.len(), 2);
        assert_eq!(restored.roots(), &[pid]);
        assert_eq!(restored.children(pid).unwrap(), &[cid]);
        assert_eq!(restored.parent(cid), Some(pid));
    }

    #[test]
    fn roundtrip_deep_hierarchy() {
        let mut graph = SceneGraph::new();
        let root = make_node("L0");
        let root_id = graph.add_root(root);

        let mut parent_id = root_id;
        let mut ids = vec![root_id];
        for i in 1..=5 {
            let child = make_node(&format!("L{i}"));
            let cid = child.id;
            graph.add_child(parent_id, child);
            ids.push(cid);
            parent_id = cid;
        }

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        assert_eq!(restored.len(), 6);
        for i in 1..ids.len() {
            assert_eq!(restored.parent(ids[i]), Some(ids[i - 1]));
        }
    }

    #[test]
    fn roundtrip_wide_hierarchy() {
        let mut graph = SceneGraph::new();
        let root = make_node("Root");
        let root_id = graph.add_root(root);

        let mut child_ids = Vec::new();
        for i in 0..10 {
            let child = make_node(&format!("Child{i}"));
            let cid = child.id;
            graph.add_child(root_id, child);
            child_ids.push(cid);
        }

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        assert_eq!(restored.len(), 11);
        assert_eq!(restored.children(root_id).unwrap(), &child_ids);
    }

    #[test]
    fn roundtrip_multiple_roots_preserves_order() {
        let mut graph = SceneGraph::new();
        let r1 = make_node("R1");
        let r1_id = graph.add_root(r1);
        let r2 = make_node("R2");
        let r2_id = graph.add_root(r2);
        let r3 = make_node("R3");
        let r3_id = graph.add_root(r3);

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        assert_eq!(restored.roots(), &[r1_id, r2_id, r3_id]);
    }

    #[test]
    fn roundtrip_scroll_offset() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("Scroller");
        node.scroll_offset = [42.0, 17.5];
        let id = graph.add_root(node);

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        assert_eq!(restored.get(id).unwrap().scroll_offset, [42.0, 17.5]);
    }

    #[test]
    fn roundtrip_text_node() {
        let mut graph = SceneGraph::new();
        let node = make_text_node("Label", "Hello World", 16.0);
        let id = graph.add_root(node);

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        match &restored.get(id).unwrap().kind {
            SceneNodeKind::Text {
                content, font_size, ..
            } => {
                assert_eq!(content, "Hello World");
                assert_eq!(*font_size, 16.0);
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_transform_and_styling() {
        let mut graph = SceneGraph::new();
        let mut node = make_node("Styled");
        node.local_transform = Transform2D::from_raw([2.0, 0.0, 0.0, 2.0, 10.0, 20.0]);
        node.opacity = 0.5;
        node.clip_mode = ClipMode::Scissor;
        let id = graph.add_root(node);

        let data = SceneGraphData::from_graph(&graph);
        assert!(data.validate().is_ok());
        let restored = data.into_graph();

        let n = restored.get(id).unwrap();
        assert_eq!(*n.local_transform.raw(), [2.0, 0.0, 0.0, 2.0, 10.0, 20.0]);
        assert_eq!(n.opacity, 0.5);
        assert_eq!(n.clip_mode, ClipMode::Scissor);
    }

    #[test]
    fn roundtrip_document_format_json() {
        let mut graph = SceneGraph::new();
        let node = make_node("Root");
        let id = graph.add_root(node);

        let doc = DocumentFormat {
            version: FORMAT_VERSION,
            saved_at: 1700000000,
            scene: SceneGraphData::from_graph(&graph),
        };

        let json = serde_json::to_string(&doc).expect("serialize");
        let restored: DocumentFormat = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.version, FORMAT_VERSION);
        assert_eq!(restored.saved_at, 1700000000);
        assert!(restored.scene.validate().is_ok());
        assert_eq!(restored.scene.roots, vec![id]);
    }

    // --- Validation rejection tests ---

    #[test]
    fn validate_rejects_missing_root() {
        let data = SceneGraphData {
            nodes: HashMap::new(),
            roots: vec![NodeId::new()],
        };
        let err = data.validate().unwrap_err();
        assert!(err.contains("not found in node map"), "got: {err}");
    }

    #[test]
    fn validate_rejects_missing_child() {
        let mut parent = make_node("Parent");
        let missing_id = NodeId::new();
        parent.children.push(missing_id);
        let pid = parent.id;

        let mut nodes = HashMap::new();
        nodes.insert(pid, parent);

        let data = SceneGraphData {
            nodes,
            roots: vec![pid],
        };
        let err = data.validate().unwrap_err();
        assert!(err.contains("not in node map"), "got: {err}");
    }

    #[test]
    fn validate_rejects_inconsistent_parent() {
        let parent = make_node("Parent");
        let pid = parent.id;
        let mut child = make_node("Child");
        child.parent = Some(pid);
        let cid = child.id;
        // Parent does NOT list child in its children.

        let mut nodes = HashMap::new();
        nodes.insert(pid, parent);
        nodes.insert(cid, child);

        let data = SceneGraphData {
            nodes,
            roots: vec![pid],
        };
        let err = data.validate().unwrap_err();
        assert!(err.contains("children list does not contain"), "got: {err}");
    }

    #[test]
    fn validate_rejects_root_with_parent() {
        // Root node `rid` has parent set to `rpid`, which should fail rule 4.
        // For rule 3 to pass first, the parent must exist and list the root as a child.
        let mut real_parent = make_node("RealParent");
        let rpid = real_parent.id;

        let mut root = make_node("RootWithParent");
        root.parent = Some(rpid);
        let rid = root.id;
        real_parent.children.push(rid);

        let mut nodes = HashMap::new();
        nodes.insert(rpid, real_parent);
        nodes.insert(rid, root);

        let data = SceneGraphData {
            nodes,
            roots: vec![rid],
        };
        let err = data.validate().unwrap_err();
        assert!(err.contains("has parent"), "got: {err}");
    }

    #[test]
    fn validate_rejects_root_in_children_list() {
        let mut parent = make_node("Parent");
        let pid = parent.id;
        let mut root_child = make_node("AlsoRoot");
        root_child.parent = None;
        let rcid = root_child.id;
        parent.children.push(rcid);

        let mut nodes = HashMap::new();
        nodes.insert(pid, parent);
        nodes.insert(rcid, root_child);

        let data = SceneGraphData {
            nodes,
            roots: vec![pid, rcid],
        };
        // Rule 3 will fail here because root_child.parent is None but parent lists it.
        // We need parent consistency for rule 5 to trigger.
        // Actually rule 3 checks: if node.parent == Some(pid), then pid.children contains id.
        // root_child.parent is None, so rule 3 won't check it.
        // Rule 5: root rcid in parent's children. Should trigger.
        let err = data.validate().unwrap_err();
        assert!(err.contains("appears in children list"), "got: {err}");
    }

    #[test]
    fn validate_rejects_cycle() {
        // A -> B -> C -> B (cycle at C pointing back to B).
        let mut a = make_node("A");
        let mut b = make_node("B");
        let mut c = make_node("C");
        let aid = a.id;
        let bid = b.id;
        let cid = c.id;

        a.children.push(bid);
        b.parent = Some(aid);
        b.children.push(cid);
        c.parent = Some(bid);
        c.children.push(bid); // cycle: C -> B

        let mut nodes = HashMap::new();
        nodes.insert(aid, a);
        nodes.insert(bid, b);
        nodes.insert(cid, c);

        let data = SceneGraphData {
            nodes,
            roots: vec![aid],
        };
        let err = data.validate().unwrap_err();
        assert!(err.contains("cycle"), "got: {err}");
    }

    #[test]
    fn validate_rejects_orphan() {
        let root = make_node("Root");
        let rid = root.id;
        let orphan = make_node("Orphan");
        let oid = orphan.id;

        let mut nodes = HashMap::new();
        nodes.insert(rid, root);
        nodes.insert(oid, orphan);

        let data = SceneGraphData {
            nodes,
            roots: vec![rid],
        };
        let err = data.validate().unwrap_err();
        assert!(err.contains("orphan"), "got: {err}");
    }
}
