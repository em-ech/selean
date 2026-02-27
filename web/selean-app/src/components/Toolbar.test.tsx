import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { Toolbar } from "./Toolbar";

describe("Toolbar", () => {
  it("renders all four tool buttons", () => {
    render(<Toolbar activeTool="select" onToolChange={() => {}} />);

    expect(screen.getByTitle("Select (V)")).toBeInTheDocument();
    expect(screen.getByTitle("Frame (F)")).toBeInTheDocument();
    expect(screen.getByTitle("Text (T)")).toBeInTheDocument();
    expect(screen.getByTitle("Image (I)")).toBeInTheDocument();
  });

  it("shows first letter of each tool label", () => {
    render(<Toolbar activeTool="select" onToolChange={() => {}} />);

    expect(screen.getByTitle("Select (V)")).toHaveTextContent("S");
    expect(screen.getByTitle("Frame (F)")).toHaveTextContent("F");
    expect(screen.getByTitle("Text (T)")).toHaveTextContent("T");
    expect(screen.getByTitle("Image (I)")).toHaveTextContent("I");
  });

  it("highlights the active tool with accent background", () => {
    const { rerender } = render(
      <Toolbar activeTool="frame" onToolChange={() => {}} />,
    );

    const frameBtn = screen.getByTitle("Frame (F)");
    const selectBtn = screen.getByTitle("Select (V)");
    expect(frameBtn.style.background).not.toBe("transparent");
    expect(selectBtn.style.background).toBe("transparent");

    rerender(<Toolbar activeTool="text" onToolChange={() => {}} />);
    const textBtn = screen.getByTitle("Text (T)");
    expect(textBtn.style.background).not.toBe("transparent");
    expect(frameBtn.style.background).toBe("transparent");
  });

  it("calls onToolChange with the correct tool type on click", () => {
    const onChange = vi.fn();
    render(<Toolbar activeTool="select" onToolChange={onChange} />);

    fireEvent.click(screen.getByTitle("Frame (F)"));
    expect(onChange).toHaveBeenCalledWith("frame");

    fireEvent.click(screen.getByTitle("Text (T)"));
    expect(onChange).toHaveBeenCalledWith("text");

    fireEvent.click(screen.getByTitle("Image (I)"));
    expect(onChange).toHaveBeenCalledWith("image");

    expect(onChange).toHaveBeenCalledTimes(3);
  });

  it("does not crash when clicking the already-active tool", () => {
    const onChange = vi.fn();
    render(<Toolbar activeTool="select" onToolChange={onChange} />);

    fireEvent.click(screen.getByTitle("Select (V)"));
    expect(onChange).toHaveBeenCalledWith("select");
  });
});
