import { render, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { SelectionOverlay } from "./SelectionOverlay";
import { createMockEditorRef, DEFAULT_CAMERA } from "../test/mock-editor";
import type { SelectionBounds } from "../wasm/types";

const EXPECTED_CURSORS = [
  "nwse-resize", // 0: top-left
  "ns-resize", // 1: top-center
  "nesw-resize", // 2: top-right
  "ew-resize", // 3: middle-right
  "nwse-resize", // 4: bottom-right
  "ns-resize", // 5: bottom-center
  "nesw-resize", // 6: bottom-left
  "ew-resize", // 7: middle-left
];

describe("SelectionOverlay", () => {
  it("renders nothing when no selection bounds", async () => {
    const ref = createMockEditorRef({
      get_selected_bounds: vi.fn().mockReturnValue([]),
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    const { container } = render(<SelectionOverlay editorRef={ref} />);

    // Wait for at least one RAF poll cycle
    await waitFor(() => {
      expect(ref.current.get_selected_bounds).toHaveBeenCalled();
    });

    // Component returns null when bounds is empty
    expect(container.innerHTML).toBe("");
  });

  it("renders selection box when bounds exist", async () => {
    const bounds: SelectionBounds[] = [
      { node_id: "node-1", x: 100, y: 200, width: 300, height: 150 },
    ];
    const ref = createMockEditorRef({
      get_selected_bounds: vi.fn().mockReturnValue(bounds),
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    const { container } = render(<SelectionOverlay editorRef={ref} />);

    await waitFor(() => {
      const overlay = container.firstChild;
      expect(overlay).not.toBeNull();
    });

    const overlay = container.firstChild as HTMLElement;
    expect(overlay.style.pointerEvents).toBe("none");
    // Selection box should have 8 resize handle children
    const selectionBox = overlay.firstChild as HTMLElement;
    expect(selectionBox).toBeTruthy();
    // 8 resize handles + 2 rotation elements (stem + handle) for single selection
    expect(selectionBox.children.length).toBe(10);
  });

  it("renders multiple selection boxes with union box for multi-select", async () => {
    const bounds: SelectionBounds[] = [
      { node_id: "node-1", x: 0, y: 0, width: 100, height: 100 },
      { node_id: "node-2", x: 200, y: 200, width: 50, height: 50 },
    ];
    const ref = createMockEditorRef({
      get_selected_bounds: vi.fn().mockReturnValue(bounds),
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    const { container } = render(<SelectionOverlay editorRef={ref} />);

    await waitFor(() => {
      const overlay = container.firstChild;
      expect(overlay).not.toBeNull();
    });

    const overlay = container.firstChild as HTMLElement;
    // 2 selection boxes + 1 union box
    expect(overlay.children.length).toBe(3);
  });

  it("polls editor on animation frame", async () => {
    const getBoundsFn = vi.fn().mockReturnValue([]);
    const getCameraFn = vi.fn().mockReturnValue(DEFAULT_CAMERA);
    const ref = createMockEditorRef({
      get_selected_bounds: getBoundsFn,
      get_camera: getCameraFn,
    });
    render(<SelectionOverlay editorRef={ref} />);

    await waitFor(() => {
      expect(getBoundsFn).toHaveBeenCalled();
    });

    expect(getCameraFn).toHaveBeenCalled();
  });

  it("renders directional cursors on resize handles", async () => {
    const bounds: SelectionBounds[] = [
      { node_id: "node-1", x: 100, y: 100, width: 200, height: 200 },
    ];
    const ref = createMockEditorRef({
      get_selected_bounds: vi.fn().mockReturnValue(bounds),
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    const { container } = render(<SelectionOverlay editorRef={ref} />);

    await waitFor(() => {
      const overlay = container.firstChild;
      expect(overlay).not.toBeNull();
    });

    const selectionBox = container.firstChild!.firstChild as HTMLElement;
    // First 8 children are resize handles (rest are rotation elements)
    const handles = Array.from(selectionBox.children).slice(
      0,
      8,
    ) as HTMLElement[];
    expect(handles.length).toBe(8);

    for (let i = 0; i < 8; i++) {
      expect(handles[i].style.cursor).toBe(EXPECTED_CURSORS[i]);
    }
  });

  it("handles have data-handle-index attributes", async () => {
    const bounds: SelectionBounds[] = [
      { node_id: "node-1", x: 0, y: 0, width: 100, height: 100 },
    ];
    const ref = createMockEditorRef({
      get_selected_bounds: vi.fn().mockReturnValue(bounds),
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    const { container } = render(<SelectionOverlay editorRef={ref} />);

    await waitFor(() => {
      const overlay = container.firstChild;
      expect(overlay).not.toBeNull();
    });

    const selectionBox = container.firstChild!.firstChild as HTMLElement;
    const handles = Array.from(selectionBox.children).slice(
      0,
      8,
    ) as HTMLElement[];

    for (let i = 0; i < 8; i++) {
      expect(handles[i].getAttribute("data-handle-index")).toBe(String(i));
    }
  });
});
