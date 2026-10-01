import { StrictMode } from "react";
import { renderHook, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useSeleanEditor } from "./useSeleanEditor";

const { initMock, createMock } = vi.hoisted(() => ({
  initMock: vi.fn().mockResolvedValue(undefined),
  createMock: vi.fn(),
}));

vi.mock("../wasm/pkg/selean_wasm", () => ({
  default: initMock,
  SeleanEditor: { create: createMock },
}));

function setGpu(value: unknown) {
  Object.defineProperty(navigator, "gpu", { value, configurable: true });
}

describe("useSeleanEditor", () => {
  beforeEach(() => {
    createMock.mockReset().mockResolvedValue({ render: vi.fn() });
    setGpu({});
  });

  afterEach(() => {
    setGpu(undefined);
  });

  it("reports unsupported when WebGPU is missing", async () => {
    setGpu(undefined);
    const { result } = renderHook(() => useSeleanEditor("canvas"));
    await waitFor(() => expect(result.current.status).toBe("unsupported"));
    expect(createMock).not.toHaveBeenCalled();
  });

  it("creates the editor on the given canvas and becomes ready", async () => {
    const { result } = renderHook(() => useSeleanEditor("canvas"));
    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(createMock).toHaveBeenCalledWith("canvas");
    expect(result.current.editorRef.current).not.toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("reports an error when the editor cannot be created", async () => {
    createMock.mockRejectedValue(new Error("no adapter"));
    const { result } = renderHook(() => useSeleanEditor("canvas"));
    await waitFor(() => expect(result.current.status).toBe("error"));
    expect(result.current.error).toBe("no adapter");
  });

  it("initialises the WASM module once across mounts, including StrictMode", async () => {
    const first = renderHook(() => useSeleanEditor("canvas"), {
      wrapper: StrictMode,
    });
    await waitFor(() => expect(first.result.current.status).toBe("ready"));
    const second = renderHook(() => useSeleanEditor("canvas"));
    await waitFor(() => expect(second.result.current.status).toBe("ready"));

    // One init for the whole file, however many hooks mounted.
    expect(initMock).toHaveBeenCalledTimes(1);
    // StrictMode mounts twice; only the surviving mount creates an editor.
    expect(createMock).toHaveBeenCalledTimes(2);
  });
});
