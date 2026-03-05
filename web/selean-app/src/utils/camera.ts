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
