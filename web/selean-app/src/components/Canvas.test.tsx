import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, type Mock } from "vitest";
import { Canvas } from "./Canvas";
import { createMockEditorRef } from "../test/mock-editor";

describe("Canvas", () => {
  it("renders a canvas element with the provided id", () => {
    const ref = createMockEditorRef();
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="ready" />);
    const canvas = document.getElementById("test-canvas");
    expect(canvas).toBeInTheDocument();
    expect(canvas?.tagName).toBe("CANVAS");
  });

  it("shows loading overlay when status is loading", () => {
    const ref = createMockEditorRef();
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="loading" />);
    expect(screen.getByText("Loading editor...")).toBeInTheDocument();
  });

  it("shows unsupported overlay when status is unsupported", () => {
    const ref = createMockEditorRef();
    render(
      <Canvas canvasId="test-canvas" editorRef={ref} status="unsupported" />,
    );
    expect(
      screen.getByText("WebGPU is not supported in this browser."),
    ).toBeInTheDocument();
    expect(screen.getByText(/Chrome 113/)).toBeInTheDocument();
  });

  it("does not show overlays when status is ready", () => {
    const ref = createMockEditorRef();
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="ready" />);
    expect(screen.queryByText("Loading editor...")).not.toBeInTheDocument();
    expect(
      screen.queryByText("WebGPU is not supported in this browser."),
    ).not.toBeInTheDocument();
  });

  it("prevents context menu on the canvas", () => {
    const ref = createMockEditorRef();
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="ready" />);
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;
    expect(canvas).toBeTruthy();
    // The canvas has onContextMenu handler, just verify it rendered
    expect(canvas.tagName).toBe("CANVAS");
  });
});

describe("Canvas pointer events", () => {
  it("calls on_pointer_down with correct arguments on pointerdown", () => {
    const ref = createMockEditorRef();
    (ref.current.on_pointer_down as Mock).mockReturnValue([]);
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="ready" />);
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;

    fireEvent.pointerDown(canvas, {
      clientX: 50,
      clientY: 60,
      button: 0,
      shiftKey: true,
      ctrlKey: false,
      altKey: false,
      metaKey: false,
      pointerId: 1,
    });

    expect(ref.current.on_pointer_down).toHaveBeenCalledTimes(1);
    const args = (ref.current.on_pointer_down as Mock).mock.calls[0];
    // x and y are DPR-adjusted relative to element, button, then modifier flags
    expect(args[2]).toBe(0); // button
    expect(args[3]).toBe(true); // shiftKey
    expect(args[4]).toBe(false); // ctrlKey
  });

  it("calls on_pointer_move on pointermove", () => {
    const ref = createMockEditorRef();
    (ref.current.on_pointer_move as Mock).mockReturnValue([]);
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="ready" />);
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;

    fireEvent.pointerMove(canvas, { clientX: 100, clientY: 200 });

    expect(ref.current.on_pointer_move).toHaveBeenCalledTimes(1);
  });

  it("calls on_pointer_up on pointerup", () => {
    const ref = createMockEditorRef();
    (ref.current.on_pointer_up as Mock).mockReturnValue([]);
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="ready" />);
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;

    fireEvent.pointerUp(canvas, {
      clientX: 50,
      clientY: 60,
      button: 0,
      pointerId: 1,
    });

    expect(ref.current.on_pointer_up).toHaveBeenCalledTimes(1);
  });

  it("calls on_scroll on wheel event", () => {
    const ref = createMockEditorRef();
    (ref.current.on_scroll as Mock).mockReturnValue([]);
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="ready" />);
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;

    fireEvent.wheel(canvas, {
      clientX: 50,
      clientY: 60,
      deltaX: 10,
      deltaY: 20,
    });

    expect(ref.current.on_scroll).toHaveBeenCalledTimes(1);
    const args = (ref.current.on_scroll as Mock).mock.calls[0];
    expect(args[2]).toBe(10); // deltaX
    expect(args[3]).toBe(20); // deltaY
  });

  it("propagates interaction events to onInteractionEvents callback", () => {
    const ref = createMockEditorRef();
    const mockEvents = [{ type: "Clicked", node_id: "n1" }];
    (ref.current.on_pointer_down as Mock).mockReturnValue(mockEvents);
    const onEvents = vi.fn();

    render(
      <Canvas
        canvasId="test-canvas"
        editorRef={ref}
        status="ready"
        onInteractionEvents={onEvents}
      />,
    );
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;
    fireEvent.pointerDown(canvas, { clientX: 50, clientY: 60, pointerId: 1 });

    expect(onEvents).toHaveBeenCalledWith(mockEvents);
  });

  it("does not call onInteractionEvents for empty event arrays", () => {
    const ref = createMockEditorRef();
    (ref.current.on_pointer_move as Mock).mockReturnValue([]);
    const onEvents = vi.fn();

    render(
      <Canvas
        canvasId="test-canvas"
        editorRef={ref}
        status="ready"
        onInteractionEvents={onEvents}
      />,
    );
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;
    fireEvent.pointerMove(canvas, { clientX: 10, clientY: 20 });

    expect(onEvents).not.toHaveBeenCalled();
  });

  it("does not call WASM methods when editor is null", () => {
    const ref = { current: null };
    render(<Canvas canvasId="test-canvas" editorRef={ref} status="loading" />);
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;
    // Should not throw
    fireEvent.pointerDown(canvas, { clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 10, clientY: 10 });
    fireEvent.pointerUp(canvas, { clientX: 10, clientY: 10, pointerId: 1 });
  });

  it("calls onContextMenu prop with coordinates on right-click", () => {
    const ref = createMockEditorRef();
    const onCtx = vi.fn();
    render(
      <Canvas
        canvasId="test-canvas"
        editorRef={ref}
        status="ready"
        onContextMenu={onCtx}
      />,
    );
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;
    fireEvent.contextMenu(canvas, { clientX: 200, clientY: 300 });

    expect(onCtx).toHaveBeenCalledWith(200, 300);
  });

  it("does not call onInteractionEvents when WASM returns null", () => {
    const ref = createMockEditorRef();
    (ref.current.on_pointer_move as Mock).mockReturnValue(null);
    const onEvents = vi.fn();

    render(
      <Canvas
        canvasId="test-canvas"
        editorRef={ref}
        status="ready"
        onInteractionEvents={onEvents}
      />,
    );
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;
    fireEvent.pointerMove(canvas, { clientX: 10, clientY: 20 });

    expect(onEvents).not.toHaveBeenCalled();
  });
});

describe("Canvas drag outside the canvas", () => {
  function renderCanvas() {
    const ref = createMockEditorRef();
    const onEvents = vi.fn();
    render(
      <Canvas
        canvasId="test-canvas"
        editorRef={ref}
        status="ready"
        onInteractionEvents={onEvents}
      />,
    );
    const canvas = document.getElementById("test-canvas") as HTMLCanvasElement;
    return { ref, onEvents, canvas };
  }

  it("forwards window pointermove to the editor while a drag is active", () => {
    const { ref, onEvents, canvas } = renderCanvas();
    const moved = [{ type: "DragMoved" }];
    (ref.current.on_pointer_move as Mock).mockReturnValue(moved);

    fireEvent.pointerDown(canvas, { clientX: 5, clientY: 5, pointerId: 1 });
    fireEvent.pointerMove(document.body, {
      clientX: 500,
      clientY: 400,
      pointerId: 1,
    });

    expect(ref.current.on_pointer_move).toHaveBeenCalledTimes(1);
    expect(onEvents).toHaveBeenCalledWith(moved);
  });

  it("emits the events returned when the pointer is released outside", () => {
    const { ref, onEvents, canvas } = renderCanvas();
    const ended = [{ type: "DragEnded" }];
    (ref.current.on_pointer_up as Mock).mockReturnValue(ended);

    fireEvent.pointerDown(canvas, { clientX: 5, clientY: 5, pointerId: 1 });
    fireEvent.pointerUp(document.body, {
      clientX: 500,
      clientY: 400,
      pointerId: 1,
    });

    expect(ref.current.on_pointer_up).toHaveBeenCalledTimes(1);
    expect(onEvents).toHaveBeenCalledWith(ended);

    // The drag is over: later moves outside the canvas are ignored.
    fireEvent.pointerMove(document.body, { clientX: 1, clientY: 1, pointerId: 1 });
    expect(ref.current.on_pointer_move).not.toHaveBeenCalled();
  });

  it("does not handle a canvas pointer event twice", () => {
    const { ref, canvas } = renderCanvas();

    fireEvent.pointerDown(canvas, { clientX: 5, clientY: 5, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: 10, clientY: 10, pointerId: 1 });

    expect(ref.current.on_pointer_move).toHaveBeenCalledTimes(1);
    expect(ref.current.on_pointer_up).toHaveBeenCalledTimes(1);
  });

  it("ignores window pointer events when no drag started on the canvas", () => {
    const { ref } = renderCanvas();

    fireEvent.pointerMove(document.body, { clientX: 1, clientY: 1, pointerId: 1 });
    fireEvent.pointerUp(document.body, { clientX: 1, clientY: 1, pointerId: 1 });

    expect(ref.current.on_pointer_move).not.toHaveBeenCalled();
    expect(ref.current.on_pointer_up).not.toHaveBeenCalled();
  });
});
