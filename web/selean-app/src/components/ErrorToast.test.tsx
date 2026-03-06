import { render, screen, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ErrorToast, showError } from "./ErrorToast";

describe("ErrorToast", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("renders nothing when no errors", () => {
    const { container } = render(<ErrorToast />);
    expect(container.textContent).toBe("");
  });

  it("shows error message when showError is called", () => {
    render(<ErrorToast />);

    act(() => {
      showError("Something went wrong");
    });

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
  });

  it("shows multiple errors", () => {
    render(<ErrorToast />);

    act(() => {
      showError("Error 1");
      showError("Error 2");
    });

    expect(screen.getByText("Error 1")).toBeInTheDocument();
    expect(screen.getByText("Error 2")).toBeInTheDocument();
  });

  it("dismisses on click", () => {
    render(<ErrorToast />);

    act(() => {
      showError("Click to dismiss");
    });

    expect(screen.getByText("Click to dismiss")).toBeInTheDocument();

    act(() => {
      screen.getByText("Click to dismiss").click();
    });

    expect(screen.queryByText("Click to dismiss")).not.toBeInTheDocument();
  });

  it("logs to console via console.warn", () => {
    const spy = vi.spyOn(console, "warn").mockImplementation(() => {});
    render(<ErrorToast />);

    act(() => {
      showError("logged error");
    });

    expect(spy).toHaveBeenCalledWith("logged error");
  });

  it("keeps only the last 5 toasts", () => {
    render(<ErrorToast />);

    act(() => {
      for (let i = 1; i <= 7; i++) {
        showError(`Error ${i}`);
      }
    });

    // Should keep last 5 (3-7) since we slice(-4) then add 1 = 5 max per batch
    expect(screen.queryByText("Error 1")).not.toBeInTheDocument();
    expect(screen.queryByText("Error 2")).not.toBeInTheDocument();
    expect(screen.getByText("Error 7")).toBeInTheDocument();
  });
});
