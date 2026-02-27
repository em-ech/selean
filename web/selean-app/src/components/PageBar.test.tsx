import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PageBar } from "./PageBar";
import { createMockEditorRef } from "../test/mock-editor";
import type { PageInfo } from "../wasm/types";

function makePages(pages: PageInfo[]): string {
  return JSON.stringify(pages);
}

const twoPages: PageInfo[] = [
  { id: "p1", name: "Page 1", width: 1920, height: 1080, node_count: 0 },
  { id: "p2", name: "Page 2", width: 1920, height: 1080, node_count: 3 },
];

describe("PageBar", () => {
  it("renders page tabs", () => {
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue(makePages(twoPages)),
    });
    render(
      <PageBar editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByText("Page 1")).toBeInTheDocument();
    expect(screen.getByText("Page 2")).toBeInTheDocument();
  });

  it("renders nothing when no pages", () => {
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue("[]"),
    });
    const { container } = render(
      <PageBar editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders add page button", () => {
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue(makePages(twoPages)),
    });
    render(
      <PageBar editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByTitle("Add page")).toBeInTheDocument();
  });

  it("calls set_active_page when a tab is clicked", () => {
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue(makePages(twoPages)),
    });
    const onChanged = vi.fn();
    render(
      <PageBar editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );

    fireEvent.click(screen.getByText("Page 2"));
    expect(ref.current.set_active_page).toHaveBeenCalledWith("p2");
    expect(onChanged).toHaveBeenCalled();
  });

  it("calls add_page and set_active_page when + is clicked", () => {
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue(makePages(twoPages)),
      add_page: vi.fn().mockReturnValue("p-new"),
    });
    const onChanged = vi.fn();
    render(
      <PageBar editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );

    fireEvent.click(screen.getByTitle("Add page"));
    expect(ref.current.add_page).toHaveBeenCalledWith("Page 3", 1920, 1080);
    expect(ref.current.set_active_page).toHaveBeenCalledWith("p-new");
    expect(onChanged).toHaveBeenCalled();
  });

  it("shows close buttons when more than one page", () => {
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue(makePages(twoPages)),
    });
    render(
      <PageBar editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    const closeButtons = screen.getAllByTitle("Remove page");
    expect(closeButtons).toHaveLength(2);
  });

  it("does not show close buttons when only one page", () => {
    const single: PageInfo[] = [
      { id: "p1", name: "Page 1", width: 1920, height: 1080, node_count: 0 },
    ];
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue(makePages(single)),
    });
    render(
      <PageBar editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.queryByTitle("Remove page")).not.toBeInTheDocument();
  });

  it("calls remove_page when close button is clicked", () => {
    const ref = createMockEditorRef({
      get_pages_json: vi.fn().mockReturnValue(makePages(twoPages)),
    });
    const onChanged = vi.fn();
    render(
      <PageBar editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );

    const closeButtons = screen.getAllByTitle("Remove page");
    fireEvent.click(closeButtons[1]); // Remove Page 2
    expect(ref.current.remove_page).toHaveBeenCalledWith("p2");
    expect(onChanged).toHaveBeenCalled();
  });

  it("re-fetches pages when refreshTick changes", () => {
    const getPagesFn = vi.fn().mockReturnValue(makePages(twoPages));
    const ref = createMockEditorRef({
      get_pages_json: getPagesFn,
    });
    const { rerender } = render(
      <PageBar editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    const initial = getPagesFn.mock.calls.length;

    rerender(
      <PageBar editorRef={ref} onSceneChanged={() => {}} refreshTick={1} />,
    );
    expect(getPagesFn.mock.calls.length).toBeGreaterThan(initial);
  });
});
