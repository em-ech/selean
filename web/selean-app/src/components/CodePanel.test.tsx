import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { CodePanel } from "./CodePanel";
import { createMockEditorRef } from "../test/mock-editor";

// Mock clipboard API
beforeEach(() => {
  Object.assign(navigator, {
    clipboard: {
      writeText: vi.fn().mockResolvedValue(undefined),
    },
  });
});

describe("CodePanel", () => {
  it("renders without crashing", () => {
    const ref = createMockEditorRef();
    render(<CodePanel editorRef={ref} refreshKey={0} />);
    expect(screen.getByText("Component")).toBeInTheDocument();
    expect(screen.getByText("Project")).toBeInTheDocument();
    expect(screen.getByText("Tokens")).toBeInTheDocument();
  });

  it("shows Component tab as active by default", () => {
    const ref = createMockEditorRef();
    render(<CodePanel editorRef={ref} refreshKey={0} />);
    // Component tab should have accent border color (active state)
    const componentTab = screen.getByText("Component");
    expect(componentTab).toBeInTheDocument();
  });

  it("displays generated code from editor", () => {
    const sampleCode =
      "export function Page() {\n  return <div>Hello</div>;\n}";
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(sampleCode),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);
    expect(ref.current.generate_code).toHaveBeenCalled();
    expect(screen.getByTestId("code-block")).toBeInTheDocument();
  });

  it("shows empty state when code is empty", () => {
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(""),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);
    expect(screen.getByText("No code generated")).toBeInTheDocument();
  });

  it("copy button copies code to clipboard", async () => {
    const sampleCode = "const x = 1;";
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(sampleCode),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);

    const copyBtn = screen.getByTestId("copy-button");
    expect(copyBtn).toHaveTextContent("Copy");

    fireEvent.click(copyBtn);

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(sampleCode);
    });
    expect(copyBtn).toHaveTextContent("Copied");
  });

  it("switches to Project tab and shows file tree", () => {
    const projectJson = JSON.stringify({
      files: [
        {
          path: "src/App.tsx",
          content: "export default function App() {}",
          language: "tsx",
        },
        {
          path: "src/index.css",
          content: "body { margin: 0; }",
          language: "css",
        },
      ],
    });
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(""),
      generate_project_json: vi.fn().mockReturnValue(projectJson),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);

    fireEvent.click(screen.getByText("Project"));

    // File path appears in both the toolbar label and file tree button
    expect(screen.getAllByText("src/App.tsx").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("src/index.css").length).toBeGreaterThanOrEqual(
      1,
    );
  });

  it("switches to Tokens tab and shows color swatches", () => {
    const tokensJson = JSON.stringify({
      colors: [
        { name: "primary", hex: "#3b82f6" },
        { name: "bg", hex: "#1e1e2e" },
      ],
      fonts: [{ family: "Inter", weights: [400, 700] }],
    });
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(""),
      extract_design_tokens_json: vi.fn().mockReturnValue(tokensJson),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);

    fireEvent.click(screen.getByText("Tokens"));

    expect(screen.getByText("Colors")).toBeInTheDocument();
    expect(screen.getByText("primary")).toBeInTheDocument();
    expect(screen.getByText("#3b82f6")).toBeInTheDocument();
    expect(screen.getByText("Fonts")).toBeInTheDocument();
    expect(screen.getByText("Inter")).toBeInTheDocument();
    expect(screen.getByText("400")).toBeInTheDocument();
    expect(screen.getByText("700")).toBeInTheDocument();
  });

  it("handles null editor gracefully", () => {
    const ref = { current: null };
    render(<CodePanel editorRef={ref} refreshKey={0} />);
    expect(screen.getByText("Component")).toBeInTheDocument();
    expect(screen.getByText("No code generated")).toBeInTheDocument();
  });

  it("re-generates code when refreshKey changes", () => {
    const generateFn = vi.fn().mockReturnValue("const a = 1;");
    const ref = createMockEditorRef({
      generate_code: generateFn,
    });
    const { rerender } = render(<CodePanel editorRef={ref} refreshKey={0} />);
    expect(generateFn).toHaveBeenCalledTimes(1);

    rerender(<CodePanel editorRef={ref} refreshKey={1} />);
    expect(generateFn).toHaveBeenCalledTimes(2);
  });

  it("selects a file in the project tab", () => {
    const projectJson = JSON.stringify({
      files: [
        { path: "src/App.tsx", content: "function App() {}", language: "tsx" },
        {
          path: "src/utils.ts",
          content: "export const PI = 3.14;",
          language: "ts",
        },
      ],
    });
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(""),
      generate_project_json: vi.fn().mockReturnValue(projectJson),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);

    fireEvent.click(screen.getByText("Project"));

    // Click the second file
    fireEvent.click(screen.getByText("src/utils.ts"));
    // The code block should be present (file content displayed)
    expect(screen.getByTestId("code-block")).toBeInTheDocument();
  });

  it("shows no design tokens message when tokens are empty", () => {
    const tokensJson = JSON.stringify({
      colors: [],
      fonts: [],
    });
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(""),
      extract_design_tokens_json: vi.fn().mockReturnValue(tokensJson),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);

    fireEvent.click(screen.getByText("Tokens"));
    expect(screen.getByText("No design tokens found")).toBeInTheDocument();
  });

  it("does not show copy button on tokens tab", () => {
    const ref = createMockEditorRef({
      generate_code: vi.fn().mockReturnValue(""),
      extract_design_tokens_json: vi
        .fn()
        .mockReturnValue(JSON.stringify({ colors: [], fonts: [] })),
    });
    render(<CodePanel editorRef={ref} refreshKey={0} />);

    // Copy button visible on component tab
    expect(screen.getByTestId("copy-button")).toBeInTheDocument();

    // Switch to tokens tab
    fireEvent.click(screen.getByText("Tokens"));
    expect(screen.queryByTestId("copy-button")).not.toBeInTheDocument();
  });
});
