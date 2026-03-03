import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useFontLoader, extractFontFamilies } from "./useFontLoader";

function createMockEditor() {
  return {
    register_font: vi.fn().mockReturnValue(true),
    register_image_asset: vi.fn(),
    resize: vi.fn(),
    render: vi.fn(),
  } as any;
}

describe("extractFontFamilies", () => {
  it("returns empty for no font_family fields", () => {
    expect(extractFontFamilies('{"name":"test"}')).toEqual([]);
  });

  it("extracts single font family", () => {
    const json = '{"font_family":"Roboto"}';
    expect(extractFontFamilies(json)).toEqual(["Roboto"]);
  });

  it("extracts multiple unique families", () => {
    const json = '{"font_family":"Roboto","children":[{"font_family":"Lato"}]}';
    const result = extractFontFamilies(json);
    expect(result).toContain("Roboto");
    expect(result).toContain("Lato");
    expect(result).toHaveLength(2);
  });

  it("deduplicates repeated families", () => {
    const json = '{"font_family":"Roboto","other":{"font_family":"Roboto"}}';
    expect(extractFontFamilies(json)).toEqual(["Roboto"]);
  });

  it("extracts Inter (default)", () => {
    const json = '{"font_family":"Inter"}';
    expect(extractFontFamilies(json)).toEqual(["Inter"]);
  });
});

describe("useFontLoader", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      arrayBuffer: () => Promise.resolve(new ArrayBuffer(10)),
    });
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("skips Inter (already loaded)", async () => {
    const editor = createMockEditor();
    const ref = { current: editor };
    const json = '{"font_family":"Inter"}';

    renderHook(() => useFontLoader(ref, json));

    // Wait for any async work.
    await vi.waitFor(() => {
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  it("fetches and registers a non-Inter font", async () => {
    const editor = createMockEditor();
    const ref = { current: editor };
    const json = '{"font_family":"Roboto"}';

    renderHook(() => useFontLoader(ref, json));

    await vi.waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith("/api/fonts/roboto");
      expect(editor.register_font).toHaveBeenCalledWith(
        "Roboto",
        expect.any(Uint8Array),
      );
    });
  });

  it("does not fetch the same family twice", async () => {
    const editor = createMockEditor();
    const ref = { current: editor };
    const json = '{"font_family":"Lato"}';

    const { rerender } = renderHook(
      ({ sceneJson }) => useFontLoader(ref, sceneJson),
      { initialProps: { sceneJson: json } },
    );

    await vi.waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    rerender({ sceneJson: json });

    // Still only one fetch call.
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("handles fetch error gracefully", async () => {
    const consoleSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    fetchMock.mockResolvedValue({ ok: false, status: 404 });

    const editor = createMockEditor();
    const ref = { current: editor };
    const json = '{"font_family":"Missing"}';

    renderHook(() => useFontLoader(ref, json));

    await vi.waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith("/api/fonts/missing");
    });

    // register_font should NOT have been called.
    expect(editor.register_font).not.toHaveBeenCalled();
    consoleSpy.mockRestore();
  });

  it("does nothing when editor is null", () => {
    const ref = { current: null };
    const json = '{"font_family":"Roboto"}';

    renderHook(() => useFontLoader(ref, json));

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("does nothing when sceneJson is null", () => {
    const editor = createMockEditor();
    const ref = { current: editor };

    renderHook(() => useFontLoader(ref, null));

    expect(fetchMock).not.toHaveBeenCalled();
  });
});
