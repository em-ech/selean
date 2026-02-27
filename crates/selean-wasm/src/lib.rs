//! WASM binding layer for the Selean design platform.
//!
//! Provides the [`SeleanEditor`] struct as the single entry point exposed
//! to JavaScript via `wasm-bindgen`. Owns the `SceneGraph`, `Renderer`,
//! `Camera`, `InputHandler`, and `CommandHistory`.

pub mod command_descriptor;
pub mod queries;

use selean_engine::command::CommandHistory;
use selean_engine::input::{InputHandler, Modifiers, PointerButton};
use selean_engine::scene::{Color, SceneGraph};

use command_descriptor::{CommandDescriptor, create_frame_node};
use queries::{get_node_json, get_scene_json, get_selected_ids_json, query_nodes_json};

/// Core editor state, independent of the WASM runtime.
///
/// This struct holds all engine state. On native targets it can be used
/// directly; on WASM it is wrapped by the `wasm_bindgen`-exported
/// `SeleanEditor` facade below.
pub struct EditorState {
    /// The scene graph (document model).
    pub scene: SceneGraph,
    /// Input handler (hover, selection, drag, camera).
    pub input: InputHandler,
    /// Undo/redo history.
    pub history: CommandHistory,
}

impl EditorState {
    /// Creates a new editor with an empty scene.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scene: SceneGraph::new(),
            input: InputHandler::new(),
            history: CommandHistory::new(),
        }
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
        self.history.execute(cmd, &mut self.scene)
    }

    /// Executes a [`CommandDescriptor`] directly.
    pub fn execute_descriptor(&mut self, desc: CommandDescriptor) -> bool {
        let cmd = desc.into_command();
        self.history.execute(cmd, &mut self.scene)
    }

    /// Undoes the last command.
    pub fn undo(&mut self) -> bool {
        self.history.undo(&mut self.scene)
    }

    /// Redoes the last undone command.
    pub fn redo(&mut self) -> bool {
        self.history.redo(&mut self.scene)
    }

    /// Begins a command group (for drag operations or LLM batches).
    pub fn begin_group(&mut self, label: &str) {
        self.history.begin_group(label);
    }

    /// Ends the active command group.
    pub fn end_group(&mut self) {
        self.history.end_group();
    }

    /// Cancels the active command group, undoing all commands in it.
    pub fn cancel_group(&mut self) {
        self.history.cancel_group(&mut self.scene);
    }

    /// Returns whether undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Returns whether redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Returns JSON for a single node.
    #[must_use]
    pub fn get_node_json(&self, node_id: &str) -> String {
        get_node_json(&self.scene, node_id)
    }

    /// Returns JSON array of selected node IDs.
    #[must_use]
    pub fn get_selected_ids(&self) -> String {
        get_selected_ids_json(self.input.state().selection.ids())
    }

    /// Returns JSON representation of the full scene.
    #[must_use]
    pub fn get_scene_json(&self) -> String {
        get_scene_json(&self.scene)
    }

    /// Executes an LLM tool call by name and JSON args string.
    ///
    /// Returns a JSON string: `{ "success": true/false, "result": ... }`.
    /// For read-only tools, the result contains the query data.
    /// For mutation tools, the result contains `{ "executed": N }`.
    pub fn execute_tool_call(&mut self, tool_name: &str, args_json: &str) -> String {
        let args: serde_json::Value = match serde_json::from_str(args_json) {
            Ok(v) => v,
            Err(e) => {
                return serde_json::json!({
                    "success": false,
                    "error": format!("invalid JSON args: {e}")
                })
                .to_string();
            }
        };

        // Handle read-only tools directly.
        if selean_llm::is_read_only_tool(tool_name) {
            let result = match tool_name {
                "get_scene_summary" => get_scene_json(&self.scene),
                "get_node" => {
                    let node_id = args
                        .get("node_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    get_node_json(&self.scene, node_id)
                }
                "query_nodes" => {
                    let name_pattern = args.get("name_pattern").and_then(serde_json::Value::as_str);
                    let kind = args.get("kind").and_then(serde_json::Value::as_str);
                    query_nodes_json(&self.scene, name_pattern, kind)
                }
                _ => "null".to_string(),
            };
            return serde_json::json!({
                "success": true,
                "result": serde_json::from_str::<serde_json::Value>(&result).unwrap_or(serde_json::Value::Null)
            })
            .to_string();
        }

        // Map tool call to command descriptors.
        let descriptors = match selean_llm::map_tool_call(tool_name, &args) {
            Ok(descs) => descs,
            Err(e) => {
                return serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                })
                .to_string();
            }
        };

        // Wrap multiple descriptors in a group.
        let use_group = descriptors.len() > 1;
        if use_group {
            self.history.begin_group(tool_name);
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
                self.history.cancel_group(&mut self.scene);
            } else {
                self.history.end_group();
            }
        }

        serde_json::json!({
            "success": !failed,
            "result": { "executed": executed }
        })
        .to_string()
    }

    /// Sets up a demo scene with colored rectangles.
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
            self.scene.add_root(node);
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
            self.renderer.render_frame(&mut self.state.scene, &view);
            frame.present();
        }

        /// Handles pointer move events from JavaScript.
        pub fn on_pointer_move(
            &mut self,
            x: f32,
            y: f32,
            shift: bool,
            ctrl: bool,
            alt: bool,
            meta: bool,
        ) -> String {
            let event = InputEvent::PointerMove {
                x,
                y,
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                &mut self.state.scene,
                self.renderer.camera_mut(),
            );
            serde_json::to_string(&events).unwrap_or_default()
        }

        /// Handles pointer down events from JavaScript.
        pub fn on_pointer_down(
            &mut self,
            x: f32,
            y: f32,
            button: u32,
            shift: bool,
            ctrl: bool,
            alt: bool,
            meta: bool,
        ) -> String {
            let event = InputEvent::PointerDown {
                x,
                y,
                button: EditorState::pointer_button(button),
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                &mut self.state.scene,
                self.renderer.camera_mut(),
            );
            serde_json::to_string(&events).unwrap_or_default()
        }

        /// Handles pointer up events from JavaScript.
        pub fn on_pointer_up(
            &mut self,
            x: f32,
            y: f32,
            button: u32,
            shift: bool,
            ctrl: bool,
            alt: bool,
            meta: bool,
        ) -> String {
            let event = InputEvent::PointerUp {
                x,
                y,
                button: EditorState::pointer_button(button),
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                &mut self.state.scene,
                self.renderer.camera_mut(),
            );
            serde_json::to_string(&events).unwrap_or_default()
        }

        /// Handles scroll events from JavaScript.
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
        ) -> String {
            let event = InputEvent::ScrollDelta {
                x,
                y,
                dx,
                dy,
                modifiers: EditorState::modifiers(shift, ctrl, alt, meta),
            };
            let events = self.state.input.handle_event(
                &event,
                &mut self.state.scene,
                self.renderer.camera_mut(),
            );
            serde_json::to_string(&events).unwrap_or_default()
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

        /// Returns JSON array of selected node IDs.
        pub fn get_selected_ids(&self) -> String {
            self.state.get_selected_ids()
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

        /// Returns whether undo is available.
        pub fn can_undo(&self) -> bool {
            self.state.can_undo()
        }

        /// Returns whether redo is available.
        pub fn can_redo(&self) -> bool {
            self.state.can_redo()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use selean_engine::scene::BoundingBox;

    #[test]
    fn editor_state_new_has_empty_scene() {
        let state = EditorState::new();
        assert!(state.scene.is_empty());
        assert!(!state.can_undo());
        assert!(!state.can_redo());
    }

    #[test]
    fn setup_demo_scene_creates_five_nodes() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        assert_eq!(state.scene.len(), 5);
        assert_eq!(state.scene.roots().len(), 5);
    }

    #[test]
    fn execute_command_json_valid() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene.roots()[0];

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
        let id = state.scene.roots()[0];

        let original_fill = state.scene.get(id).unwrap().fill;

        let desc = CommandDescriptor::SetFill {
            node_id: id,
            fill: Some(Color::WHITE),
        };
        state.execute_descriptor(desc);
        assert_ne!(state.scene.get(id).unwrap().fill, original_fill);

        state.undo();
        assert_eq!(state.scene.get(id).unwrap().fill, original_fill);

        state.redo();
        assert_ne!(state.scene.get(id).unwrap().fill, original_fill);
    }

    #[test]
    fn command_group_for_drag() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene.roots()[0];

        let original_bounds = state.scene.get(id).unwrap().bounds;

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
        assert_eq!(state.history.undo_count(), 1);
        state.undo();
        assert_eq!(state.scene.get(id).unwrap().bounds, original_bounds);
    }

    #[test]
    fn get_node_json_returns_data() {
        let mut state = EditorState::new();
        state.setup_demo_scene();
        let id = state.scene.roots()[0];
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
        let id = state.scene.roots()[0];

        let args = format!(r#"{{"node_id":"{id}","r":0.0,"g":1.0,"b":0.0,"a":1.0}}"#);
        let result = state.execute_tool_call("set_fill", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);

        let node = state.scene.get(id).unwrap();
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
        assert_eq!(state.scene.len(), 1);
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
        let id = state.scene.roots()[0];
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
        assert_eq!(state.scene.len(), 6);

        // Find the text node.
        let text_id = state
            .scene
            .roots()
            .iter()
            .find(|&&rid| state.scene.get(rid).is_some_and(|n| n.name == "Label"))
            .copied()
            .unwrap();

        let args = format!(r#"{{"node_id":"{text_id}","content":"World","font_size":24}}"#);
        let result = state.execute_tool_call("set_text", &args);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["result"]["executed"], 2);

        // Single undo should revert both.
        state.undo();
        let node = state.scene.get(text_id).unwrap();
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
}
