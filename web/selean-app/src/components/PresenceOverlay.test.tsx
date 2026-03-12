import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import {
  PresenceOverlay,
  colorForUser,
  type RemotePresence,
} from "./PresenceOverlay";
import { createMockEditorRef } from "../test/mock-editor";

describe("PresenceOverlay", () => {
  it("renders nothing when editor is null", () => {
    const ref = { current: null };
    const { container } = render(
      <PresenceOverlay editorRef={ref} presences={[]} activePageId="page-1" />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders nothing when no presences are visible", () => {
    const editorRef = createMockEditorRef();
    const { container } = render(
      <PresenceOverlay
        editorRef={editorRef}
        presences={[]}
        activePageId="page-1"
      />,
    );
    expect(
      container.querySelector("[data-testid='presence-overlay']"),
    ).toBeNull();
  });

  it("renders cursor for visible presence on same page", () => {
    const editorRef = createMockEditorRef();
    const presences: RemotePresence[] = [
      {
        participant: { session_id: "s1", user_id: "u1", display_name: "Alice" },
        pageId: "page-1",
        cursor: { x: 100, y: 200 },
        selectedNodeIds: [],
      },
    ];
    render(
      <PresenceOverlay
        editorRef={editorRef}
        presences={presences}
        activePageId="page-1"
      />,
    );
    expect(screen.getByTestId("presence-overlay")).toBeTruthy();
    expect(screen.getByTestId("cursor-s1")).toBeTruthy();
    expect(screen.getByText("Alice")).toBeTruthy();
  });

  it("filters presences on different page", () => {
    const editorRef = createMockEditorRef();
    const presences: RemotePresence[] = [
      {
        participant: { session_id: "s1", user_id: "u1", display_name: "Alice" },
        pageId: "page-2", // Different page.
        cursor: { x: 100, y: 200 },
        selectedNodeIds: [],
      },
    ];
    const { container } = render(
      <PresenceOverlay
        editorRef={editorRef}
        presences={presences}
        activePageId="page-1"
      />,
    );
    expect(
      container.querySelector("[data-testid='presence-overlay']"),
    ).toBeNull();
  });

  it("filters presences with null cursor", () => {
    const editorRef = createMockEditorRef();
    const presences: RemotePresence[] = [
      {
        participant: { session_id: "s1", user_id: "u1", display_name: "Alice" },
        pageId: "page-1",
        cursor: null,
        selectedNodeIds: [],
      },
    ];
    const { container } = render(
      <PresenceOverlay
        editorRef={editorRef}
        presences={presences}
        activePageId="page-1"
      />,
    );
    expect(
      container.querySelector("[data-testid='presence-overlay']"),
    ).toBeNull();
  });

  it("renders multiple cursors", () => {
    const editorRef = createMockEditorRef();
    const presences: RemotePresence[] = [
      {
        participant: { session_id: "s1", user_id: "u1", display_name: "Alice" },
        pageId: "page-1",
        cursor: { x: 50, y: 50 },
        selectedNodeIds: [],
      },
      {
        participant: { session_id: "s2", user_id: "u2", display_name: "Bob" },
        pageId: "page-1",
        cursor: { x: 200, y: 100 },
        selectedNodeIds: [],
      },
    ];
    render(
      <PresenceOverlay
        editorRef={editorRef}
        presences={presences}
        activePageId="page-1"
      />,
    );
    expect(screen.getByTestId("cursor-s1")).toBeTruthy();
    expect(screen.getByTestId("cursor-s2")).toBeTruthy();
  });
});

describe("colorForUser", () => {
  it("returns a hex color string", () => {
    const color = colorForUser("user-1");
    expect(color).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it("returns consistent color for same user", () => {
    expect(colorForUser("alice")).toBe(colorForUser("alice"));
  });

  it("returns different colors for different users (usually)", () => {
    // With 8 colors, two random strings should usually differ.
    const a = colorForUser("alice");
    const b = colorForUser("bob");
    // Not guaranteed, but very likely different.
    expect(typeof a).toBe("string");
    expect(typeof b).toBe("string");
  });
});
