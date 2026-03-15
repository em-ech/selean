import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ColorPicker, type ColorPickerValue } from "./ColorPicker";

const solidBlack: ColorPickerValue = {
  mode: "solid",
  color: { r: 0, g: 0, b: 0, a: 1 },
};

const solidRed: ColorPickerValue = {
  mode: "solid",
  color: { r: 1, g: 0, b: 0, a: 1 },
};

const noFill: ColorPickerValue = {
  mode: "none",
  color: { r: 0, g: 0, b: 0, a: 0 },
};

describe("ColorPicker", () => {
  it("renders mode tabs", () => {
    render(
      <ColorPicker value={solidBlack} onChange={vi.fn()} onClose={vi.fn()} />,
    );
    expect(screen.getByText("Solid")).toBeInTheDocument();
    expect(screen.getByText("Linear")).toBeInTheDocument();
    expect(screen.getByText("Radial")).toBeInTheDocument();
    expect(screen.getByText("None")).toBeInTheDocument();
  });

  it("renders hex input for solid mode", () => {
    render(
      <ColorPicker value={solidRed} onChange={vi.fn()} onClose={vi.fn()} />,
    );
    const hexInput = screen.getByDisplayValue("#ff0000");
    expect(hexInput).toBeInTheDocument();
  });

  it("renders preset swatches", () => {
    render(
      <ColorPicker value={solidBlack} onChange={vi.fn()} onClose={vi.fn()} />,
    );
    // 24 preset colors
    const buttons = screen.getAllByTitle(/#[0-9a-f]{6}/i);
    expect(buttons.length).toBe(24);
  });

  it("calls onChange when preset swatch clicked", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidBlack} onChange={onChange} onClose={vi.fn()} />,
    );
    // Click the red preset (#e74c3c)
    fireEvent.click(screen.getByTitle("#e74c3c"));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "solid",
        color: expect.objectContaining({
          r: expect.closeTo(0.906, 1),
          g: expect.closeTo(0.298, 1),
          b: expect.closeTo(0.235, 1),
        }),
      }),
    );
  });

  it("commits hex input on blur", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidBlack} onChange={onChange} onClose={vi.fn()} />,
    );
    const hexInput = screen.getByDisplayValue("#000000");
    fireEvent.change(hexInput, { target: { value: "#ff0000" } });
    fireEvent.blur(hexInput);
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        color: expect.objectContaining({ r: 1, g: 0, b: 0 }),
      }),
    );
  });

  it("commits hex input on Enter", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidBlack} onChange={onChange} onClose={vi.fn()} />,
    );
    const hexInput = screen.getByDisplayValue("#000000");
    fireEvent.change(hexInput, { target: { value: "#00ff00" } });
    fireEvent.keyDown(hexInput, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        color: expect.objectContaining({ r: 0, g: 1, b: 0 }),
      }),
    );
  });

  it("reverts invalid hex on blur", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidBlack} onChange={onChange} onClose={vi.fn()} />,
    );
    const hexInput = screen.getByDisplayValue("#000000");
    fireEvent.change(hexInput, { target: { value: "invalid" } });
    fireEvent.blur(hexInput);
    // Should not call onChange for invalid input
    expect(onChange).not.toHaveBeenCalled();
    // Hex input should revert
    expect(screen.getByDisplayValue("#000000")).toBeInTheDocument();
  });

  it("switches to None mode", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidBlack} onChange={onChange} onClose={vi.fn()} />,
    );
    fireEvent.click(screen.getByText("None"));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ mode: "none" }),
    );
  });

  it("hides spectrum when mode is None", () => {
    render(<ColorPicker value={noFill} onChange={vi.fn()} onClose={vi.fn()} />);
    // No hex input visible in none mode
    expect(screen.queryByText("HEX")).not.toBeInTheDocument();
  });

  it("switches to Linear gradient mode", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidRed} onChange={onChange} onClose={vi.fn()} />,
    );
    fireEvent.click(screen.getByText("Linear"));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "linear",
        gradient: expect.objectContaining({
          type: "linear",
          stops: expect.arrayContaining([
            expect.objectContaining({ position: 0 }),
            expect.objectContaining({ position: 1 }),
          ]),
        }),
      }),
    );
  });

  it("switches to Radial gradient mode", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidRed} onChange={onChange} onClose={vi.fn()} />,
    );
    fireEvent.click(screen.getByText("Radial"));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "radial",
        gradient: expect.objectContaining({
          type: "radial",
        }),
      }),
    );
  });

  it("renders opacity slider", () => {
    render(
      <ColorPicker value={solidBlack} onChange={vi.fn()} onClose={vi.fn()} />,
    );
    expect(screen.getByText("Opacity")).toBeInTheDocument();
    expect(screen.getByText("100%")).toBeInTheDocument();
  });

  it("updates opacity via slider", () => {
    const onChange = vi.fn();
    render(
      <ColorPicker value={solidBlack} onChange={onChange} onClose={vi.fn()} />,
    );
    const slider = screen.getByRole("slider");
    fireEvent.change(slider, { target: { value: "50" } });
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        color: expect.objectContaining({ a: 0.5 }),
      }),
    );
  });
});
