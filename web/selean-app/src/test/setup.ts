import "@testing-library/jest-dom/vitest";

// Stub navigator.gpu so WebGPU availability checks don't fail.
Object.defineProperty(globalThis.navigator, "gpu", {
  value: {
    requestAdapter: async () => null,
  },
  writable: true,
  configurable: true,
});

// Stub ResizeObserver (not available in jsdom).
class MockResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver =
  MockResizeObserver as unknown as typeof ResizeObserver;

// Stub requestAnimationFrame / cancelAnimationFrame.
// Uses setTimeout(0) so fake timers can control RAF execution.
// This avoids infinite microtask chains from polling loops (e.g. SelectionOverlay).
let rafId = 0;
const rafTimers = new Map<number, ReturnType<typeof setTimeout>>();

globalThis.requestAnimationFrame = (cb: FrameRequestCallback): number => {
  const id = ++rafId;
  const timer = setTimeout(() => {
    rafTimers.delete(id);
    cb(performance.now());
  }, 0);
  rafTimers.set(id, timer);
  return id;
};

globalThis.cancelAnimationFrame = (id: number) => {
  const timer = rafTimers.get(id);
  if (timer !== undefined) {
    clearTimeout(timer);
    rafTimers.delete(id);
  }
};

// Stub HTMLCanvasElement.getContext to avoid jsdom errors when Canvas component mounts.
HTMLCanvasElement.prototype.getContext = (() => null) as never;

// Stub PointerEvent (jsdom doesn't support it).
if (typeof PointerEvent === "undefined") {
  // @ts-expect-error partial stub
  globalThis.PointerEvent = class PointerEvent extends MouseEvent {
    pointerId: number;
    constructor(
      type: string,
      init?: PointerEventInit & { pointerId?: number },
    ) {
      super(type, init);
      this.pointerId = init?.pointerId ?? 0;
    }
  };
}

// Stub Element.setPointerCapture / releasePointerCapture.
Element.prototype.setPointerCapture = function () {};
Element.prototype.releasePointerCapture = function () {};

// Stub Element.scrollIntoView (not implemented in jsdom).
Element.prototype.scrollIntoView = function () {};
