import { describe, it, expect } from "vitest";
import { createMockEditor, makeNodeInfo } from "./mock-editor";

describe("test infrastructure", () => {
  it("creates a mock editor with all methods", () => {
    const editor = createMockEditor();
    expect(editor.get_selected_ids()).toEqual([]);
    expect(editor.can_undo()).toBe(false);
    expect(editor.can_redo()).toBe(false);
  });

  it("creates a node info with defaults", () => {
    const node = makeNodeInfo();
    expect(node.id).toBe("node-1");
    expect(node.kind).toBe("Frame");
    expect(node.width).toBe(300);
  });

  it("creates a node info with overrides", () => {
    const node = makeNodeInfo({
      id: "custom",
      kind: "Text",
      text_content: "Hello",
    });
    expect(node.id).toBe("custom");
    expect(node.kind).toBe("Text");
    expect(node.text_content).toBe("Hello");
  });
});
