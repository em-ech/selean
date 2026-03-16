import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { ErrorToast, showError } from "./ErrorToast";

describe("ErrorToast", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("showError renders a toast message", () => {
    render(<ErrorToast />);
    act(() => {
      showError("Something went wrong");
    });
    expect(screen.getByText("Something went wrong")).toBeTruthy();
  });

  it("auto-dismisses after 5 seconds", () => {
    render(<ErrorToast />);
    act(() => {
      showError("Temporary error");
    });
    expect(screen.getByText("Temporary error")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.queryByText("Temporary error")).toBeNull();
  });

  it("click-to-dismiss removes toast", () => {
    render(<ErrorToast />);
    act(() => {
      showError("Click me away");
    });
    const toast = screen.getByText("Click me away");
    fireEvent.click(toast);
    expect(screen.queryByText("Click me away")).toBeNull();
  });

  it("max 5 toasts visible", () => {
    render(<ErrorToast />);
    act(() => {
      for (let i = 1; i <= 7; i++) {
        showError(`Toast ${i}`);
      }
    });
    // slice(-4) keeps last 4 of prev, then pushes new -> max 5
    // After 7 calls the oldest two are dropped
    expect(screen.queryByText("Toast 1")).toBeNull();
    expect(screen.queryByText("Toast 2")).toBeNull();
    expect(screen.getByText("Toast 3")).toBeTruthy();
    expect(screen.getByText("Toast 4")).toBeTruthy();
    expect(screen.getByText("Toast 5")).toBeTruthy();
    expect(screen.getByText("Toast 6")).toBeTruthy();
    expect(screen.getByText("Toast 7")).toBeTruthy();
  });

  it("listener error does not break broadcast", () => {
    // Suppress console output from the try-catch in showError
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});

    render(<ErrorToast />);

    act(() => {
      showError("Still works");
    });

    expect(screen.getByText("Still works")).toBeTruthy();

    consoleError.mockRestore();
    consoleWarn.mockRestore();
  });
});
