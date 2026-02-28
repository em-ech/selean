import type { SeleanEditor, NodeInfo, CameraInfo } from "../wasm/types";
import { vi } from "vitest";

/** Default camera state returned by the mock. */
export const DEFAULT_CAMERA: CameraInfo = {
  pan_x: 0,
  pan_y: 0,
  zoom: 1,
  viewport_width: 1920,
  viewport_height: 1080,
};

/** A minimal NodeInfo with sensible defaults. Override fields as needed. */
export function makeNodeInfo(overrides: Partial<NodeInfo> = {}): NodeInfo {
  return {
    id: "node-1",
    name: "Rectangle",
    kind: "Frame",
    x: 100,
    y: 200,
    width: 300,
    height: 150,
    fill: [0.5, 0.5, 0.5, 1],
    stroke: null,
    stroke_width: 0,
    opacity: 1,
    visible: true,
    blend_mode: "Normal",
    clip_mode: "None",
    transform: [1, 0, 0, 1, 0, 0],
    scroll_offset: [0, 0],
    corner_radius: [0, 0, 0, 0],
    text_content: null,
    font_size: null,
    asset_ref: null,
    path_data: null,
    font_family: null,
    font_weight: null,
    font_style: null,
    text_align: null,
    line_height: null,
    text_color: null,
    children: [],
    parent: null,
    ...overrides,
  };
}

/** Creates a mock SeleanEditor with all methods stubbed via vi.fn(). */
export function createMockEditor(
  overrides: Partial<Record<keyof SeleanEditor, unknown>> = {},
): SeleanEditor {
  const defaults: SeleanEditor = {
    resize: vi.fn(),
    render: vi.fn(),
    on_pointer_move: vi.fn().mockReturnValue("[]"),
    on_pointer_down: vi.fn().mockReturnValue("[]"),
    on_pointer_up: vi.fn().mockReturnValue("[]"),
    on_scroll: vi.fn().mockReturnValue("[]"),
    execute_command: vi.fn().mockReturnValue(true),
    undo: vi.fn().mockReturnValue(true),
    redo: vi.fn().mockReturnValue(true),
    get_node_json: vi.fn().mockReturnValue("null"),
    get_selected_ids: vi.fn().mockReturnValue("[]"),
    get_scene_json: vi
      .fn()
      .mockReturnValue('{"nodes":[],"roots":[],"node_count":0}'),
    execute_tool_call: vi.fn().mockReturnValue('{"ok":true}'),
    can_undo: vi.fn().mockReturnValue(false),
    can_redo: vi.fn().mockReturnValue(false),
    get_pages_json: vi.fn().mockReturnValue(
      JSON.stringify([
        {
          id: "page-1",
          name: "Page 1",
          width: 1920,
          height: 1080,
          node_count: 0,
        },
      ]),
    ),
    set_active_page: vi.fn().mockReturnValue(true),
    add_page: vi.fn().mockReturnValue("page-new"),
    remove_page: vi.fn().mockReturnValue(true),
    get_scene_tree_json: vi.fn().mockReturnValue("[]"),
    import_document: vi.fn().mockReturnValue(true),
    export_document_json: vi
      .fn()
      .mockReturnValue('{"format_version":2,"pages":[]}'),
    get_selected_bounds_json: vi.fn().mockReturnValue("[]"),
    get_camera_json: vi.fn().mockReturnValue(JSON.stringify(DEFAULT_CAMERA)),
    clear_selection: vi.fn(),
    register_image_asset: vi.fn().mockReturnValue(true),
    begin_group: vi.fn(),
    end_group: vi.fn(),
    cancel_group: vi.fn(),
  };

  return { ...defaults, ...overrides } as SeleanEditor;
}

/**
 * Creates a React ref holding a mock editor. Use this for components that
 * accept `editorRef: React.RefObject<SeleanEditor | null>`.
 */
export function createMockEditorRef(
  overrides: Partial<Record<keyof SeleanEditor, unknown>> = {},
) {
  const editor = createMockEditor(overrides);
  return { current: editor };
}
