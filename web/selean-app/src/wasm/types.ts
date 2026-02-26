/**
 * Type definitions for the selean-wasm WASM module.
 *
 * These mirror the wasm_bindgen exports from crates/selean-wasm/src/lib.rs.
 * When wasm-pack generates the actual bindings, these types should match.
 */

export interface SeleanEditor {
  resize(width: number, height: number): void;
  render(): void;
  on_pointer_move(
    x: number,
    y: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  on_pointer_down(
    x: number,
    y: number,
    button: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  on_pointer_up(
    x: number,
    y: number,
    button: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  on_scroll(
    x: number,
    y: number,
    dx: number,
    dy: number,
    shift: boolean,
    ctrl: boolean,
    alt: boolean,
    meta: boolean,
  ): string;
  execute_command(json: string): boolean;
  undo(): boolean;
  redo(): boolean;
  get_node_json(nodeId: string): string;
  get_selected_ids(): string;
  get_scene_json(): string;
}

export type EditorStatus = "loading" | "ready" | "error" | "unsupported";

/** Mirrors NodeInfo from crates/selean-wasm/src/queries.rs */
export interface NodeInfo {
  id: string;
  name: string;
  kind: string;
  x: number;
  y: number;
  width: number;
  height: number;
  fill: [number, number, number, number] | null;
  stroke: [number, number, number, number] | null;
  stroke_width: number;
  opacity: number;
  visible: boolean;
  blend_mode: string;
  clip_mode: string;
  transform: [number, number, number, number, number, number];
  scroll_offset: [number, number];
  corner_radius: [number, number, number, number];
  text_content: string | null;
  font_size: number | null;
  asset_ref: string | null;
  path_data: string | null;
  children: string[];
  parent: string | null;
}

/** Mirrors SceneInfo from crates/selean-wasm/src/queries.rs */
export interface SceneInfo {
  nodes: NodeInfo[];
  roots: string[];
  node_count: number;
}

/** A chat message in the conversation */
export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

/** SSE event from the server */
export type ChatEvent =
  | { type: "text"; text: string }
  | { type: "tool_use"; id: string; name: string; input: unknown }
  | { type: "done"; stop_reason: string }
  | { type: "error"; message: string };
