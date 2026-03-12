//! WASM binding layer for the Selean design platform.
//!
//! Provides the [`SeleanEditor`] struct as the single entry point exposed
//! to JavaScript via `wasm-bindgen`. Owns the `Document`, `Renderer`,
//! `Camera`, `InputHandler`, and per-page `CommandHistory` instances.

pub mod command_descriptor;
pub mod queries;

use selean_engine::command::CommandHistory;
use selean_engine::input::{InputHandler, Modifiers, PointerButton};
use selean_engine::persistence::Document;
use selean_engine::scene::{Color, SceneGraph};

use command_descriptor::{CommandDescriptor, create_frame_node};
use queries::{
    get_all_node_bounds, get_node_json, get_pages_json, get_scene_json, get_scene_tree_json,
    get_selected_bounds_json, get_selected_ids_json, query_nodes_json,
};

/// Parses a string as a `NodeId`, returning `None` on invalid UUID.
fn parse_node_id(s: &str) -> Option<selean_common::types::NodeId> {
    uuid::Uuid::parse_str(s)
        .ok()
        .map(selean_common::types::NodeId::from_uuid)
}

/// Parses a string as a `PageId`, returning `None` on invalid UUID.
fn parse_page_id(s: &str) -> Option<selean_common::types::PageId> {
    uuid::Uuid::parse_str(s)
        .ok()
        .map(selean_common::types::PageId::from_uuid)
}

/// Parses a JSON array of node ID strings into a `Vec<NodeId>`.
/// Returns `None` if any ID is invalid.
fn parse_node_ids(json: &str) -> Option<Vec<selean_common::types::NodeId>> {
    let strings: Vec<String> = serde_json::from_str(json).ok()?;
    strings.iter().map(|s| parse_node_id(s)).collect()
}

/// Returns a JSON error response: `{ "success": false, "error": msg }`.
fn tool_error(msg: &str) -> String {
    serde_json::json!({ "success": false, "error": msg }).to_string()
}

/// Returns a JSON success response: `{ "success": true, "result": result }`.
fn tool_ok(result: serde_json::Value) -> String {
    serde_json::json!({ "success": true, "result": result }).to_string()
}

/// Core editor state, independent of the WASM runtime.
///
/// This struct holds all engine state. On native targets it can be used
/// directly; on WASM it is wrapped by the `wasm_bindgen`-exported
/// `SeleanEditor` facade below.
pub struct EditorState {
    /// The multi-page document model.
    pub document: Document,
    /// Per-page undo/redo histories, keyed by `PageId`.
    histories: std::collections::HashMap<selean_common::types::PageId, CommandHistory>,
    /// Input handler (hover, selection, drag, camera).
    pub input: InputHandler,
    /// Monotonically increasing version counter. Incremented on any mutation,
    /// selection change, or camera change. Frontend components can compare
    /// this against a cached value to skip redundant WASM FFI polls.
    version: u64,
}

impl EditorState {
    /// Creates a new editor with a default single-page document.
    #[must_use]
    pub fn new() -> Self {
        let doc = Document::new();
        let page_id = doc.active_page().id;
        let mut histories = std::collections::HashMap::new();
        histories.insert(page_id, CommandHistory::new());
        Self {
            document: doc,
            histories,
            input: InputHandler::new(),
            version: 0,
        }
    }

    /// Returns the current version counter. Incremented on every state change.
    /// Frontend components can compare this against a cached value to skip
    /// redundant WASM FFI polls when nothing has changed.
    #[must_use]
    pub fn scene_version(&self) -> u64 {
        self.version
    }

    /// Returns a reference to the active page's scene graph.
    #[must_use]
    pub fn scene(&self) -> &SceneGraph {
        &self.document.active_page().scene
    }

    /// Returns a mutable reference to the active page's scene graph.
    #[must_use]
    pub fn scene_mut(&mut self) -> &mut SceneGraph {
        &mut self.document.active_page_mut().scene
    }

    /// Returns a reference to the active page's command history.
    fn history(&self) -> &CommandHistory {
        let page_id = self.document.active_page().id;
        self.histories
            .get(&page_id)
            .expect("history missing for active page")
    }

    /// Returns a mutable reference to the active page's command history.
    fn history_mut(&mut self) -> &mut CommandHistory {
        let page_id = self.document.active_page().id;
        self.histories
            .get_mut(&page_id)
            .expect("history missing for active page")
    }

    /// Executes a command described by a JSON string.
    ///
    /// Returns `true` if the command was applied successfully.
    pub fn execute_command_json(&mut self, json: &str) -> bool {
        let desc: CommandDescriptor = match serde_json::from_str(json) {
            Ok(d) => d,
            Err(_) => return false,
        };
        let cmd = desc.into_command();
        let page_id = self.document.active_page().id;
        let history = self
            .histories
            .get_mut(&page_id)
            .expect("history missing for active page");
        let ok = history.execute(cmd, &mut self.document.active_page_mut().scene);
        if ok {
            self.version += 1;
        }
        ok
    }

    /// Executes a [`CommandDescriptor`] directly.
    pub fn execute_descriptor(&mut self, desc: CommandDescriptor) -> bool {
        let cmd = desc.into_command();
        let page_id = self.document.active_page().id;
        let history = self
            .histories
            .get_mut(&page_id)
            .expect("history missing for active page");
        let ok = history.execute(cmd, &mut self.document.active_page_mut().scene);
        if ok {
            self.version += 1;
        }
        ok
    }

    /// Undoes the last command on the active page.
    pub fn undo(&mut self) -> bool {
        let page_id = self.document.active_page().id;
        let history = self
            .histories
            .get_mut(&page_id)
            .expect("history missing for active page");
        let ok = history.undo(&mut self.document.active_page_mut().scene);
        if ok {
            self.version += 1;
        }
        ok
    }

    /// Redoes the last undone command on the active page.
    pub fn redo(&mut self) -> bool {
        let page_id = self.document.active_page().id;
        let history = self
            .histories
            .get_mut(&page_id)
            .expect("history missing for active page");
        let ok = history.redo(&mut self.document.active_page_mut().scene);
        if ok {
            self.version += 1;
        }
        ok
    }

    /// Begins a command group (for drag operations or LLM batches).
    pub fn begin_group(&mut self, label: &str) {
        self.history_mut().begin_group(label);
    }

    /// Ends the active command group.
    pub fn end_group(&mut self) {
        self.history_mut().end_group();
    }

    /// Cancels the active command group, undoing all commands in it.
    pub fn cancel_group(&mut self) {
        let page_id = self.document.active_page().id;
        let history = self
            .histories
            .get_mut(&page_id)
            .expect("history missing for active page");
        history.cancel_group(&mut self.document.active_page_mut().scene);
    }

    /// Applies a remote operation to a specific page, bypassing the local
    /// `CommandHistory`. Used for operations from other collaborative editing
    /// participants that should not appear in this user's undo stack.
    ///
    /// Returns `true` if the command was applied successfully.
    pub fn apply_remote_op(&mut self, page_id: &str, descriptor_json: &str) -> bool {
        let Some(target) = parse_page_id(page_id) else {
            return false;
        };
        let Some(page) = self.document.page_mut(target) else {
            return false;
        };
        let Ok(desc) = serde_json::from_str::<CommandDescriptor>(descriptor_json) else {
            return false;
        };
        let mut cmd = desc.into_command();
        cmd.execute(&mut page.scene)
    }

    /// Applies a group of remote operations to a specific page, bypassing
    /// the local `CommandHistory`.
    ///
    /// Returns `true` if all commands were applied successfully.
    pub fn apply_remote_op_group(&mut self, page_id: &str, descriptors_json: &str) -> bool {
        let Some(target) = parse_page_id(page_id) else {
            return false;
        };
        let Some(page) = self.document.page_mut(target) else {
            return false;
        };
        let Ok(descs) = serde_json::from_str::<Vec<CommandDescriptor>>(descriptors_json) else {
            return false;
        };
        let mut all_ok = true;
        for desc in descs {
            let mut cmd = desc.into_command();
            if !cmd.execute(&mut page.scene) {
                all_ok = false;
            }
        }
        all_ok
    }

    /// Applies a remote page-level operation (add, remove, rename).
    ///
    /// For collaborative editing: page ops are broadcast by the server
    /// and applied on all clients via this method.
    ///
    /// `op_type` is `"add"`, `"remove"`, or `"rename"`.
    pub fn apply_remote_page_op(
        &mut self,
        op_type: &str,
        page_id: &str,
        name: Option<&str>,
        width: Option<f32>,
        height: Option<f32>,
    ) -> bool {
        let Some(target) = parse_page_id(page_id) else {
            return false;
        };

        match op_type {
            "add" => {
                let page_name = name.unwrap_or("New Page");
                let w = width.unwrap_or(1920.0);
                let h = height.unwrap_or(1080.0);
                if self.document.add_page_with_id(target, page_name, w, h) {
                    self.histories.insert(target, CommandHistory::new());
                    true
                } else {
                    false
                }
            }
            "remove" => self.remove_page(page_id),
            "rename" => {
                if let Some(new_name) = name {
                    if let Some(page) = self.document.page_mut(target) {
                        page.name = new_name.to_string();
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Returns the active page's ID as a string.
    #[must_use]
    pub fn active_page_id(&self) -> String {
        self.document.active_page().id.to_string()
    }

    /// Returns whether undo is available on the active page.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history().can_undo()
    }

    /// Returns whether redo is available on the active page.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history().can_redo()
    }

    /// Returns JSON for a single node.
    #[must_use]
    pub fn get_node_json(&self, node_id: &str) -> String {
        get_node_json(self.scene(), node_id)
    }

    /// Returns JSON array of selected node IDs.
    #[must_use]
    pub fn get_selected_ids(&self) -> String {
        get_selected_ids_json(self.input.state().selection.ids())
    }

    /// Returns JSON representation of the active page's scene.
    #[must_use]
    pub fn get_scene_json(&self) -> String {
        get_scene_json(self.scene())
    }

    /// Returns JSON array of page metadata.
    #[must_use]
    pub fn get_pages_json(&self) -> String {
        get_pages_json(&self.document)
    }

    /// Switches the active page by ID string. Returns `true` on success.
    pub fn set_active_page(&mut self, page_id: &str) -> bool {
        let Some(id) = parse_page_id(page_id) else {
            return false;
        };
        self.document.set_active_page(id)
    }

    /// Adds a new page and returns its ID as a string.
    pub fn add_page(&mut self, name: &str, width: f32, height: f32) -> String {
        let id = self.document.add_page(name, width, height);
        self.histories.insert(id, CommandHistory::new());
        id.to_string()
    }

    /// Removes a page by ID string. Returns `true` on success.
    pub fn remove_page(&mut self, page_id: &str) -> bool {
        let Some(id) = parse_page_id(page_id) else {
            return false;
        };
        if !self.document.remove_page(id) {
            return false;
        }
        // History is keyed by PageId (HashMap), no index sync needed.
        self.histories.remove(&id);
        true
    }

    /// Returns a JSON tree of the active page's scene hierarchy.
    #[must_use]
    pub fn get_scene_tree_json(&self) -> String {
        get_scene_tree_json(self.scene())
    }

    /// Renames a page by ID string. Returns `true` on success.
    pub fn rename_page(&mut self, page_id: &str, name: &str) -> bool {
        let Some(id) = parse_page_id(page_id) else {
            return false;
        };
        self.document.rename_page(id, name)
    }

    /// Duplicates a page by ID string. Returns the new page's ID, or empty string on failure.
    pub fn duplicate_page(&mut self, page_id: &str) -> String {
        let Some(id) = parse_page_id(page_id) else {
            return String::new();
        };
        match self.document.duplicate_page(id) {
            Some(new_id) => {
                self.histories.insert(new_id, CommandHistory::new());
                new_id.to_string()
            }
            None => String::new(),
        }
    }

    /// Reorders pages by a JSON array of page ID strings. Returns `true` on success.
    pub fn reorder_pages(&mut self, page_ids_json: &str) -> bool {
        let Ok(ids_str) = serde_json::from_str::<Vec<String>>(page_ids_json) else {
            return false;
        };
        let ids: Vec<selean_common::types::PageId> =
            ids_str.iter().filter_map(|s| parse_page_id(s)).collect();
        if ids.len() != ids_str.len() {
            return false;
        }
        // Histories are keyed by PageId, so reordering pages doesn't affect them.
        self.document.reorder_pages(&ids)
    }

    /// Resizes a page by ID string. Returns `true` on success.
    pub fn resize_page(&mut self, page_id: &str, width: f32, height: f32) -> bool {
        let Some(id) = parse_page_id(page_id) else {
            return false;
        };
        self.document.resize_page(id, width, height)
    }

    /// Returns JSON array of world-space bounds for selected nodes.
    #[must_use]
    pub fn get_selected_bounds_json(&self) -> String {
        get_selected_bounds_json(self.scene(), self.input.state().selection.ids())
    }

    /// Returns lightweight bounds for all nodes except the currently selected ones.
    /// Used by SnapGuides to find alignment targets without full-scene serialization.
    #[must_use]
    pub fn get_snap_targets(&self) -> Vec<queries::NodeBoundsInfo> {
        get_all_node_bounds(self.scene(), self.input.state().selection.ids())
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        self.input.state_mut().selection.clear();
        self.version += 1;
    }

    /// Aligns the given nodes according to the specified alignment kind.
    ///
    /// `node_ids_json` is a JSON array of node ID strings.
    /// `alignment` is a variant name from `AlignmentKind`.
    ///
    /// Returns `true` if the alignment was applied successfully.
    pub fn align_nodes(&mut self, node_ids_json: &str, alignment: &str) -> bool {
        use selean_engine::scene::align::{AlignmentKind, compute_alignment};

        let Some(kind) = AlignmentKind::parse(alignment) else {
            return false;
        };

        let Some(node_ids) = parse_node_ids(node_ids_json) else {
            return false;
        };

        let results = compute_alignment(self.scene(), &node_ids, kind);
        if results.is_empty() {
            return false;
        }

        self.history_mut().begin_group("Align");
        for result in results {
            let desc = command_descriptor::CommandDescriptor::SetBounds {
                node_id: result.node_id,
                bounds: result.new_bounds,
            };
            self.execute_descriptor(desc);
        }
        self.history_mut().end_group();
        true
    }

    /// Executes the `set_rotation` tool, which needs scene access to compute
    /// the bounds center for rotation.
    #[allow(clippy::cast_possible_truncation)]
    fn execute_rotation_tool(&mut self, args: &serde_json::Value) -> String {
        let node_id_str = args
            .get("node_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let angle_degrees = args
            .get("angle_degrees")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;

        let Some(node_id) = parse_node_id(node_id_str) else {
            return tool_error("invalid node ID");
        };

        let Some(node) = self.scene().get(node_id) else {
            return tool_error("node not found");
        };

        let cx = node.bounds.x + node.bounds.width / 2.0;
        let cy = node.bounds.y + node.bounds.height / 2.0;
        let angle_rad = angle_degrees.to_radians();
        let transform = selean_engine::scene::Transform2D::from_rotation_around(angle_rad, cx, cy);

        let desc = CommandDescriptor::SetTransform {
            node_id,
            transform: *transform.raw(),
        };
        let ok = self.execute_descriptor(desc);

        tool_ok(serde_json::json!({ "executed": i32::from(ok) }))
    }

    /// Executes a grouping tool (`group_nodes` or `ungroup_node`).
    #[allow(clippy::cast_possible_truncation)]
    fn execute_group_tool(&mut self, tool_name: &str, args: &serde_json::Value) -> String {
        match tool_name {
            "group_nodes" => self.execute_group_nodes(args),
            "ungroup_node" => self.execute_ungroup_node(args),
            _ => tool_error(&format!("unknown group tool: {tool_name}")),
        }
    }

    /// Groups the given nodes under a new Group node.
    #[allow(clippy::cast_possible_truncation)]
    fn execute_group_nodes(&mut self, args: &serde_json::Value) -> String {
        use selean_engine::scene::{BoundingBox, SceneNode, SceneNodeKind};

        let ids_raw: Vec<String> = match args.get("node_ids") {
            Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
            None => return tool_error("missing node_ids"),
        };

        if ids_raw.len() < 2 {
            return tool_error("need at least 2 nodes to group");
        }

        let mut node_ids = Vec::with_capacity(ids_raw.len());
        for s in &ids_raw {
            let Some(id) = parse_node_id(s) else {
                return tool_error(&format!("invalid node ID: {s}"));
            };
            node_ids.push(id);
        }

        // Compute union bounds.
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for &nid in &node_ids {
            let Some(node) = self.scene().get(nid) else {
                return tool_error(&format!("node not found: {nid}"));
            };
            let b = &node.bounds;
            min_x = min_x.min(b.x);
            min_y = min_y.min(b.y);
            max_x = max_x.max(b.x + b.width);
            max_y = max_y.max(b.y + b.height);
        }

        // Create Group node.
        let group_id = selean_common::types::NodeId::new();
        let group_node = SceneNode::new(
            group_id,
            "Group".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(min_x, min_y, max_x - min_x, max_y - min_y),
        );

        self.history_mut().begin_group("Group");

        // Add group as root.
        let desc = CommandDescriptor::AddRoot { node: group_node };
        self.execute_descriptor(desc);

        // Reparent each node into the group.
        for &nid in &node_ids {
            let desc = CommandDescriptor::Reparent {
                node_id: nid,
                new_parent_id: group_id,
            };
            self.execute_descriptor(desc);
        }

        self.history_mut().end_group();

        tool_ok(serde_json::json!({ "group_id": group_id.to_string() }))
    }

    /// Dissolves a Group node, reparenting its children to the group's parent.
    fn execute_ungroup_node(&mut self, args: &serde_json::Value) -> String {
        let node_id_str = args
            .get("node_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");

        let Some(group_id) = parse_node_id(node_id_str) else {
            return tool_error("invalid node ID");
        };

        let Some(node) = self.scene().get(group_id) else {
            return tool_error("node not found");
        };

        if !matches!(node.kind, selean_engine::scene::SceneNodeKind::Group) {
            return tool_error("node is not a Group");
        }

        let children = node.children.clone();
        let parent_id = node.parent;

        if children.is_empty() {
            // Empty group: just remove it.
            self.execute_descriptor(CommandDescriptor::RemoveNode { node_id: group_id });
            return tool_ok(serde_json::json!({ "ungrouped": 0 }));
        }

        self.history_mut().begin_group("Ungroup");

        // Reparent children to the group's parent (or make them roots).
        for &child_id in &children {
            if let Some(pid) = parent_id {
                let desc = CommandDescriptor::Reparent {
                    node_id: child_id,
                    new_parent_id: pid,
                };
                self.execute_descriptor(desc);
            } else {
                // Group was a root. Move child (with its full subtree) to root level.
                let desc = CommandDescriptor::ReparentToRoot { node_id: child_id };
                self.execute_descriptor(desc);
            }
        }

        // Remove the now-empty group.
        let desc = CommandDescriptor::RemoveNode { node_id: group_id };
        self.execute_descriptor(desc);

        self.history_mut().end_group();

        let count = children.len();
        tool_ok(serde_json::json!({ "ungrouped": count }))
    }

    /// Executes a z-order tool (`move_to_front`, `move_to_back`, `move_forward`, `move_backward`).
    fn execute_z_order_tool(&mut self, tool_name: &str, args: &serde_json::Value) -> String {
        let node_id_str = args
            .get("node_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");

        let Some(node_id) = parse_node_id(node_id_str) else {
            return tool_error("invalid node ID");
        };

        let Some(node) = self.scene().get(node_id) else {
            return tool_error("node not found");
        };
        let parent_id = node.parent;

        let (siblings, is_root) = if let Some(pid) = parent_id {
            let Some(children) = self.scene().children(pid) else {
                return tool_error("parent not found");
            };
            (children.to_vec(), false)
        } else {
            (self.scene().roots().to_vec(), true)
        };

        let Some(new_order) = compute_z_reorder(&siblings, node_id, tool_name) else {
            return tool_ok(serde_json::json!({ "moved": false }));
        };

        let desc = if is_root {
            CommandDescriptor::ReorderRoots { new_order }
        } else {
            let Some(pid) = parent_id else {
                return tool_error("parent_id missing for non-root node");
            };
            CommandDescriptor::ReorderChildren {
                parent_id: pid,
                new_order,
            }
        };

        let ok = self.execute_descriptor(desc);
        tool_ok(serde_json::json!({ "moved": ok }))
    }

    /// Returns the current zoom level.
    #[must_use]
    pub fn get_zoom(&self) -> f32 {
        // Zoom is stored on the Camera, which lives on the Renderer (WASM only).
        // For non-WASM (EditorState) we don't own a Camera. Returning 1.0 is the
        // safe default for tests; the WASM facade overrides this.
        1.0
    }

    /// Selects a single node by ID string. Returns `true` if the node exists.
    pub fn select_node_by_id(&mut self, node_id: &str) -> bool {
        let Some(id) = parse_node_id(node_id) else {
            return false;
        };
        if self.scene().get(id).is_none() {
            return false;
        }
        self.input.state_mut().selection.select_one(id);
        self.version += 1;
        true
    }

    /// Executes a page-level tool call (`add_page`, `remove_page`, `set_active_page`).
    #[allow(clippy::cast_possible_truncation)]
    fn execute_page_tool(&mut self, tool_name: &str, args: &serde_json::Value) -> String {
        match tool_name {
            "add_page" => {
                let name = args
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("New Page");
                let width = args
                    .get("width")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1920.0) as f32;
                let height = args
                    .get("height")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1080.0) as f32;
                let id = self.add_page(name, width, height);
                tool_ok(serde_json::json!({ "page_id": id }))
            }
            "remove_page" => {
                let page_id = args
                    .get("page_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let ok = self.remove_page(page_id);
                tool_ok(serde_json::json!({ "removed": ok }))
            }
            "set_active_page" => {
                let page_id = args
                    .get("page_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let ok = self.set_active_page(page_id);
                tool_ok(serde_json::json!({ "switched": ok }))
            }
            _ => tool_error(&format!("unknown page tool: {tool_name}")),
        }
    }

    /// Imports a document from JSON, replacing the current state.
    /// Returns `true` on success.
    pub fn import_document(&mut self, json: &str) -> bool {
        match selean_engine::persistence::load_document(json) {
            Ok(doc) => {
                let histories = doc
                    .pages()
                    .iter()
                    .map(|p| (p.id, CommandHistory::new()))
                    .collect();
                self.document = doc;
                self.histories = histories;
                self.input = InputHandler::new();
                self.version += 1;
                true
            }
            Err(_) => false,
        }
    }

    /// Exports the current document as a JSON string.
    #[must_use]
    pub fn export_document_json(&self) -> String {
        selean_engine::persistence::save_document(&self.document)
            .unwrap_or_else(|_| "{}".to_string())
    }

    /// Executes an LLM tool call by name and JSON args string.
    ///
    /// Returns a JSON string: `{ "success": true/false, "result": ... }`.
    /// For read-only tools, the result contains the query data.
    /// For mutation tools, the result contains `{ "executed": N }`.
    pub fn execute_tool_call(&mut self, tool_name: &str, args_json: &str) -> String {
        let args: serde_json::Value = match serde_json::from_str(args_json) {
            Ok(v) => v,
            Err(e) => return tool_error(&format!("invalid JSON args: {e}")),
        };

        // Dispatch by tool category.
        match selean_llm::tool_category(tool_name) {
            selean_llm::ToolCategory::ReadOnly => {
                let result = match tool_name {
                    "get_scene_summary" => get_scene_json(self.scene()),
                    "get_node" => {
                        let node_id = args
                            .get("node_id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        get_node_json(self.scene(), node_id)
                    }
                    "query_nodes" => {
                        let name_pattern =
                            args.get("name_pattern").and_then(serde_json::Value::as_str);
                        let kind = args.get("kind").and_then(serde_json::Value::as_str);
                        query_nodes_json(self.scene(), name_pattern, kind)
                    }
                    "get_pages" => get_pages_json(&self.document),
                    _ => "null".to_string(),
                };
                let parsed = serde_json::from_str::<serde_json::Value>(&result)
                    .unwrap_or(serde_json::Value::Null);
                return tool_ok(parsed);
            }
            selean_llm::ToolCategory::Page => {
                return self.execute_page_tool(tool_name, &args);
            }
            selean_llm::ToolCategory::Align => {
                let node_ids_json = args
                    .get("node_ids")
                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                    .unwrap_or_default();
                let alignment = args
                    .get("alignment")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let ok = self.align_nodes(&node_ids_json, alignment);
                return tool_ok(serde_json::json!({ "aligned": ok }));
            }
            selean_llm::ToolCategory::Rotation => {
                return self.execute_rotation_tool(&args);
            }
            selean_llm::ToolCategory::Group => {
                return self.execute_group_tool(tool_name, &args);
            }
            selean_llm::ToolCategory::ZOrder => {
                return self.execute_z_order_tool(tool_name, &args);
            }
            selean_llm::ToolCategory::Mutation => {}
        }

        self.execute_mutation_tool(tool_name, &args)
    }

    /// Executes a mutation tool by mapping it to command descriptors.
    fn execute_mutation_tool(&mut self, tool_name: &str, args: &serde_json::Value) -> String {
        let descriptors = match selean_llm::map_tool_call(tool_name, args) {
            Ok(descs) => descs,
            Err(e) => return tool_error(&e.to_string()),
        };

        let use_group = descriptors.len() > 1;
        if use_group {
            self.history_mut().begin_group(tool_name);
        }

        let mut executed = 0;
        let mut failed = false;
        for desc in descriptors {
            if self.execute_descriptor(desc) {
                executed += 1;
            } else {
                failed = true;
                break;
            }
        }

        if use_group {
            if failed {
                self.cancel_group();
            } else {
                self.history_mut().end_group();
            }
        }

        if failed {
            tool_error(&format!("tool partially failed after {executed} commands"))
        } else {
            tool_ok(serde_json::json!({ "executed": executed }))
        }
    }

    /// Sets up a demo scene with colored rectangles on the active page.
    pub fn setup_demo_scene(&mut self) {
        let colors = [
            (
                "Red Box",
                50.0,
                50.0,
                150.0,
                100.0,
                Color::new(0.9, 0.2, 0.2, 1.0),
            ),
            (
                "Blue Box",
                250.0,
                80.0,
                120.0,
                120.0,
                Color::new(0.2, 0.4, 0.9, 1.0),
            ),
            (
                "Green Box",
                100.0,
                200.0,
                180.0,
                80.0,
                Color::new(0.2, 0.8, 0.3, 1.0),
            ),
            (
                "Yellow Box",
                350.0,
                30.0,
                100.0,
                160.0,
                Color::new(0.95, 0.85, 0.2, 1.0),
            ),
            (
                "Purple Box",
                300.0,
                250.0,
                140.0,
                90.0,
                Color::new(0.6, 0.2, 0.8, 1.0),
            ),
        ];

        for (name, x, y, w, h, color) in &colors {
            let node = create_frame_node(name, *x, *y, *w, *h, Some(*color), [8.0; 4]);
            self.scene_mut().add_root(node);
        }
    }

    /// Converts pointer button index from JS (0=left, 1=middle, 2=right) to engine enum.
    pub fn pointer_button(button: u32) -> PointerButton {
        match button {
            1 => PointerButton::Middle,
            2 => PointerButton::Right,
            _ => PointerButton::Left,
        }
    }

    /// Builds a `Modifiers` struct from boolean flags.
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn modifiers(shift: bool, ctrl: bool, alt: bool, meta: bool) -> Modifiers {
        Modifiers {
            shift,
            ctrl,
            alt,
            meta,
        }
    }

    // --- Code generation bindings ---

    /// Generates React + Tailwind code for the active page.
    #[must_use]
    pub fn generate_code(&self) -> String {
        selean_codegen::generate_page_code(self.document.active_page())
    }

    /// Generates React + Tailwind code for a specific page by ID.
    ///
    /// # Errors
    ///
    /// Returns an error string if the page ID is invalid or the page is not found.
    pub fn generate_page_code(&self, page_id: &str) -> Result<String, String> {
        let pid = parse_page_id(page_id).ok_or_else(|| "invalid page ID".to_string())?;
        let page = self
            .document
            .page(pid)
            .ok_or_else(|| "page not found".to_string())?;
        Ok(selean_codegen::generate_page_code(page))
    }

    /// Generates a full project structure as JSON.
    ///
    /// Returns a JSON object with `files: [{ path, content }]`.
    #[must_use]
    pub fn generate_project_json(&self) -> String {
        let output = selean_codegen::generate_project(&self.document);
        serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
    }

    /// Extracts design tokens as JSON.
    #[must_use]
    pub fn extract_design_tokens_json(&self) -> String {
        let tokens = selean_codegen::extract_tokens(&self.document);
        serde_json::to_string(&tokens).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Computes the new sibling order after applying a z-order tool.
///
/// Returns `None` if the node is already at the boundary (no-op) or the tool is unknown.
/// Returns `Some(new_order)` with the reordered sibling list.
fn compute_z_reorder(
    siblings: &[selean_common::types::NodeId],
    node_id: selean_common::types::NodeId,
    tool_name: &str,
) -> Option<Vec<selean_common::types::NodeId>> {
    let current_idx = siblings.iter().position(|&id| id == node_id)?;
    let last = siblings.len() - 1;
    let mut order = siblings.to_vec();

    match tool_name {
        "move_to_front" => {
            if current_idx == last {
                return None;
            }
            order.remove(current_idx);
            order.push(node_id);
        }
        "move_to_back" => {
            if current_idx == 0 {
                return None;
            }
            order.remove(current_idx);
            order.insert(0, node_id);
        }
        "move_forward" => {
            if current_idx == last {
                return None;
            }
            order.swap(current_idx, current_idx + 1);
        }
        "move_backward" => {
            if current_idx == 0 {
                return None;
            }
            order.swap(current_idx, current_idx - 1);
        }
        _ => return None,
    }
    Some(order)
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

// --- WASM-specific exports ---

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use selean_engine::input::InputEvent;
    use selean_engine::renderer::{Renderer, RendererDescriptor};
    use wasm_bindgen::prelude::*;

    /// Browser-facing editor handle exposed via `wasm-bindgen`.
    #[wasm_bindgen]
    pub struct SeleanEditor {
        state: EditorState,
        renderer: Renderer,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
    }

    #[wasm_bindgen]
    impl SeleanEditor {
        /// Creates a new editor bound to the given canvas element ID.
        pub async fn create(canvas_id: &str) -> Result<SeleanEditor, JsValue> {
            console_error_panic_hook::set_once();

            let window = web_sys::window().ok_or("no window")?;
            let document = window.document().ok_or("no document")?;
            let canvas = document
                .get_element_by_id(canvas_id)
                .ok_or("canvas not found")?
                .dyn_into::<web_sys::HtmlCanvasElement>()?;

            let width = canvas.client_width() as f32;
            let height = canvas.client_height() as f32;

            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::BROWSER_WEBGPU,
                ..Default::default()
            });

            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .ok_or("no adapter")?;

            let target_format = surface
                .get_capabilities(&adapter)
                .formats
                .first()
                .copied()
                .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

            let desc = RendererDescriptor {
                viewport_width: width,
                viewport_height: height,
                ..RendererDescriptor::default()
            };

            let renderer = Renderer::new(&desc, target_format)
                .await
                .map_err(|e| JsValue::from_str(&format!("{e:?}")))?;

            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: target_format,
                width: width as u32,
                height: height as u32,
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&renderer.gpu().device, &surface_config);

            let mut state = EditorState::new();
            state.setup_demo_scene();

            Ok(Self {
                state,
                renderer,
                surface,
                surface_config,
            })
        }

        /// Resizes the viewport and reconfigures the surface.
        pub fn resize(&mut self, width: f32, height: f32) {
            let w = width as u32;
            let h = height as u32;
            if w == 0 || h == 0 {
                return;
            }
            self.surface_config.width = w;
            self.surface_config.height = h;
            self.surface
                .configure(&self.renderer.gpu().device, &self.surface_config);
            self.renderer.resize(width, height);
        }

        /// Renders a frame to the canvas.
        pub fn render(&mut self) {
            let frame = match self.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    self.surface
                        .configure(&self.renderer.gpu().device, &self.surface_config);
                    return;
                }
                Err(_) => return,
            };
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.renderer.render_frame(self.state.scene_mut(), &view);
            frame.present();
        }

        /// Handles pointer move events from JavaScript.
        /// Returns a JS array of `InteractionEvent` objects directly (no JSON string).
        pub fn on_pointer_move(
            &mut self,
            x: f32,
            y: f32,
            shift: bool,
            ctrl: bool,
            alt: bool,
            meta: bool,
        ) -> JsValue {
            let event = InputEvent::PointerMove {
                x,
                y,
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                self.state.scene_mut(),
                self.renderer.camera_mut(),
            );
            serde_wasm_bindgen::to_value(&events).unwrap_or(JsValue::NULL)
        }

        /// Handles pointer down events from JavaScript.
        /// Returns a JS array of `InteractionEvent` objects directly (no JSON string).
        pub fn on_pointer_down(
            &mut self,
            x: f32,
            y: f32,
            button: u32,
            shift: bool,
            ctrl: bool,
            alt: bool,
            meta: bool,
        ) -> JsValue {
            let event = InputEvent::PointerDown {
                x,
                y,
                button: EditorState::pointer_button(button),
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                self.state.scene_mut(),
                self.renderer.camera_mut(),
            );
            serde_wasm_bindgen::to_value(&events).unwrap_or(JsValue::NULL)
        }

        /// Handles pointer up events from JavaScript.
        /// Returns a JS array of `InteractionEvent` objects directly (no JSON string).
        pub fn on_pointer_up(
            &mut self,
            x: f32,
            y: f32,
            button: u32,
            shift: bool,
            ctrl: bool,
            alt: bool,
            meta: bool,
        ) -> JsValue {
            let event = InputEvent::PointerUp {
                x,
                y,
                button: EditorState::pointer_button(button),
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                self.state.scene_mut(),
                self.renderer.camera_mut(),
            );
            serde_wasm_bindgen::to_value(&events).unwrap_or(JsValue::NULL)
        }

        /// Handles scroll events from JavaScript.
        /// Returns a JS array of `InteractionEvent` objects directly (no JSON string).
        pub fn on_scroll(
            &mut self,
            x: f32,
            y: f32,
            dx: f32,
            dy: f32,
            shift: bool,
            ctrl: bool,
            alt: bool,
            meta: bool,
        ) -> JsValue {
            let event = InputEvent::ScrollDelta {
                x,
                y,
                dx,
                dy,
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                self.state.scene_mut(),
                self.renderer.camera_mut(),
            );
            serde_wasm_bindgen::to_value(&events).unwrap_or(JsValue::NULL)
        }

        /// Executes a command from a JSON descriptor string.
        pub fn execute_command(&mut self, json: &str) -> bool {
            self.state.execute_command_json(json)
        }

        /// Undoes the last command.
        pub fn undo(&mut self) -> bool {
            self.state.undo()
        }

        /// Redoes the last undone command.
        pub fn redo(&mut self) -> bool {
            self.state.redo()
        }

        /// Returns JSON for a single node by ID string.
        pub fn get_node_json(&self, node_id: &str) -> String {
            self.state.get_node_json(node_id)
        }

        /// Returns a JS array of selected node ID strings directly.
        pub fn get_selected_ids(&self) -> JsValue {
            let ids: Vec<String> = self
                .state
                .input
                .state()
                .selection
                .ids()
                .iter()
                .map(ToString::to_string)
                .collect();
            serde_wasm_bindgen::to_value(&ids).unwrap_or(JsValue::NULL)
        }

        /// Returns JSON representation of the full scene.
        pub fn get_scene_json(&self) -> String {
            self.state.get_scene_json()
        }

        /// Executes an LLM tool call by name and JSON args.
        /// Returns JSON `{ "success": bool, "result": ... }`.
        pub fn execute_tool_call(&mut self, tool_name: &str, args_json: &str) -> String {
            self.state.execute_tool_call(tool_name, args_json)
        }

        /// Returns the active page's ID as a string.
        pub fn active_page_id(&self) -> String {
            self.state.active_page_id()
        }

        /// Returns the scene version counter. Monotonically increasing;
        /// changes on any mutation, selection, or page switch.
        pub fn scene_version(&self) -> u64 {
            self.state.scene_version()
        }

        /// Returns whether undo is available.
        pub fn can_undo(&self) -> bool {
            self.state.can_undo()
        }

        /// Returns whether redo is available.
        pub fn can_redo(&self) -> bool {
            self.state.can_redo()
        }

        /// Returns JSON array of page metadata.
        pub fn get_pages_json(&self) -> String {
            self.state.get_pages_json()
        }

        /// Switches the active page by ID.
        pub fn set_active_page(&mut self, page_id: &str) -> bool {
            self.state.set_active_page(page_id)
        }

        /// Adds a page and returns its ID as a string.
        pub fn add_page(&mut self, name: &str, width: f32, height: f32) -> String {
            self.state.add_page(name, width, height)
        }

        /// Removes a page by ID.
        pub fn remove_page(&mut self, page_id: &str) -> bool {
            self.state.remove_page(page_id)
        }

        /// Renames a page by ID.
        pub fn rename_page(&mut self, page_id: &str, name: &str) -> bool {
            self.state.rename_page(page_id, name)
        }

        /// Duplicates a page by ID. Returns the new page's ID, or empty string on failure.
        pub fn duplicate_page(&mut self, page_id: &str) -> String {
            self.state.duplicate_page(page_id)
        }

        /// Reorders pages by a JSON array of page ID strings.
        pub fn reorder_pages(&mut self, page_ids_json: &str) -> bool {
            self.state.reorder_pages(page_ids_json)
        }

        /// Resizes a page by ID.
        pub fn resize_page(&mut self, page_id: &str, width: f32, height: f32) -> bool {
            self.state.resize_page(page_id, width, height)
        }

        /// Returns the scene tree as JSON.
        pub fn get_scene_tree_json(&self) -> String {
            self.state.get_scene_tree_json()
        }

        /// Imports a document from JSON.
        pub fn import_document(&mut self, json: &str) -> bool {
            self.state.import_document(json)
        }

        /// Exports the document as JSON.
        pub fn export_document_json(&self) -> String {
            self.state.export_document_json()
        }

        /// Returns a JS array of world-space bounds for selected nodes directly.
        pub fn get_selected_bounds(&self) -> JsValue {
            let bounds = crate::queries::get_selected_bounds(
                self.state.scene(),
                self.state.input.state().selection.ids(),
            );
            serde_wasm_bindgen::to_value(&bounds).unwrap_or(JsValue::NULL)
        }

        /// Returns camera state as a JS object (pan, zoom, viewport).
        pub fn get_camera(&self) -> JsValue {
            let info = crate::queries::get_camera_info(self.renderer.camera());
            serde_wasm_bindgen::to_value(&info).unwrap_or(JsValue::NULL)
        }

        /// Returns lightweight bounds for all non-selected nodes.
        /// Used by SnapGuides to find alignment targets efficiently.
        pub fn get_snap_targets(&self) -> JsValue {
            let targets = self.state.get_snap_targets();
            serde_wasm_bindgen::to_value(&targets).unwrap_or(JsValue::NULL)
        }

        /// Clears the current selection.
        pub fn clear_selection(&mut self) {
            self.state.clear_selection();
        }

        /// Selects a single node by ID. Returns `true` if the node exists.
        pub fn select_node_by_id(&mut self, node_id: &str) -> bool {
            self.state.select_node_by_id(node_id)
        }

        /// Aligns the given nodes according to the specified alignment kind.
        pub fn align_nodes(&mut self, node_ids_json: &str, alignment: &str) -> bool {
            self.state.align_nodes(node_ids_json, alignment)
        }

        /// Begins a command group (for drag gestures).
        pub fn begin_group(&mut self, label: &str) {
            self.state.begin_group(label);
        }

        /// Ends the active command group.
        pub fn end_group(&mut self) {
            self.state.end_group();
        }

        /// Cancels the active command group, undoing all commands in it.
        pub fn cancel_group(&mut self) {
            self.state.cancel_group();
        }

        /// Registers an image asset from raw bytes.
        /// Returns `true` on success.
        pub fn register_image_asset(&mut self, asset_ref: &str, data: &[u8]) -> bool {
            self.renderer.register_image_asset(asset_ref, data).is_ok()
        }

        /// Registers a font family for text rendering. Returns `true` on success.
        pub fn register_font(&mut self, family: &str, data: &[u8]) -> bool {
            self.renderer.register_font(family, data.to_vec()).is_ok()
        }

        /// Sets the camera zoom to an absolute level (clamped to [0.1, 100]).
        pub fn zoom_to(&mut self, level: f32) {
            self.renderer.camera_mut().set_zoom(level);
        }

        /// Multiplies the current camera zoom by a factor.
        pub fn zoom_by(&mut self, factor: f32) {
            self.renderer.camera_mut().zoom_by(factor);
        }

        /// Pans the camera by a delta in world coordinates.
        pub fn pan_by(&mut self, dx: f32, dy: f32) {
            self.renderer.camera_mut().pan_by(dx, dy);
        }

        /// Returns the current zoom level.
        pub fn get_zoom(&self) -> f32 {
            self.renderer.camera().zoom()
        }

        /// Applies a remote operation to a specific page, bypassing local
        /// `CommandHistory`. Used for collaborative editing to apply ops from
        /// other participants.
        pub fn apply_remote_op(&mut self, page_id: &str, descriptor_json: &str) -> bool {
            self.state.apply_remote_op(page_id, descriptor_json)
        }

        /// Applies a group of remote operations to a specific page, bypassing
        /// local `CommandHistory`.
        pub fn apply_remote_op_group(&mut self, page_id: &str, descriptors_json: &str) -> bool {
            self.state.apply_remote_op_group(page_id, descriptors_json)
        }

        /// Applies a remote page-level operation (add, remove, rename).
        pub fn apply_remote_page_op(
            &mut self,
            op_type: &str,
            page_id: &str,
            name: Option<String>,
            width: Option<f32>,
            height: Option<f32>,
        ) -> bool {
            self.state
                .apply_remote_page_op(op_type, page_id, name.as_deref(), width, height)
        }

        /// Computes the bounding box of all root nodes and sets camera to fit them.
        pub fn fit_to_all(&mut self) {
            let scene = &self.state.document.active_page().scene;
            let roots = scene.roots();
            if roots.is_empty() {
                return;
            }

            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;

            for &root_id in roots {
                if let Some(node) = scene.get(root_id) {
                    let b = &node.bounds;
                    min_x = min_x.min(b.x);
                    min_y = min_y.min(b.y);
                    max_x = max_x.max(b.x + b.width);
                    max_y = max_y.max(b.y + b.height);
                }
            }

            let content_w = max_x - min_x;
            let content_h = max_y - min_y;
            if content_w <= 0.0 || content_h <= 0.0 {
                return;
            }

            let (vp_w, vp_h) = self.renderer.camera().viewport_size();
            let margin = 0.9; // 10% padding on each side
            let zoom_x = (vp_w * margin) / content_w;
            let zoom_y = (vp_h * margin) / content_h;
            let zoom = zoom_x.min(zoom_y).clamp(0.1, 100.0);

            let center_x = (min_x + max_x) / 2.0;
            let center_y = (min_y + max_y) / 2.0;

            let cam = self.renderer.camera_mut();
            cam.set_zoom(zoom);
            cam.set_pan(
                center_x - vp_w / (2.0 * zoom),
                center_y - vp_h / (2.0 * zoom),
            );
        }

        // --- Enum variant lists for UI dropdowns ---

        /// Returns all BlendMode variant names as a JSON array of strings.
        /// Used by PropertyInspector to populate dropdown options.
        pub fn blend_mode_variants(&self) -> JsValue {
            serde_wasm_bindgen::to_value(&selean_engine::scene::BlendMode::ALL_VARIANTS)
                .unwrap_or(JsValue::NULL)
        }

        /// Returns all ClipMode variant names as a JSON array of strings.
        pub fn clip_mode_variants(&self) -> JsValue {
            serde_wasm_bindgen::to_value(&selean_engine::scene::ClipMode::ALL_VARIANTS)
                .unwrap_or(JsValue::NULL)
        }

        /// Returns all FontStyle variant names as a JSON array of strings.
        pub fn font_style_variants(&self) -> JsValue {
            serde_wasm_bindgen::to_value(&selean_engine::scene::FontStyle::ALL_VARIANTS)
                .unwrap_or(JsValue::NULL)
        }

        /// Returns all TextAlign variant names as a JSON array of strings.
        pub fn text_align_variants(&self) -> JsValue {
            serde_wasm_bindgen::to_value(&selean_engine::scene::TextAlign::ALL_VARIANTS)
                .unwrap_or(JsValue::NULL)
        }

        // --- Code generation bindings ---

        /// Generates React + Tailwind code for the active page.
        #[wasm_bindgen]
        pub fn generate_code(&self) -> String {
            self.state.generate_code()
        }

        /// Generates React + Tailwind code for a specific page by ID.
        ///
        /// Returns JSON: `{ "success": true, "code": "..." }` or
        /// `{ "success": false, "error": "..." }`.
        #[wasm_bindgen]
        pub fn generate_page_code(&self, page_id: &str) -> String {
            match self.state.generate_page_code(page_id) {
                Ok(code) => serde_json::json!({ "success": true, "code": code }).to_string(),
                Err(e) => serde_json::json!({ "success": false, "error": e }).to_string(),
            }
        }

        /// Generates a full Vite + React + Tailwind project as JSON.
        #[wasm_bindgen]
        pub fn generate_project_json(&self) -> String {
            self.state.generate_project_json()
        }

        /// Extracts design tokens as JSON.
        #[wasm_bindgen]
        pub fn extract_design_tokens_json(&self) -> String {
            self.state.extract_design_tokens_json()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use selean_engine::scene::BoundingBox;

    #[test]
    fn editor_state_new_has_empty_scene() {
        let state = EditorState::new();
        assert!(state.scene().is_empty());
        assert!(!state.can_undo());
        assert!(!state.can_redo());
    }

    #[test]
    fn setup_demo_scene_creates_five_nodes() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        assert_eq!(state.scene().len(), 5);
        assert_eq!(state.scene().roots().len(), 5);
    }

    #[test]
    fn execute_command_json_valid() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];

        let json = format!(
            r#"{{"type":"SetFill","node_id":"{id}","fill":{{"r":1.0,"g":0.0,"b":0.0,"a":1.0}}}}"#,
        );
        assert!(state.execute_command_json(&json));
        assert!(state.can_undo());
    }

    #[test]
    fn execute_command_json_invalid_json() {
        let mut state = EditorState::new();
        assert!(!state.execute_command_json("not json"));
    }

    #[test]
    fn undo_redo_cycle() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];

        let original_fill = state.scene().get(id).unwrap().fill;

        let desc = CommandDescriptor::SetFill {
            node_id: id,
            fill: Some(Color::WHITE),
        };
        state.execute_descriptor(desc);
        assert_ne!(state.scene().get(id).unwrap().fill, original_fill);

        state.undo();
        assert_eq!(state.scene().get(id).unwrap().fill, original_fill);

        state.redo();
        assert_ne!(state.scene().get(id).unwrap().fill, original_fill);
    }

    #[test]
    fn command_group_for_drag() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];

        let original_bounds = state.scene().get(id).unwrap().bounds;

        state.begin_group("Drag");
        state.execute_descriptor(CommandDescriptor::SetBounds {
            node_id: id,
            bounds: BoundingBox::new(10.0, 10.0, 150.0, 100.0),
        });
        state.execute_descriptor(CommandDescriptor::SetBounds {
            node_id: id,
            bounds: BoundingBox::new(20.0, 20.0, 150.0, 100.0),
        });
        state.end_group();

        // Single undo reverts both
        assert_eq!(state.history().undo_count(), 1);
        state.undo();
        assert_eq!(state.scene().get(id).unwrap().bounds, original_bounds);
    }

    #[test]
    fn get_node_json_returns_data() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let json = state.get_node_json(&id.to_string());
        assert!(json.contains("Red Box"));
    }

    #[test]
    fn get_selected_ids_empty_initially() {
        let state = EditorState::new();
        let json = state.get_selected_ids();
        assert_eq!(json, "[]");
    }

    #[test]
    fn get_scene_json_returns_nodes() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let json = state.get_scene_json();
        assert!(json.contains("\"node_count\":5"));
    }

    #[test]
    fn pointer_button_mapping() {
        assert_eq!(EditorState::pointer_button(0), PointerButton::Left);
        assert_eq!(EditorState::pointer_button(1), PointerButton::Middle);
        assert_eq!(EditorState::pointer_button(2), PointerButton::Right);
        assert_eq!(EditorState::pointer_button(99), PointerButton::Left);
    }

    #[test]
    fn modifiers_construction() {
        let m = EditorState::modifiers(true, false, true, false);
        assert!(m.shift);
        assert!(!m.ctrl);
        assert!(m.alt);
        assert!(!m.meta);
    }

    #[test]
    fn execute_tool_call_set_fill() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];

        let args = format!(r#"{{"node_id":"{id}","r":0.0,"g":1.0,"b":0.0,"a":1.0}}"#);
        let result = state.execute_tool_call("set_fill", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);

        let node = state.scene().get(id).unwrap();
        let fill = node.fill.unwrap();
        assert!((fill.g - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn execute_tool_call_create_node() {
        let mut state = EditorState::new();
        let args = r#"{"name":"Test","kind":"Frame","x":0,"y":0,"width":100,"height":100}"#;
        let result = state.execute_tool_call("create_node", args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(state.scene().len(), 1);
    }

    #[test]
    fn execute_tool_call_unknown_tool() {
        let mut state = EditorState::new();
        let result = state.execute_tool_call("nonexistent", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["error"].as_str().unwrap().contains("unknown tool"));
    }

    #[test]
    fn execute_tool_call_invalid_json() {
        let mut state = EditorState::new();
        let result = state.execute_tool_call("set_fill", "not json");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
    }

    #[test]
    fn execute_tool_call_get_scene_summary() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let result = state.execute_tool_call("get_scene_summary", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"]["node_count"], 5);
    }

    #[test]
    fn execute_tool_call_get_node() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let args = format!(r#"{{"node_id":"{id}"}}"#);
        let result = state.execute_tool_call("get_node", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["result"]["name"].as_str().unwrap().contains("Red"));
    }

    #[test]
    fn execute_tool_call_query_nodes() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let result = state.execute_tool_call("query_nodes", r#"{"name_pattern":"box"}"#);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn execute_tool_call_multi_command_group() {
        let mut state = EditorState::new();
        state.setup_demo_scene();

        // set_text produces two descriptors when font_size is included (SetTextContent + SetFontSize).
        // First create a Text node.
        let create_args = r#"{"name":"Label","kind":"Text","x":0,"y":0,"width":200,"height":30,"text_content":"Hello"}"#;
        state.execute_tool_call("create_node", create_args);
        assert_eq!(state.scene().len(), 6);

        // Find the text node.
        let text_id = state
            .scene()
            .roots()
            .iter()
            .find(|&&rid| state.scene().get(rid).is_some_and(|n| n.name == "Label"))
            .copied()
            .unwrap();

        let args = format!(r#"{{"node_id":"{text_id}","content":"World","font_size":24}}"#);
        let result = state.execute_tool_call("set_text", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"]["executed"], 2);

        // Single undo should revert both.
        state.undo();
        let node = state.scene().get(text_id).unwrap();
        match &node.kind {
            selean_engine::scene::SceneNodeKind::Text {
                content, font_size, ..
            } => {
                assert_eq!(content, "Hello");
                assert!((font_size - 16.0).abs() < f32::EPSILON);
            }
            _ => panic!("expected Text node"),
        }
    }

    // --- Phase 12: New tests for Document-based EditorState ---

    #[test]
    fn get_pages_json_returns_default_page() {
        let state = EditorState::new();
        let json = state.get_pages_json();
        let pages: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0]["name"], "Page 1");
        assert_eq!(pages[0]["node_count"], 0);
    }

    #[test]
    fn add_page_increases_count() {
        let mut state = EditorState::new();
        let id_str = state.add_page("Page 2", 800.0, 600.0);
        assert!(!id_str.is_empty());
        let pages: Vec<serde_json::Value> = serde_json::from_str(&state.get_pages_json()).unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[1]["name"], "Page 2");
    }

    #[test]
    fn remove_page_decreases_count() {
        let mut state = EditorState::new();
        let id_str = state.add_page("Temp", 800.0, 600.0);
        assert!(state.remove_page(&id_str));
        let pages: Vec<serde_json::Value> = serde_json::from_str(&state.get_pages_json()).unwrap();
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn remove_last_page_fails() {
        let mut state = EditorState::new();
        let pages: Vec<serde_json::Value> = serde_json::from_str(&state.get_pages_json()).unwrap();
        let id = pages[0]["id"].as_str().unwrap();
        assert!(!state.remove_page(id));
    }

    #[test]
    fn set_active_page_switches_scene() {
        let mut state = EditorState::new();
        state.setup_demo_scene(); // 5 nodes on page 1
        let page2_id = state.add_page("Page 2", 800.0, 600.0);
        assert!(state.set_active_page(&page2_id));
        assert_eq!(state.scene().len(), 0); // page 2 is empty
        // Switch back
        let pages: Vec<serde_json::Value> = serde_json::from_str(&state.get_pages_json()).unwrap();
        let page1_id = pages[0]["id"].as_str().unwrap();
        assert!(state.set_active_page(page1_id));
        assert_eq!(state.scene().len(), 5);
    }

    #[test]
    fn undo_per_page_isolation() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        // Make a change on page 1
        state.execute_descriptor(CommandDescriptor::SetFill {
            node_id: id,
            fill: Some(Color::WHITE),
        });
        assert!(state.can_undo());

        // Switch to page 2
        let page2_id = state.add_page("Page 2", 800.0, 600.0);
        assert!(state.set_active_page(&page2_id));
        // Page 2 has no undo
        assert!(!state.can_undo());
    }

    #[test]
    fn import_document_replaces_state() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        assert_eq!(state.scene().len(), 5);

        // Export, add a page, re-import
        let mut doc = Document::new();
        doc.add_page("Imported Page 2", 1280.0, 720.0);
        let json = selean_engine::persistence::save_document(&doc).unwrap();
        assert!(state.import_document(&json));
        assert_eq!(state.document.page_count(), 2);
        assert_eq!(state.scene().len(), 0); // empty document
    }

    #[test]
    fn export_document_json_roundtrips() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        state.add_page("Page 2", 800.0, 600.0);

        let json = state.export_document_json();
        let mut state2 = EditorState::new();
        assert!(state2.import_document(&json));
        assert_eq!(state2.document.page_count(), 2);
        assert_eq!(state2.scene().len(), 5); // page 1 has demo scene
    }

    #[test]
    fn execute_tool_call_get_pages() {
        let mut state = EditorState::new();
        let result = state.execute_tool_call("get_pages", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        let pages = parsed["result"].as_array().unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0]["name"], "Page 1");
    }

    #[test]
    fn execute_tool_call_add_page() {
        let mut state = EditorState::new();
        let result = state.execute_tool_call(
            "add_page",
            r#"{"name":"Slide 2","width":1920,"height":1080}"#,
        );
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["result"]["page_id"].as_str().is_some());
        assert_eq!(state.document.page_count(), 2);
    }

    #[test]
    fn execute_tool_call_set_active_page() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let page2_id = state.add_page("Page 2", 800.0, 600.0);
        let result =
            state.execute_tool_call("set_active_page", &format!(r#"{{"page_id":"{page2_id}"}}"#));
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(state.scene().len(), 0);
    }

    #[test]
    fn execute_tool_call_remove_page() {
        let mut state = EditorState::new();
        let page2_id = state.add_page("Temp", 800.0, 600.0);
        let result =
            state.execute_tool_call("remove_page", &format!(r#"{{"page_id":"{page2_id}"}}"#));
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(state.document.page_count(), 1);
    }

    #[test]
    fn clear_selection_empties_set() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        state.input.state_mut().selection.select_one(id);
        assert!(!state.input.state().selection.is_empty());
        state.clear_selection();
        assert!(state.input.state().selection.is_empty());
    }

    #[test]
    fn get_scene_tree_json_hierarchy() {
        let mut state = EditorState::new();
        // Add a parent and a child
        let parent = create_frame_node(
            "Parent",
            0.0,
            0.0,
            200.0,
            200.0,
            Some(Color::WHITE),
            [0.0; 4],
        );
        let parent_id = parent.id;
        state.scene_mut().add_root(parent);
        let child = create_frame_node(
            "Child",
            10.0,
            10.0,
            50.0,
            50.0,
            Some(Color::WHITE),
            [0.0; 4],
        );
        state.scene_mut().add_child(parent_id, child);

        let json = state.get_scene_tree_json();
        let tree: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0]["name"], "Parent");
        assert_eq!(tree[0]["children"].as_array().unwrap().len(), 1);
        assert_eq!(tree[0]["children"][0]["name"], "Child");
    }

    #[test]
    fn cancel_group_reverts_all_commands() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let original_bounds = state.scene().get(id).unwrap().bounds;

        state.begin_group("Resize");
        state.execute_descriptor(CommandDescriptor::SetBounds {
            node_id: id,
            bounds: BoundingBox::new(50.0, 50.0, 200.0, 200.0),
        });
        state.execute_descriptor(CommandDescriptor::SetBounds {
            node_id: id,
            bounds: BoundingBox::new(60.0, 60.0, 250.0, 250.0),
        });
        state.cancel_group();

        assert_eq!(state.scene().get(id).unwrap().bounds, original_bounds);
        // No undo entry should exist for the cancelled group.
        assert!(!state.can_undo());
    }

    #[test]
    fn align_nodes_left() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let ids: Vec<String> = state
            .scene()
            .roots()
            .iter()
            .take(2)
            .map(std::string::ToString::to_string)
            .collect();
        let json = serde_json::to_string(&ids).unwrap();
        assert!(state.align_nodes(&json, "Left"));
        // Both should now have the same x
        let b0 = state.scene().get(state.scene().roots()[0]).unwrap().bounds;
        let b1 = state.scene().get(state.scene().roots()[1]).unwrap().bounds;
        assert!((b0.x - b1.x).abs() < f32::EPSILON);
    }

    #[test]
    fn align_nodes_invalid_kind() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let ids: Vec<String> = state
            .scene()
            .roots()
            .iter()
            .take(2)
            .map(std::string::ToString::to_string)
            .collect();
        let json = serde_json::to_string(&ids).unwrap();
        assert!(!state.align_nodes(&json, "InvalidKind"));
    }

    #[test]
    fn align_nodes_single_node_fails() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let ids = vec![state.scene().roots()[0].to_string()];
        let json = serde_json::to_string(&ids).unwrap();
        assert!(!state.align_nodes(&json, "Left"));
    }

    #[test]
    fn select_node_by_id_valid() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        assert!(state.select_node_by_id(&id.to_string()));
        assert!(state.input.state().selection.contains(id));
    }

    #[test]
    fn select_node_by_id_invalid_returns_false() {
        let mut state = EditorState::new();
        assert!(!state.select_node_by_id("not-a-uuid"));
        assert!(!state.select_node_by_id("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn get_zoom_default() {
        let state = EditorState::new();
        assert!((state.get_zoom() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn execute_rotation_tool_success() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "angle_degrees": 45.0
        });
        let result = state.execute_rotation_tool(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        // Verify the transform was changed from identity
        let node = state.scene().get(id).unwrap();
        assert!(!node.local_transform.is_identity());
    }

    #[test]
    fn execute_rotation_tool_zero_degrees() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "angle_degrees": 0.0
        });
        let result = state.execute_rotation_tool(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
    }

    #[test]
    fn execute_rotation_tool_invalid_node() {
        let mut state = EditorState::new();
        let args = serde_json::json!({
            "node_id": "00000000-0000-0000-0000-000000000000",
            "angle_degrees": 45.0
        });
        let result = state.execute_rotation_tool(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
    }

    #[test]
    fn execute_rotation_tool_invalid_uuid() {
        let mut state = EditorState::new();
        let args = serde_json::json!({
            "node_id": "not-a-uuid",
            "angle_degrees": 45.0
        });
        let result = state.execute_rotation_tool(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
    }

    #[test]
    fn execute_rotation_tool_undo_restores_transform() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let original = state.scene().get(id).unwrap().local_transform;

        let args = serde_json::json!({
            "node_id": id.to_string(),
            "angle_degrees": 90.0
        });
        state.execute_rotation_tool(&args);
        assert_ne!(state.scene().get(id).unwrap().local_transform, original);

        state.undo();
        assert_eq!(state.scene().get(id).unwrap().local_transform, original);
    }

    #[test]
    fn group_nodes_success() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let ids: Vec<String> = state
            .scene()
            .roots()
            .iter()
            .take(2)
            .map(std::string::ToString::to_string)
            .collect();
        let args = serde_json::json!({ "node_ids": ids });
        let result = state.execute_group_nodes(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["result"]["group_id"].as_str().is_some());
        // The two nodes should now be children of the group.
        // Roots should be: remaining 3 original + 1 group = 4
        assert_eq!(state.scene().roots().len(), 4);
    }

    #[test]
    fn group_nodes_too_few() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let args = serde_json::json!({
            "node_ids": [state.scene().roots()[0].to_string()]
        });
        let result = state.execute_group_nodes(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
    }

    #[test]
    fn group_nodes_via_tool_call() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let ids: Vec<String> = state
            .scene()
            .roots()
            .iter()
            .take(3)
            .map(std::string::ToString::to_string)
            .collect();
        let args = serde_json::json!({ "node_ids": ids });
        let result = state.execute_tool_call("group_nodes", &serde_json::to_string(&args).unwrap());
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
    }

    #[test]
    fn group_nodes_undo_restores() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let original_roots = state.scene().roots().to_vec();
        let ids: Vec<String> = original_roots
            .iter()
            .take(2)
            .map(std::string::ToString::to_string)
            .collect();
        let args = serde_json::json!({ "node_ids": ids });
        state.execute_group_nodes(&args);
        // Undo the group (single undo for the command group).
        state.undo();
        assert_eq!(state.scene().roots().len(), 5);
    }

    #[test]
    fn ungroup_node_success() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let ids: Vec<String> = state
            .scene()
            .roots()
            .iter()
            .take(2)
            .map(std::string::ToString::to_string)
            .collect();

        // First group them.
        let args = serde_json::json!({ "node_ids": ids });
        let result = state.execute_group_nodes(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let group_id = parsed["result"]["group_id"].as_str().unwrap();

        // Now ungroup.
        let args = serde_json::json!({ "node_id": group_id });
        let result = state.execute_ungroup_node(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
    }

    #[test]
    fn ungroup_preserves_grandchildren() {
        use selean_engine::scene::{BoundingBox, SceneNode, SceneNodeKind};

        let mut state = EditorState::new();

        // Build a hierarchy: Group -> Child -> Grandchild
        let grandchild_id = selean_common::types::NodeId::new();
        let grandchild = SceneNode::new(
            grandchild_id,
            "Grandchild".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(5.0, 5.0, 20.0, 20.0),
        );

        let child_id = selean_common::types::NodeId::new();
        let child = SceneNode::new(
            child_id,
            "Child".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );

        let group_id = selean_common::types::NodeId::new();
        let group = SceneNode::new(
            group_id,
            "Group".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );

        // Manually build the hierarchy in the scene graph.
        state.scene_mut().add_root(group);
        state.scene_mut().add_child(group_id, child);
        state.scene_mut().add_child(child_id, grandchild);

        // Verify hierarchy before ungroup.
        assert_eq!(state.scene().children(group_id).unwrap(), &[child_id]);
        assert_eq!(state.scene().children(child_id).unwrap(), &[grandchild_id]);

        // Ungroup the root-level group.
        let args = serde_json::json!({ "node_id": group_id.to_string() });
        let result = state.execute_ungroup_node(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);

        // Child should now be a root.
        assert!(state.scene().roots().contains(&child_id));
        // Group should be gone.
        assert!(!state.scene().contains(group_id));
        // Grandchild must still exist as a child of Child.
        assert!(state.scene().contains(grandchild_id));
        assert_eq!(state.scene().parent(grandchild_id), Some(child_id));
        assert_eq!(state.scene().children(child_id).unwrap(), &[grandchild_id]);
    }

    #[test]
    fn ungroup_non_group_fails() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let args = serde_json::json!({ "node_id": id.to_string() });
        let result = state.execute_ungroup_node(&args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["error"].as_str().unwrap().contains("not a Group"));
    }

    #[test]
    fn ungroup_via_tool_call() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let ids: Vec<String> = state
            .scene()
            .roots()
            .iter()
            .take(2)
            .map(std::string::ToString::to_string)
            .collect();
        let group_args = serde_json::json!({ "node_ids": ids });
        let result = state.execute_group_nodes(&group_args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let group_id = parsed["result"]["group_id"].as_str().unwrap();

        let args = format!(r#"{{"node_id":"{group_id}"}}"#);
        let result = state.execute_tool_call("ungroup_node", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
    }

    #[test]
    fn execute_rotation_tool_via_execute_tool_call() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let args = format!(r#"{{"node_id":"{id}","angle_degrees":30}}"#);
        let result = state.execute_tool_call("set_rotation", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
    }

    // --- Z-order tool tests ---

    #[test]
    fn z_order_move_to_front() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let roots = state.scene().roots().to_vec();
        let first_id = roots[0];
        let last_id = *roots.last().unwrap();

        let args = serde_json::json!({ "node_id": first_id.to_string() });
        let result = state.execute_z_order_tool("move_to_front", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"]["moved"], true);

        // First root should now be last.
        let new_roots = state.scene().roots().to_vec();
        assert_eq!(*new_roots.last().unwrap(), first_id);
        // Original last should be second-to-last.
        assert_eq!(new_roots[new_roots.len() - 2], last_id);
    }

    #[test]
    fn z_order_move_to_back() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let roots = state.scene().roots().to_vec();
        let last_id = *roots.last().unwrap();

        let args = serde_json::json!({ "node_id": last_id.to_string() });
        let result = state.execute_z_order_tool("move_to_back", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);

        assert_eq!(state.scene().roots()[0], last_id);
    }

    #[test]
    fn z_order_move_forward() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let roots = state.scene().roots().to_vec();
        let id = roots[1]; // second node

        let args = serde_json::json!({ "node_id": id.to_string() });
        let result = state.execute_z_order_tool("move_forward", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);

        // Should now be at index 2.
        assert_eq!(state.scene().roots()[2], id);
    }

    #[test]
    fn z_order_move_backward() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let roots = state.scene().roots().to_vec();
        let id = roots[2]; // third node

        let args = serde_json::json!({ "node_id": id.to_string() });
        let result = state.execute_z_order_tool("move_backward", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);

        // Should now be at index 1.
        assert_eq!(state.scene().roots()[1], id);
    }

    #[test]
    fn z_order_already_at_front_is_noop() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let roots = state.scene().roots().to_vec();
        let last_id = *roots.last().unwrap();

        let args = serde_json::json!({ "node_id": last_id.to_string() });
        let result = state.execute_z_order_tool("move_to_front", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"]["moved"], false);

        // Roots unchanged.
        assert_eq!(state.scene().roots(), &roots);
    }

    #[test]
    fn z_order_already_at_back_is_noop() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let roots = state.scene().roots().to_vec();
        let first_id = roots[0];

        let args = serde_json::json!({ "node_id": first_id.to_string() });
        let result = state.execute_z_order_tool("move_to_back", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"]["moved"], false);
    }

    #[test]
    fn z_order_undo_restores_order() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let original_roots = state.scene().roots().to_vec();
        let first_id = original_roots[0];

        let args = serde_json::json!({ "node_id": first_id.to_string() });
        state.execute_z_order_tool("move_to_front", &args);
        assert_ne!(state.scene().roots(), &original_roots);

        state.undo();
        assert_eq!(state.scene().roots(), &original_roots);
    }

    #[test]
    fn z_order_via_execute_tool_call() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let roots = state.scene().roots().to_vec();
        let first_id = roots[0];

        let args = format!(r#"{{"node_id":"{first_id}"}}"#);
        let result = state.execute_tool_call("move_forward", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);

        // Should be at index 1 now.
        assert_eq!(state.scene().roots()[1], first_id);
    }

    #[test]
    fn z_order_invalid_node() {
        let mut state = EditorState::new();
        let args = serde_json::json!({ "node_id": "00000000-0000-0000-0000-000000000000" });
        let result = state.execute_z_order_tool("move_forward", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
    }

    #[test]
    fn compute_z_reorder_forward() {
        use selean_common::types::NodeId;
        let a = NodeId::new();
        let b = NodeId::new();
        let c = NodeId::new();
        let siblings = vec![a, b, c];

        let result = compute_z_reorder(&siblings, a, "move_forward");
        assert_eq!(result, Some(vec![b, a, c]));
    }

    #[test]
    fn compute_z_reorder_unknown_tool() {
        use selean_common::types::NodeId;
        let a = NodeId::new();
        let siblings = vec![a];
        assert_eq!(compute_z_reorder(&siblings, a, "unknown"), None);
    }

    // --- Remote op application tests (collab) ---

    #[test]
    fn apply_remote_op_sets_fill_without_undo_entry() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let page_id = state.document.active_page().id.to_string();

        let desc_json = format!(
            r#"{{"type":"SetFill","node_id":"{id}","fill":{{"r":0.0,"g":1.0,"b":0.0,"a":1.0}}}}"#,
        );
        assert!(state.apply_remote_op(&page_id, &desc_json));

        // Fill should be changed.
        let fill = state.scene().get(id).unwrap().fill.unwrap();
        assert!((fill.g - 1.0).abs() < f32::EPSILON);

        // No undo entry since remote ops bypass CommandHistory.
        assert!(!state.can_undo());
    }

    #[test]
    fn apply_remote_op_invalid_page_id() {
        let mut state = EditorState::new();
        assert!(!state.apply_remote_op("not-a-uuid", "{}"));
    }

    #[test]
    fn apply_remote_op_nonexistent_page() {
        let mut state = EditorState::new();
        assert!(!state.apply_remote_op(
            "00000000-0000-0000-0000-000000000000",
            r#"{"type":"SetFill","node_id":"00000000-0000-0000-0000-000000000001","fill":null}"#,
        ));
    }

    #[test]
    fn apply_remote_op_invalid_descriptor_json() {
        let mut state = EditorState::new();
        let page_id = state.document.active_page().id.to_string();
        assert!(!state.apply_remote_op(&page_id, "not json"));
    }

    #[test]
    fn apply_remote_op_group_applies_all() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene().roots()[0];
        let page_id = state.document.active_page().id.to_string();

        let descs = format!(
            r#"[{{"type":"SetFill","node_id":"{id}","fill":{{"r":1.0,"g":0.0,"b":0.0,"a":1.0}}}},{{"type":"SetOpacity","node_id":"{id}","opacity":0.5}}]"#,
        );
        assert!(state.apply_remote_op_group(&page_id, &descs));

        let node = state.scene().get(id).unwrap();
        let fill = node.fill.unwrap();
        assert!((fill.r - 1.0).abs() < f32::EPSILON);
        assert!((node.opacity - 0.5).abs() < f32::EPSILON);
        assert!(!state.can_undo());
    }

    #[test]
    fn apply_remote_op_group_invalid_json() {
        let mut state = EditorState::new();
        let page_id = state.document.active_page().id.to_string();
        assert!(!state.apply_remote_op_group(&page_id, "not json"));
    }

    #[test]
    fn apply_remote_op_on_different_page() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let page2_id = state.add_page("Page 2", 800.0, 600.0);

        // Create a node on page 2.
        state.set_active_page(&page2_id);
        let create_args =
            r#"{"name":"Remote","kind":"Frame","x":0,"y":0,"width":100,"height":100}"#;
        state.execute_tool_call("create_node", create_args);
        let id = state.scene().roots()[0];

        // Switch back to page 1.
        let pages: Vec<serde_json::Value> = serde_json::from_str(&state.get_pages_json()).unwrap();
        let page1_id = pages[0]["id"].as_str().unwrap().to_string();
        state.set_active_page(&page1_id);

        // Apply remote op to page 2 while page 1 is active.
        let desc_json = format!(r#"{{"type":"SetOpacity","node_id":"{id}","opacity":0.3}}"#,);
        assert!(state.apply_remote_op(&page2_id, &desc_json));

        // Verify the change on page 2.
        state.set_active_page(&page2_id);
        let node = state.scene().get(id).unwrap();
        assert!((node.opacity - 0.3).abs() < f32::EPSILON);
    }

    // -- apply_remote_page_op tests --

    #[test]
    fn remote_page_op_add() {
        let mut state = EditorState::new();
        assert_eq!(state.document.page_count(), 1);

        let page_id = selean_common::types::PageId::new().to_string();
        assert!(state.apply_remote_page_op(
            "add",
            &page_id,
            Some("Remote Page"),
            Some(800.0),
            Some(600.0)
        ));
        assert_eq!(state.document.page_count(), 2);

        // Verify the page has the right properties.
        let pid = uuid::Uuid::parse_str(&page_id).unwrap();
        let target = selean_common::types::PageId::from_uuid(pid);
        let page = state.document.page(target).unwrap();
        assert_eq!(page.name, "Remote Page");
        assert_eq!(page.width, 800.0);
        assert_eq!(page.height, 600.0);
    }

    #[test]
    fn remote_page_op_add_duplicate_returns_false() {
        let mut state = EditorState::new();
        let existing_id = state.document.active_page().id.to_string();
        assert!(!state.apply_remote_page_op("add", &existing_id, Some("Dup"), None, None));
        assert_eq!(state.document.page_count(), 1);
    }

    #[test]
    fn remote_page_op_remove() {
        let mut state = EditorState::new();
        let page2_id = state.add_page("Page 2", 800.0, 600.0);
        assert_eq!(state.document.page_count(), 2);

        assert!(state.apply_remote_page_op("remove", &page2_id, None, None, None));
        assert_eq!(state.document.page_count(), 1);
    }

    #[test]
    fn remote_page_op_rename() {
        let mut state = EditorState::new();
        let page_id = state.document.active_page().id.to_string();

        assert!(state.apply_remote_page_op("rename", &page_id, Some("Renamed"), None, None));
        assert_eq!(state.document.active_page().name, "Renamed");
    }

    #[test]
    fn remote_page_op_invalid_type() {
        let mut state = EditorState::new();
        assert!(!state.apply_remote_page_op("invalid", "some-id", None, None, None));
    }

    #[test]
    fn remote_page_op_add_default_dimensions() {
        let mut state = EditorState::new();
        let page_id = selean_common::types::PageId::new().to_string();
        assert!(state.apply_remote_page_op("add", &page_id, None, None, None));

        let pid = uuid::Uuid::parse_str(&page_id).unwrap();
        let target = selean_common::types::PageId::from_uuid(pid);
        let page = state.document.page(target).unwrap();
        assert_eq!(page.name, "New Page");
        assert_eq!(page.width, 1920.0);
        assert_eq!(page.height, 1080.0);
    }
}
