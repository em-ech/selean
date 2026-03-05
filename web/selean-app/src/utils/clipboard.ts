import type { NodeInfo } from "../wasm/types";

/** Minimal editor interface needed by pasteNode. */
interface EditorLike {
  execute_tool_call: (name: string, args: string) => string;
}

/** Creates a copy of a node at an offset position. */
export function pasteNode(editor: EditorLike, node: NodeInfo): void {
  const args: Record<string, unknown> = {
    name: `${node.name} copy`,
    kind: node.kind,
    x: node.x + 10,
    y: node.y + 10,
    width: node.width,
    height: node.height,
  };

  if (node.fill) {
    args.fill_r = node.fill[0];
    args.fill_g = node.fill[1];
    args.fill_b = node.fill[2];
    args.fill_a = node.fill[3];
  }

  if (node.text_content) {
    args.text_content = node.text_content;
  }

  if (node.font_size) {
    args.font_size = node.font_size;
  }

  if (node.asset_ref) {
    args.asset_ref = node.asset_ref;
  }

  if (node.path_data) {
    args.path_data = node.path_data;
  }

  editor.execute_tool_call("create_node", JSON.stringify(args));
}
