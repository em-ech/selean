import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useAutoSave } from "./useAutoSave";
import { createMockEditor } from "../test/mock-editor";

const STORAGE_KEY = "selean_autosave";

describe("useAutoSave", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    localStorage.clear();
  });

  it("saves to localStorage on interval when dirty", () => {
    const editor = createMockEditor({
      export_document_json: vi.fn().mockReturnValue('{"test":"data"}'),
    });
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    act(() => {
      result.current.markDirty();
    });

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(editor.export_document_json).toHaveBeenCalledTimes(1);
    expect(localStorage.getItem(STORAGE_KEY)).toBe('{"test":"data"}');
  });

  it("does not save when not dirty", () => {
    const editor = createMockEditor();
    const editorRef = { current: editor };

    renderHook(() => useAutoSave({ editorRef, isReady: true }));

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(editor.export_document_json).not.toHaveBeenCalled();
    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it("clears dirty flag after save", () => {
    const exportFn = vi.fn().mockReturnValue('{"saved":true}');
    const editor = createMockEditor({ export_document_json: exportFn });
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    act(() => {
      result.current.markDirty();
    });

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(exportFn).toHaveBeenCalledTimes(1);

    // Second interval should not save again (dirty cleared).
    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(exportFn).toHaveBeenCalledTimes(1);
  });

  it("does not start interval when not ready", () => {
    const editor = createMockEditor();
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: false }),
    );

    act(() => {
      result.current.markDirty();
    });

    act(() => {
      vi.advanceTimersByTime(10000);
    });

    expect(editor.export_document_json).not.toHaveBeenCalled();
  });

  it("loadSavedDocument returns stored JSON", () => {
    localStorage.setItem(STORAGE_KEY, '{"pages":[]}');

    const editor = createMockEditor();
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    expect(result.current.loadSavedDocument()).toBe('{"pages":[]}');
  });

  it("loadSavedDocument returns null when empty", () => {
    const editor = createMockEditor();
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    expect(result.current.loadSavedDocument()).toBeNull();
  });

  it("clearSavedDocument removes from localStorage", () => {
    localStorage.setItem(STORAGE_KEY, '{"pages":[]}');

    const editor = createMockEditor();
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    act(() => {
      result.current.clearSavedDocument();
    });

    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it("handles export_document_json throwing (quota exceeded)", () => {
    const editor = createMockEditor({
      export_document_json: vi.fn().mockImplementation(() => {
        throw new Error("Quota exceeded");
      }),
    });
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    act(() => {
      result.current.markDirty();
    });

    // Should not throw.
    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it("does not save when editor ref is null", () => {
    const editorRef = { current: null };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    act(() => {
      result.current.markDirty();
    });

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it("saves multiple times when marked dirty repeatedly", () => {
    const exportFn = vi
      .fn()
      .mockReturnValueOnce('{"v":1}')
      .mockReturnValueOnce('{"v":2}');
    const editor = createMockEditor({ export_document_json: exportFn });
    const editorRef = { current: editor };

    const { result } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    // First save cycle.
    act(() => {
      result.current.markDirty();
    });
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(localStorage.getItem(STORAGE_KEY)).toBe('{"v":1}');

    // Mark dirty again for second cycle.
    act(() => {
      result.current.markDirty();
    });
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(localStorage.getItem(STORAGE_KEY)).toBe('{"v":2}');
    expect(exportFn).toHaveBeenCalledTimes(2);
  });

  it("cleans up interval on unmount", () => {
    const editor = createMockEditor();
    const editorRef = { current: editor };

    const { result, unmount } = renderHook(() =>
      useAutoSave({ editorRef, isReady: true }),
    );

    act(() => {
      result.current.markDirty();
    });

    unmount();

    act(() => {
      vi.advanceTimersByTime(10000);
    });

    // Should not have saved after unmount.
    expect(editor.export_document_json).not.toHaveBeenCalled();
  });
});
