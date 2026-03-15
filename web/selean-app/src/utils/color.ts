/**
 * Shared color conversion utilities.
 * Used by ColorPicker, FloatingToolbar, and any component that works with
 * RGBA color arrays from the WASM engine.
 */

/** RGBA color as 0-1 floats. */
export interface RgbaColor {
  r: number;
  g: number;
  b: number;
  a: number;
}

/** Convert an RGBA 0-1 array to a CSS rgba() string. */
export function rgbaArrayToString(
  rgba: [number, number, number, number],
): string {
  return `rgba(${Math.round(rgba[0] * 255)},${Math.round(rgba[1] * 255)},${Math.round(rgba[2] * 255)},${rgba[3]})`;
}

/** Convert an RgbaColor object to a CSS rgba() string. */
export function rgbaToString(c: RgbaColor): string {
  return `rgba(${Math.round(c.r * 255)},${Math.round(c.g * 255)},${Math.round(c.b * 255)},${c.a})`;
}

/** Convert an RgbaColor to a hex string (#rrggbb). */
export function rgbaToHex(c: RgbaColor): string {
  const r = Math.round(c.r * 255)
    .toString(16)
    .padStart(2, "0");
  const g = Math.round(c.g * 255)
    .toString(16)
    .padStart(2, "0");
  const b = Math.round(c.b * 255)
    .toString(16)
    .padStart(2, "0");
  return `#${r}${g}${b}`;
}

/** Convert a hex string to an RgbaColor. Returns null if invalid. */
export function hexToRgba(hex: string, alpha = 1): RgbaColor | null {
  const clean = hex.trim().replace(/^#+/, "");
  if (!/^[0-9a-fA-F]{6}$/.test(clean)) return null;
  const r = parseInt(clean.slice(0, 2), 16) / 255;
  const g = parseInt(clean.slice(2, 4), 16) / 255;
  const b = parseInt(clean.slice(4, 6), 16) / 255;
  return { r, g, b, a: alpha };
}

/** Convert an RgbaColor to HSL (h, s, l in 0-1 range). */
export function rgbaToHsl(c: RgbaColor): { h: number; s: number; l: number } {
  const max = Math.max(c.r, c.g, c.b);
  const min = Math.min(c.r, c.g, c.b);
  const l = (max + min) / 2;
  // Achromatic (grayscale): hue and saturation are undefined, return neutral.
  if (max === min) return { h: 0, s: 0, l };
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === c.r) h = ((c.g - c.b) / d + (c.g < c.b ? 6 : 0)) / 6;
  else if (max === c.g) h = ((c.b - c.r) / d + 2) / 6;
  else h = ((c.r - c.g) / d + 4) / 6;
  return { h, s, l };
}

/** Convert HSL (0-1 range) + alpha to RgbaColor. */
export function hslToRgba(
  h: number,
  s: number,
  l: number,
  a: number,
): RgbaColor {
  if (s === 0) return { r: l, g: l, b: l, a };
  const hue2rgb = (p: number, q: number, t: number) => {
    let tt = t;
    if (tt < 0) tt += 1;
    if (tt > 1) tt -= 1;
    if (tt < 1 / 6) return p + (q - p) * 6 * tt;
    if (tt < 1 / 2) return q;
    if (tt < 2 / 3) return p + (q - p) * (2 / 3 - tt) * 6;
    return p;
  };
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  return {
    r: hue2rgb(p, q, h + 1 / 3),
    g: hue2rgb(p, q, h),
    b: hue2rgb(p, q, h - 1 / 3),
    a,
  };
}
