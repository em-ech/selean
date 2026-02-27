import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

function ThrowingComponent({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error("Test render error");
  }
  return <div>Content rendered</div>;
}

describe("ErrorBoundary", () => {
  it("renders children when no error", () => {
    render(
      <ErrorBoundary name="Test">
        <div>Working content</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText("Working content")).toBeInTheDocument();
  });

  it("renders fallback when child throws", () => {
    // Suppress console.error from React error boundary logging
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <ErrorBoundary name="Canvas">
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>,
    );

    expect(screen.getByText("Canvas crashed")).toBeInTheDocument();
    expect(screen.getByText("Test render error")).toBeInTheDocument();
    expect(screen.getByText("Retry")).toBeInTheDocument();

    consoleSpy.mockRestore();
  });

  it("recovers on retry click", () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    let shouldThrow = true;
    function ConditionalThrower() {
      if (shouldThrow) throw new Error("boom");
      return <div>Recovered</div>;
    }

    render(
      <ErrorBoundary name="Panel">
        <ConditionalThrower />
      </ErrorBoundary>,
    );

    expect(screen.getByText("Panel crashed")).toBeInTheDocument();

    // Fix the error condition
    shouldThrow = false;
    fireEvent.click(screen.getByText("Retry"));

    expect(screen.getByText("Recovered")).toBeInTheDocument();
    expect(screen.queryByText("Panel crashed")).not.toBeInTheDocument();

    consoleSpy.mockRestore();
  });

  it("displays the boundary name in the fallback", () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <ErrorBoundary name="LayerPanel">
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>,
    );

    expect(screen.getByText("LayerPanel crashed")).toBeInTheDocument();

    consoleSpy.mockRestore();
  });
});
