import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { LeftSidebar } from "./LeftSidebar";
import { createMockEditorRef } from "../test/mock-editor";

const editorRef = createMockEditorRef();
const onSceneChanged = vi.fn();
const onToggle = vi.fn();

beforeEach(() => {
  onSceneChanged.mockClear();
  onToggle.mockClear();
  (editorRef.current.execute_tool_call as ReturnType<typeof vi.fn>).mockClear();
});

describe("LeftSidebar", () => {
  it("renders collapsed button when not open", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={false}
        onToggle={onToggle}
      />,
    );
    const btn = screen.getByTitle("Open sidebar");
    expect(btn).toBeInTheDocument();
  });

  it("calls onToggle when collapsed button clicked", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={false}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByTitle("Open sidebar"));
    expect(onToggle).toHaveBeenCalled();
  });

  it("renders three tabs when open", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("Elements")).toBeInTheDocument();
    expect(screen.getByText("Templates")).toBeInTheDocument();
    expect(screen.getByText("Uploads")).toBeInTheDocument();
  });

  it("shows shapes section by default (Elements tab)", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("Shapes")).toBeInTheDocument();
    expect(screen.getByText("Rectangle")).toBeInTheDocument();
    expect(screen.getByText("Square")).toBeInTheDocument();
    expect(screen.getByText("Frame")).toBeInTheDocument();
  });

  it("shows text elements", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("Heading")).toBeInTheDocument();
    expect(screen.getByText("Subheading")).toBeInTheDocument();
    expect(screen.getByText("Body text")).toBeInTheDocument();
  });

  it("shows media section with Image button", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByTitle("Upload image")).toBeInTheDocument();
  });

  it("creates rectangle on click", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Rectangle"));
    expect(editorRef.current.execute_tool_call).toHaveBeenCalledWith(
      "create_node",
      expect.stringContaining('"kind":"Frame"'),
    );
    expect(editorRef.current.execute_tool_call).toHaveBeenCalledWith(
      "create_node",
      expect.stringContaining('"fill_r":0.85'),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("does not report a scene change when the editor rejects the element", () => {
    (
      editorRef.current.execute_tool_call as ReturnType<typeof vi.fn>
    ).mockReturnValueOnce('{"success":false,"error":"unknown node kind"}');
    vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Rectangle"));
    expect(editorRef.current.execute_tool_call).toHaveBeenCalledTimes(1);
    expect(onSceneChanged).not.toHaveBeenCalled();
    vi.restoreAllMocks();
  });

  it("creates text on click", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Heading"));
    expect(editorRef.current.execute_tool_call).toHaveBeenCalledWith(
      "create_node",
      expect.stringContaining('"kind":"Text"'),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("uses camera center for element placement", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Rectangle"));
    // Default camera pan_x=0, pan_y=0, so x = 0 - 200/2 = -100, y = 0 - 150/2 = -75
    const callArgs = (
      editorRef.current.execute_tool_call as ReturnType<typeof vi.fn>
    ).mock.calls[0][1];
    const parsed = JSON.parse(callArgs);
    expect(parsed.x).toBe(-100);
    expect(parsed.y).toBe(-75);
  });

  it("switches to Templates tab", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Templates"));
    expect(screen.getByText("Social Media Post")).toBeInTheDocument();
    expect(screen.getByText("Presentation Slide")).toBeInTheDocument();
    expect(screen.getByText("Business Card")).toBeInTheDocument();
    expect(screen.getByText("Poster")).toBeInTheDocument();
    expect(screen.getByText("Flyer")).toBeInTheDocument();
    expect(screen.getByText("Resume")).toBeInTheDocument();
  });

  it("switches to Uploads tab", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Uploads"));
    expect(screen.getByText("Upload image")).toBeInTheDocument();
  });

  it("calls onToggle when close button clicked", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByTitle("Close sidebar"));
    expect(onToggle).toHaveBeenCalled();
  });

  it("creates square with correct dimensions", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Square"));
    const callArgs = (
      editorRef.current.execute_tool_call as ReturnType<typeof vi.fn>
    ).mock.calls[0][1];
    const parsed = JSON.parse(callArgs);
    expect(parsed.width).toBe(150);
    expect(parsed.height).toBe(150);
  });

  it("creates frame with correct dimensions", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Frame"));
    const callArgs = (
      editorRef.current.execute_tool_call as ReturnType<typeof vi.fn>
    ).mock.calls[0][1];
    const parsed = JSON.parse(callArgs);
    expect(parsed.width).toBe(400);
    expect(parsed.height).toBe(300);
  });

  it("applies template on click (creates page + elements)", () => {
    render(
      <LeftSidebar
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Templates"));
    fireEvent.click(screen.getByText("Social Media Post"));

    // Should create a page with template dimensions
    expect(editorRef.current.add_page).toHaveBeenCalledWith(
      "Social Media Post",
      1080,
      1080,
    );
    // Should switch to the new page
    expect(editorRef.current.set_active_page).toHaveBeenCalledWith("page-new");
    // Should create template elements (background + title + subtitle = 3)
    expect(
      (editorRef.current.execute_tool_call as ReturnType<typeof vi.fn>).mock
        .calls.length,
    ).toBe(3);
    expect(onSceneChanged).toHaveBeenCalled();
  });
});
