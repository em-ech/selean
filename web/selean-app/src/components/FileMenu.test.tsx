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
    expect(screen.getByText("Import IDML...")).toBeInTheDocument();
    expect(screen.getByText("Export IDML")).toBeInTheDocument();
    expect(screen.getByText("Import InDesign...")).toBeInTheDocument();
    expect(screen.getByText("Import Figma...")).toBeInTheDocument();
    expect(screen.getByText("Export to Figma")).toBeInTheDocument();
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

  it("calls Export IDML via fetch", async () => {
    const revokeUrl = vi.fn();
    vi.stubGlobal("URL", {
      createObjectURL: vi.fn().mockReturnValue("blob:idml"),
      revokeObjectURL: revokeUrl,
    });
    const mockBlob = new Blob(["idml-data"]);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        blob: () => Promise.resolve(mockBlob),
      }),
    );

    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);

    fireEvent.click(screen.getByText("File"));
    fireEvent.click(screen.getByText("Export IDML"));

    // Wait for async fetch
    await vi.waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/export/idml",
        expect.objectContaining({ method: "POST" }),
      );
    });

    vi.unstubAllGlobals();
  });

  it("calls Import Figma via fetch", async () => {
    vi.stubGlobal(
      "prompt",
      vi.fn().mockReturnValue("https://www.figma.com/file/abc123/MyFile"),
    );
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"format_version":2,"pages":[]}'),
      }),
    );

    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(<FileMenu editorRef={ref} onSceneChanged={onChanged} />);

    fireEvent.click(screen.getByText("File"));
    fireEvent.click(screen.getByText("Import Figma..."));

    await vi.waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/import/figma",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ file_key: "abc123" }),
        }),
      );
    });

    vi.unstubAllGlobals();
  });

  it("calls Export to Figma via fetch", async () => {
    const revokeUrl = vi.fn();
    vi.stubGlobal("URL", {
      createObjectURL: vi.fn().mockReturnValue("blob:figma"),
      revokeObjectURL: revokeUrl,
    });
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"name":"doc","pages":[]}'),
      }),
    );

    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);

    fireEvent.click(screen.getByText("File"));
    fireEvent.click(screen.getByText("Export to Figma"));

    await vi.waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/export/figma",
        expect.objectContaining({ method: "POST" }),
      );
    });

    vi.unstubAllGlobals();
  });

  it("shows alert on Figma 403 error", async () => {
    vi.stubGlobal("prompt", vi.fn().mockReturnValue("abc123"));
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 403,
      }),
    );
    const alertFn = vi.fn();
    vi.stubGlobal("alert", alertFn);

    const ref = createMockEditorRef();
    render(<FileMenu editorRef={ref} onSceneChanged={() => {}} />);

    fireEvent.click(screen.getByText("File"));
    fireEvent.click(screen.getByText("Import Figma..."));

    await vi.waitFor(() => {
      expect(alertFn).toHaveBeenCalledWith(
        expect.stringContaining("access denied"),
      );
    });

    vi.unstubAllGlobals();
  });
});
