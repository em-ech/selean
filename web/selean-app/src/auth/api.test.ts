import { describe, it, expect, vi, afterEach } from "vitest";
import { fetchAuthEnabled } from "./api";

function mockFetch(response: Partial<Response> | Error) {
  const fetchMock =
    response instanceof Error
      ? vi.fn().mockRejectedValue(response)
      : vi.fn().mockResolvedValue(response);
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("fetchAuthEnabled", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns false when the server reports auth disabled", async () => {
    const fetchMock = mockFetch({
      ok: true,
      json: () => Promise.resolve({ auth_enabled: false }),
    });
    await expect(fetchAuthEnabled()).resolves.toBe(false);
    expect(fetchMock).toHaveBeenCalledWith("/api/auth/status");
  });

  it("returns true when the server reports auth enabled", async () => {
    mockFetch({
      ok: true,
      json: () => Promise.resolve({ auth_enabled: true }),
    });
    await expect(fetchAuthEnabled()).resolves.toBe(true);
  });

  it("fails closed on a non-OK response", async () => {
    mockFetch({ ok: false, status: 502 });
    await expect(fetchAuthEnabled()).resolves.toBe(true);
  });

  it("fails closed when the server is unreachable", async () => {
    mockFetch(new TypeError("Failed to fetch"));
    await expect(fetchAuthEnabled()).resolves.toBe(true);
  });

  it("fails closed on an unexpected body", async () => {
    mockFetch({ ok: true, json: () => Promise.resolve({}) });
    await expect(fetchAuthEnabled()).resolves.toBe(true);
  });

  it("fails closed when the body is not JSON", async () => {
    mockFetch({
      ok: true,
      json: () => Promise.reject(new SyntaxError("Unexpected token <")),
    });
    await expect(fetchAuthEnabled()).resolves.toBe(true);
  });
});
