import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { FileMenu } from "./FileMenu";
import { createMockEditorRef } from "../test/mock-editor";

describe("FileMenu", () => {
  it("renders the File button", () => {
    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);
    expect(screen.getByText("File")).toBeInTheDocument();
  });

  it("menu is closed by default", () => {
    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);
    expect(screen.queryByText("New")).not.toBeInTheDocument();
    expect(screen.queryByText("Save")).not.toBeInTheDocument();
  });

  it("opens menu on File button click", () => {
    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);

    fireEvent.click(screen.getByText("File"));
    expect(screen.getByText("New")).toBeInTheDocument();
    expect(screen.getByText("Save")).toBeInTheDocument();
    expect(screen.getByText("Open...")).toBeInTheDocument();
    expect(screen.getByText("Import PPTX...")).toBeInTheDocument();
    expect(screen.getByText("Export PPTX")).toBeInTheDocument();
  });

  it("calls import_document when New is clicked", () => {
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(<FileMenu editorRef={ref} onSceneChanged={onChanged} />);

    fireEvent.click(screen.getByText("File"));
    fireEvent.click(screen.getByText("New"));

    expect(ref.current.import_document).toHaveBeenCalledTimes(1);
    const arg = (ref.current.import_document as ReturnType<typeof vi.fn>).mock
      .calls[0][0];
    const parsed = JSON.parse(arg);
    expect(parsed.format_version).toBe(2);
    expect(parsed.pages).toHaveLength(1);
    expect(parsed.pages[0].name).toBe("Page 1");
    expect(onChanged).toHaveBeenCalled();
  });

  it("calls export_document_json when Save is clicked", () => {
    // Stub URL.createObjectURL and document.createElement('a').click
    const revokeUrl = vi.fn();
    vi.stubGlobal("URL", {
      createObjectURL: vi.fn().mockReturnValue("blob:test"),
      revokeObjectURL: revokeUrl,
    });

    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);

    fireEvent.click(screen.getByText("File"));
    fireEvent.click(screen.getByText("Save"));

    expect(ref.current.export_document_json).toHaveBeenCalled();
    expect(revokeUrl).toHaveBeenCalledWith("blob:test");

    vi.unstubAllGlobals();
  });

  it("closes menu after an action", () => {
    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);

    fireEvent.click(screen.getByText("File"));
    expect(screen.getByText("New")).toBeInTheDocument();

    fireEvent.click(screen.getByText("New"));
    expect(screen.queryByText("New")).not.toBeInTheDocument();
  });

  it("toggles menu open and closed", () => {
    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);

    // Open
    fireEvent.click(screen.getByText("File"));
    expect(screen.getByText("New")).toBeInTheDocument();

    // Close via toggling File button
    fireEvent.click(screen.getByText("File"));
    expect(screen.queryByText("New")).not.toBeInTheDocument();
  });
});
