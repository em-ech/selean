import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PropertyInspector } from "./PropertyInspector";
import { createMockEditorRef, makeNodeInfo } from "../test/mock-editor";

describe("PropertyInspector", () => {
  it("shows empty state when no node is selected", () => {
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={null}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByText("No node selected")).toBeInTheDocument();
    expect(screen.getByText("Properties")).toBeInTheDocument();
  });

  it("renders node identity fields", () => {
    const node = makeNodeInfo({
      id: "abc-123",
      name: "My Frame",
      kind: "Frame",
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByText("Identity")).toBeInTheDocument();
    expect(screen.getByDisplayValue("My Frame")).toBeInTheDocument();
    // "Frame" appears both as kind value and as section label; use getAllByText
    const frameTexts = screen.getAllByText("Frame");
    expect(frameTexts.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("abc-123")).toBeInTheDocument();
  });

  it("renders position and size fields", () => {
    // Use values that don't collide with opacity (100) or other default fields
    const node = makeNodeInfo({ x: 42, y: 87, width: 310, height: 155 });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByDisplayValue("42")).toBeInTheDocument();
    expect(screen.getByDisplayValue("87")).toBeInTheDocument();
    expect(screen.getByDisplayValue("310")).toBeInTheDocument();
    expect(screen.getByDisplayValue("155")).toBeInTheDocument();
  });

  it("renders Frame-specific corner radius field for Frame nodes", () => {
    const node = makeNodeInfo({ kind: "Frame", corner_radius: [8, 8, 8, 8] });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByText("Radius")).toBeInTheDocument();
    expect(screen.getByDisplayValue("8")).toBeInTheDocument();
  });

  it("does not render Frame section for non-Frame nodes", () => {
    const node = makeNodeInfo({ kind: "Text", text_content: "hello" });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.queryByText("Radius")).not.toBeInTheDocument();
  });

  it("renders Text-specific fields for Text nodes", () => {
    const node = makeNodeInfo({
      kind: "Text",
      text_content: "Hello world",
      font_size: 24,
      font_family: "Inter",
      font_weight: 400,
      font_style: "Normal",
      text_align: "Left",
      line_height: 1.2,
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByText("Content")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Hello world")).toBeInTheDocument();
    expect(screen.getByDisplayValue("24")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Inter")).toBeInTheDocument();
  });

  it("does not render Text section for Frame nodes", () => {
    const node = makeNodeInfo({ kind: "Frame" });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.queryByText("Content")).not.toBeInTheDocument();
  });

  it("executes SetName command when name input is committed", () => {
    const node = makeNodeInfo({ id: "n1", name: "Old Name" });
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={onChanged}
      />,
    );

    const input = screen.getByDisplayValue("Old Name");
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.blur(input);

    expect(ref.current.execute_command).toHaveBeenCalledWith(
      JSON.stringify({ type: "SetName", node_id: "n1", name: "New Name" }),
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("executes SetBounds command when X is committed", () => {
    const node = makeNodeInfo({
      id: "n1",
      x: 10,
      y: 20,
      width: 100,
      height: 50,
    });
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={onChanged}
      />,
    );

    const xInput = screen.getByDisplayValue("10");
    fireEvent.change(xInput, { target: { value: "55" } });
    fireEvent.blur(xInput);

    const call = (ref.current.execute_command as ReturnType<typeof vi.fn>).mock
      .calls[0][0];
    const parsed = JSON.parse(call);
    expect(parsed.type).toBe("SetBounds");
    expect(parsed.node_id).toBe("n1");
    expect(parsed.bounds.x).toBe(55);
    expect(parsed.bounds.y).toBe(20);
  });

  it("does not execute command when value is unchanged on blur", () => {
    const node = makeNodeInfo({ name: "Same" });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );

    const input = screen.getByDisplayValue("Same");
    fireEvent.blur(input);

    expect(ref.current.execute_command).not.toHaveBeenCalled();
  });

  it("renders blend mode dropdown with all options", () => {
    const node = makeNodeInfo({ blend_mode: "Normal" });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );

    const select = screen.getByDisplayValue("Normal");
    expect(select).toBeInTheDocument();
    expect(select.tagName).toBe("SELECT");
  });

  it("executes SetVisible when checkbox is toggled", () => {
    const node = makeNodeInfo({ id: "n1", visible: true });
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={onChanged}
      />,
    );

    const checkbox = screen.getByRole("checkbox");
    fireEvent.click(checkbox);

    const call = (ref.current.execute_command as ReturnType<typeof vi.fn>).mock
      .calls[0][0];
    const parsed = JSON.parse(call);
    expect(parsed.type).toBe("SetVisible");
    expect(parsed.visible).toBe(false);
    expect(onChanged).toHaveBeenCalled();
  });
});
