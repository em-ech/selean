import type { CameraInfo } from "../wasm/types";

/** Converts world coordinates to screen (CSS pixel) coordinates using camera state. */
export function worldToScreen(
  worldX: number,
  worldY: number,
  camera: CameraInfo,
): { x: number; y: number } {
  const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
  return {
    x:
      ((worldX - camera.pan_x) * camera.zoom + camera.viewport_width / 2) / dpr,
    y:
      ((worldY - camera.pan_y) * camera.zoom + camera.viewport_height / 2) /
      dpr,
  };
}

/** Converts world-space dimensions to screen (CSS pixel) dimensions. */
export function worldDimsToScreen(
  width: number,
  height: number,
  zoom: number,
): { w: number; h: number } {
  const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
  return {
    w: (width * zoom) / dpr,
    h: (height * zoom) / dpr,
  };
}

/**
 * Converts a client-space (CSS pixel) delta to a world-space delta.
 *
 * CSS pixels * devicePixelRatio = physical pixels.
 * Physical pixels / zoom = world units.
 */
export function clientToWorldDelta(
  dx: number,
  dy: number,
  camera: CameraInfo,
): [number, number] {
  const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
  return [(dx * dpr) / camera.zoom, (dy * dpr) / camera.zoom];
}

/**
 * Converts a screen-space physical pixel position to world coordinates.
 *
 * This is the inverse of `worldToScreen`: given a position in physical pixels
 * (e.g. clientX * dpr), returns the corresponding world-space point.
 */
export function screenToWorld(
  screenX: number,
  screenY: number,
  camera: CameraInfo,
): { x: number; y: number } {
  return {
    x: (screenX - camera.viewport_width / 2) / camera.zoom + camera.pan_x,
    y: (screenY - camera.viewport_height / 2) / camera.zoom + camera.pan_y,
  };
}
