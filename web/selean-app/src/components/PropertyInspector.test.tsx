import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PropertyInspector, extractRotationDegrees } from "./PropertyInspector";
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

    const checkboxes = screen.getAllByRole("checkbox");
    // The "Visible" checkbox is the checked one (visible: true)
    const visibleCheckbox = checkboxes.find(
      (cb) => (cb as HTMLInputElement).checked,
    )!;
    fireEvent.click(visibleCheckbox);

    const call = (ref.current.execute_command as ReturnType<typeof vi.fn>).mock
      .calls[0][0];
    const parsed = JSON.parse(call);
    expect(parsed.type).toBe("SetVisible");
    expect(parsed.visible).toBe(false);
    expect(onChanged).toHaveBeenCalled();
  });

  it("renders Image section with asset_ref for Image nodes", () => {
    const node = makeNodeInfo({
      kind: "Image",
      asset_ref: "img_12345",
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    // "Image" appears as both kind label and section header
    const imageTexts = screen.getAllByText("Image");
    expect(imageTexts.length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("img_12345")).toBeInTheDocument();
    expect(screen.getByText("Replace Image")).toBeInTheDocument();
  });

  it("renders Vector section with path_data for Vector nodes", () => {
    const node = makeNodeInfo({
      kind: "Vector",
      path_data: "M 0 0 L 100 100 L 200 0 Z",
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    // "Vector" appears as both kind label and section header
    const vectorTexts = screen.getAllByText("Vector");
    expect(vectorTexts.length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("M 0 0 L 100 100 L 200 0 Z")).toBeInTheDocument();
  });

  it("truncates long path_data in Vector section", () => {
    const longPath = "M 0 0 L 100 100 L 200 0 L 300 100 L 400 0 Z";
    const node = makeNodeInfo({
      kind: "Vector",
      path_data: longPath,
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByText(`${longPath.slice(0, 30)}...`)).toBeInTheDocument();
  });
});

describe("PropertyInspector edge cases", () => {
  it("renders Group node without Frame/Text/Image/Vector sections", () => {
    const node = makeNodeInfo({ kind: "Group" });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByText("Identity")).toBeInTheDocument();
    expect(screen.queryByText("Radius")).not.toBeInTheDocument();
    expect(screen.queryByText("Content")).not.toBeInTheDocument();
    expect(screen.queryByText("Replace Image")).not.toBeInTheDocument();
    // Group kind visible
    const groupTexts = screen.getAllByText("Group");
    expect(groupTexts.length).toBeGreaterThanOrEqual(1);
  });

  it("renders Effects section with empty effects array", () => {
    const node = makeNodeInfo({ effects: [] });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByText("Effects")).toBeInTheDocument();
    expect(screen.getByText("Add Effect")).toBeInTheDocument();
  });

  it("renders existing Drop Shadow effect fields", () => {
    const node = makeNodeInfo({
      effects: [
        {
          type: "DropShadow",
          color: { r: 0, g: 0, b: 0, a: 0.5 },
          offset_x: 4,
          offset_y: 4,
          blur_radius: 8,
        },
      ],
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    // "Drop Shadow" appears as effect label and dropdown option
    const labels = screen.getAllByText("Drop Shadow");
    expect(labels.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByTitle("Remove effect")).toBeInTheDocument();
  });

  it("renders existing Blur effect fields", () => {
    const node = makeNodeInfo({
      effects: [{ type: "Blur", radius: 10 }],
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    // "Blur" appears as effect label
    const blurLabels = screen.getAllByText("Blur");
    expect(blurLabels.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByDisplayValue("10")).toBeInTheDocument();
  });

  it("adds effect when Add Effect is clicked", () => {
    const node = makeNodeInfo({ id: "n1", effects: [] });
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={onChanged}
      />,
    );

    fireEvent.click(screen.getByText("Add Effect"));
    const call = (ref.current.execute_command as ReturnType<typeof vi.fn>).mock
      .calls[0][0];
    const parsed = JSON.parse(call);
    expect(parsed.type).toBe("SetEffects");
    expect(parsed.effects.length).toBe(1);
    expect(parsed.effects[0].type).toBe("DropShadow");
  });

  it("removes effect when remove button is clicked", () => {
    const node = makeNodeInfo({
      id: "n1",
      effects: [{ type: "Blur", radius: 10 }],
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );

    fireEvent.click(screen.getByTitle("Remove effect"));
    const call = (ref.current.execute_command as ReturnType<typeof vi.fn>).mock
      .calls[0][0];
    const parsed = JSON.parse(call);
    expect(parsed.type).toBe("SetEffects");
    expect(parsed.effects.length).toBe(0);
  });

  it("handles Text node with null optional fields using defaults", () => {
    const node = makeNodeInfo({
      kind: "Text",
      text_content: null,
      font_size: null,
      font_family: null,
      font_weight: null,
      font_style: null,
      text_align: null,
      line_height: null,
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    // Should render with defaults, not crash
    expect(screen.getByDisplayValue("")).toBeInTheDocument(); // empty text_content
    expect(screen.getByDisplayValue("16")).toBeInTheDocument(); // default font_size
    expect(screen.getByDisplayValue("Inter")).toBeInTheDocument(); // default font_family
    expect(screen.getByDisplayValue("400")).toBeInTheDocument(); // default font_weight
  });

  it("renders null fill color as 'none'", () => {
    const node = makeNodeInfo({ fill: null });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    // "none" appears for null fill and possibly null stroke
    const noneLabels = screen.getAllByText("none");
    expect(noneLabels.length).toBeGreaterThanOrEqual(1);
  });

  it("executes SetBlendMode when blend dropdown changes", () => {
    const node = makeNodeInfo({ id: "n1", blend_mode: "Normal" });
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={onChanged}
      />,
    );

    const select = screen.getByDisplayValue("Normal");
    fireEvent.change(select, { target: { value: "Multiply" } });

    const call = (ref.current.execute_command as ReturnType<typeof vi.fn>).mock
      .calls[0][0];
    const parsed = JSON.parse(call);
    expect(parsed.type).toBe("SetBlendMode");
    expect(parsed.blend_mode).toBe("Multiply");
    expect(onChanged).toHaveBeenCalled();
  });

  it("does not fire command for NaN number input on blur", () => {
    const node = makeNodeInfo({
      id: "n1",
      x: 10,
      y: 20,
      width: 100,
      height: 50,
    });
    const ref = createMockEditorRef();
    render(
      <PropertyInspector
        node={node}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );

    const xInput = screen.getByDisplayValue("10");
    fireEvent.change(xInput, { target: { value: "abc" } });
    fireEvent.blur(xInput);

    expect(ref.current.execute_command).not.toHaveBeenCalled();
  });
});

describe("extractRotationDegrees", () => {
  it("returns 0 for identity transform", () => {
    expect(extractRotationDegrees([1, 0, 0, 1, 0, 0])).toBe(0);
  });

  it("returns 90 for 90-degree rotation", () => {
    expect(extractRotationDegrees([0, 1, -1, 0, 0, 0])).toBe(90);
  });

  it("returns -90 for -90-degree rotation", () => {
    expect(extractRotationDegrees([0, -1, 1, 0, 0, 0])).toBe(-90);
  });

  it("returns 45 for 45-degree rotation", () => {
    const cos45 = Math.cos(Math.PI / 4);
    const sin45 = Math.sin(Math.PI / 4);
    expect(extractRotationDegrees([cos45, sin45, -sin45, cos45, 0, 0])).toBe(
      45,
    );
  });
});
