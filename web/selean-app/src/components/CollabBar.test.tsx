import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CollabBar } from "./CollabBar";
import type { Participant } from "../collab/types";

describe("CollabBar", () => {
  it("shows Offline when disconnected", () => {
    render(
      <CollabBar
        status="disconnected"
        participants={[]}
        hasPendingOps={false}
      />,
    );
    expect(screen.getByTestId("collab-status-text").textContent).toBe(
      "Offline",
    );
  });

  it("shows Connecting when connecting", () => {
    render(
      <CollabBar status="connecting" participants={[]} hasPendingOps={false} />,
    );
    expect(screen.getByTestId("collab-status-text").textContent).toBe(
      "Connecting...",
    );
  });

  it("shows Connected when connected with no pending ops", () => {
    render(
      <CollabBar status="connected" participants={[]} hasPendingOps={false} />,
    );
    expect(screen.getByTestId("collab-status-text").textContent).toBe(
      "Connected",
    );
  });

  it("shows Saving when connected with pending ops", () => {
    render(
      <CollabBar status="connected" participants={[]} hasPendingOps={true} />,
    );
    expect(screen.getByTestId("collab-status-text").textContent).toBe(
      "Saving...",
    );
  });

  it("shows Join button when disconnected and onConnect provided", () => {
    const onConnect = vi.fn();
    render(
      <CollabBar
        status="disconnected"
        participants={[]}
        hasPendingOps={false}
        onConnect={onConnect}
      />,
    );
    const btn = screen.getByTestId("collab-connect-btn");
    expect(btn.textContent).toBe("Join");
    fireEvent.click(btn);
    expect(onConnect).toHaveBeenCalledOnce();
  });

  it("shows Leave button when connected and onDisconnect provided", () => {
    const onDisconnect = vi.fn();
    render(
      <CollabBar
        status="connected"
        participants={[]}
        hasPendingOps={false}
        onDisconnect={onDisconnect}
      />,
    );
    const btn = screen.getByTestId("collab-disconnect-btn");
    expect(btn.textContent).toBe("Leave");
    fireEvent.click(btn);
    expect(onDisconnect).toHaveBeenCalledOnce();
  });

  it("renders participant avatars", () => {
    const participants: Participant[] = [
      { session_id: "s1", user_id: "u1", display_name: "Alice" },
      { session_id: "s2", user_id: "u2", display_name: "Bob" },
    ];
    render(
      <CollabBar
        status="connected"
        participants={participants}
        hasPendingOps={false}
      />,
    );
    expect(screen.getByTestId("participant-list")).toBeTruthy();
    expect(screen.getByTestId("participant-s1").textContent).toBe("A");
    expect(screen.getByTestId("participant-s2").textContent).toBe("B");
  });

  it("hides participant list when empty", () => {
    const { container } = render(
      <CollabBar status="connected" participants={[]} hasPendingOps={false} />,
    );
    expect(
      container.querySelector("[data-testid='participant-list']"),
    ).toBeNull();
  });

  it("status dot is green when connected", () => {
    render(
      <CollabBar status="connected" participants={[]} hasPendingOps={false} />,
    );
    const dot = screen.getByTestId("collab-status-dot");
    expect(dot.style.backgroundColor).toBe("rgb(39, 174, 96)");
  });

  it("status dot is grey when disconnected", () => {
    render(
      <CollabBar
        status="disconnected"
        participants={[]}
        hasPendingOps={false}
      />,
    );
    const dot = screen.getByTestId("collab-status-dot");
    expect(dot.style.backgroundColor).toBe("rgb(153, 153, 153)");
  });
});
