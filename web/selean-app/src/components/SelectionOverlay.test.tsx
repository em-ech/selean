import { render, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { SelectionOverlay } from "./SelectionOverlay";
import { createMockEditorRef, DEFAULT_CAMERA } from "../test/mock-editor";
import type { SelectionBounds } from "../wasm/types";

describe("SelectionOverlay", () => {
  it("renders nothing when no selection bounds", async () => {
    const ref = createMockEditorRef({
      get_selected_bounds_json: vi.fn().mockReturnValue("[]"),
      get_camera_json: vi.fn().mockReturnValue(JSON.stringify(DEFAULT_CAMERA)),
    });
    const { container } = render(<SelectionOverlay editorRef={ref} />);

    // Wait for at least one RAF poll cycle
    await waitFor(() => {
      expect(ref.current.get_selected_bounds_json).toHaveBeenCalled();
    });

    // Component returns null when bounds is empty
    expect(container.innerHTML).toBe("");
  });

  it("renders selection box when bounds exist", async () => {
    const bounds: SelectionBounds[] = [
      { x: 100, y: 200, width: 300, height: 150 },
    ];
    const ref = createMockEditorRef({
      get_selected_bounds_json: vi.fn().mockReturnValue(JSON.stringify(bounds)),
      get_camera_json: vi.fn().mockReturnValue(JSON.stringify(DEFAULT_CAMERA)),
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
    expect(selectionBox.children.length).toBe(8);
  });

  it("renders multiple selection boxes for multi-select", async () => {
    const bounds: SelectionBounds[] = [
      { x: 0, y: 0, width: 100, height: 100 },
      { x: 200, y: 200, width: 50, height: 50 },
    ];
    const ref = createMockEditorRef({
      get_selected_bounds_json: vi.fn().mockReturnValue(JSON.stringify(bounds)),
      get_camera_json: vi.fn().mockReturnValue(JSON.stringify(DEFAULT_CAMERA)),
    });
    const { container } = render(<SelectionOverlay editorRef={ref} />);

    await waitFor(() => {
      const overlay = container.firstChild;
      expect(overlay).not.toBeNull();
    });

    const overlay = container.firstChild as HTMLElement;
    expect(overlay.children.length).toBe(2);
  });

  it("polls editor on animation frame", async () => {
    const getBoundsFn = vi.fn().mockReturnValue("[]");
    const getCameraFn = vi.fn().mockReturnValue(JSON.stringify(DEFAULT_CAMERA));
    const ref = createMockEditorRef({
      get_selected_bounds_json: getBoundsFn,
      get_camera_json: getCameraFn,
    });
    render(<SelectionOverlay editorRef={ref} />);

    await waitFor(() => {
      expect(getBoundsFn).toHaveBeenCalled();
    });

    expect(getCameraFn).toHaveBeenCalled();
  });
});
