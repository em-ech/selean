import { describe, it, expect, vi, beforeEach } from "vitest";
import { uploadAsset, type AssetResponse } from "./useFileOperations";

vi.mock("../utils/api", () => ({
  authFetch: vi.fn(),
}));

import { authFetch } from "../utils/api";

const mockAuthFetch = authFetch as ReturnType<typeof vi.fn>;

const SAMPLE_ASSET: AssetResponse = {
  id: "asset-001",
  workspace_id: "ws-123",
  filename: "photo.png",
  content_type: "image/png",
  size_bytes: 1024,
  created_at: "2026-03-10T00:00:00Z",
  url: "/api/assets/asset-001",
};

describe("uploadAsset", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("sends FormData with file and workspace_id", async () => {
    mockAuthFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(SAMPLE_ASSET),
    });

    const file = new File(["pixels"], "photo.png", { type: "image/png" });
    await uploadAsset(file, "ws-123");

    expect(mockAuthFetch).toHaveBeenCalledTimes(1);
    const [url, opts] = mockAuthFetch.mock.calls[0];
    expect(url).toBe("/api/assets");
    expect(opts.method).toBe("POST");
    expect(opts.body).toBeInstanceOf(FormData);
    const formData = opts.body as FormData;
    expect(formData.get("file")).toBeInstanceOf(File);
    expect(formData.get("workspace_id")).toBe("ws-123");
  });

  it("returns AssetResponse on success", async () => {
    mockAuthFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(SAMPLE_ASSET),
    });

    const file = new File(["pixels"], "photo.png", { type: "image/png" });
    const result = await uploadAsset(file, "ws-123");

    expect(result).toEqual(SAMPLE_ASSET);
  });

  it("returns null on 403 (quota exceeded)", async () => {
    mockAuthFetch.mockResolvedValue({
      ok: false,
      status: 403,
      statusText: "Forbidden",
      json: () => Promise.resolve({ error: "Storage quota exceeded" }),
    });

    const file = new File(["pixels"], "photo.png", { type: "image/png" });
    const result = await uploadAsset(file, "ws-123");

    expect(result).toBeNull();
  });

  it("returns null on network error", async () => {
    mockAuthFetch.mockRejectedValue(new Error("Network failure"));

    const file = new File(["pixels"], "photo.png", { type: "image/png" });
    const result = await uploadAsset(file, "ws-123");

    expect(result).toBeNull();
  });
});
