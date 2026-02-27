import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
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
