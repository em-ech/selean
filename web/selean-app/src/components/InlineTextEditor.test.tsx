import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { InlineTextEditor } from "./InlineTextEditor";
import { createMockEditorRef, DEFAULT_CAMERA } from "../test/mock-editor";

const defaultBounds = { x: 100, y: 50, width: 200, height: 40 };

describe("InlineTextEditor", () => {
  it("renders a textarea with initial content", () => {
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Hello"
        bounds={defaultBounds}
        fontSize={16}
        editorRef={ref}
        onCommit={() => {}}
        onCancel={() => {}}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    expect(ta).toBeInTheDocument();
    expect(ta).toHaveValue("Hello");
  });

  it("auto-focuses on mount", async () => {
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Focus me"
        bounds={defaultBounds}
        fontSize={16}
        editorRef={ref}
        onCommit={() => {}}
        onCancel={() => {}}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    await waitFor(() => {
      expect(document.activeElement).toBe(ta);
    });
  });

  it("calls onCancel when Escape is pressed", () => {
    const onCancel = vi.fn();
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Test"
        bounds={defaultBounds}
        fontSize={16}
        editorRef={ref}
        onCommit={() => {}}
        onCancel={onCancel}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    fireEvent.keyDown(ta, { key: "Escape" });
    expect(onCancel).toHaveBeenCalled();
  });

  it("calls onCommit with current value on blur", () => {
    const onCommit = vi.fn();
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Original"
        bounds={defaultBounds}
        fontSize={16}
        editorRef={ref}
        onCommit={onCommit}
        onCancel={() => {}}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    fireEvent.change(ta, { target: { value: "Updated" } });
    fireEvent.blur(ta);
    expect(onCommit).toHaveBeenCalledWith("Updated");
  });

  it("does not call onCommit if Escape was pressed before blur", () => {
    const onCommit = vi.fn();
    const onCancel = vi.fn();
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Test"
        bounds={defaultBounds}
        fontSize={16}
        editorRef={ref}
        onCommit={onCommit}
        onCancel={onCancel}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    fireEvent.keyDown(ta, { key: "Escape" });
    fireEvent.blur(ta);
    // onCancel should be called, but onCommit should not.
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("stops propagation on keydown events", () => {
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Test"
        bounds={defaultBounds}
        fontSize={16}
        editorRef={ref}
        onCommit={() => {}}
        onCancel={() => {}}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    const event = new KeyboardEvent("keydown", {
      key: "a",
      bubbles: true,
    });
    const stopSpy = vi.spyOn(event, "stopPropagation");
    ta.dispatchEvent(event);
    expect(stopSpy).toHaveBeenCalled();
  });

  it("updates value on change", () => {
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Before"
        bounds={defaultBounds}
        fontSize={16}
        editorRef={ref}
        onCommit={() => {}}
        onCancel={() => {}}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    fireEvent.change(ta, { target: { value: "After" } });
    expect(ta).toHaveValue("After");
  });

  it("computes position from camera and bounds", async () => {
    const camera = {
      pan_x: 0,
      pan_y: 0,
      zoom: 2,
      viewport_width: 1920,
      viewport_height: 1080,
    };
    const ref = createMockEditorRef({
      get_camera: vi.fn().mockReturnValue(camera),
    });
    render(
      <InlineTextEditor
        nodeId="n1"
        initialContent="Scaled"
        bounds={{ x: 50, y: 25, width: 100, height: 20 }}
        fontSize={14}
        editorRef={ref}
        onCommit={() => {}}
        onCancel={() => {}}
      />,
    );
    const ta = screen.getByTestId("inline-text-editor");
    // With zoom=2, dpr=1: sx = 50*2 = 100, sy = 25*2 = 50
    await waitFor(() => {
      expect(ta.style.left).toBe("100px");
      expect(ta.style.top).toBe("50px");
    });
  });
});
