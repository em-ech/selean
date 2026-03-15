import { describe, it, expect } from "vitest";
import {
  rgbaArrayToString,
  rgbaToString,
  rgbaToHex,
  hexToRgba,
  rgbaToHsl,
  hslToRgba,
} from "./color";

describe("rgbaArrayToString", () => {
  it("converts 0-1 array to CSS rgba string", () => {
    expect(rgbaArrayToString([1, 0, 0, 1])).toBe("rgba(255,0,0,1)");
  });

  it("handles fractional values", () => {
    expect(rgbaArrayToString([0.5, 0.5, 0.5, 0.5])).toBe(
      "rgba(128,128,128,0.5)",
    );
  });

  it("handles zeros", () => {
    expect(rgbaArrayToString([0, 0, 0, 0])).toBe("rgba(0,0,0,0)");
  });
});

describe("rgbaToString", () => {
  it("converts RgbaColor to CSS string", () => {
    expect(rgbaToString({ r: 1, g: 0, b: 0, a: 1 })).toBe("rgba(255,0,0,1)");
  });
});

describe("rgbaToHex", () => {
  it("converts red to #ff0000", () => {
    expect(rgbaToHex({ r: 1, g: 0, b: 0, a: 1 })).toBe("#ff0000");
  });

  it("converts black to #000000", () => {
    expect(rgbaToHex({ r: 0, g: 0, b: 0, a: 1 })).toBe("#000000");
  });

  it("converts white to #ffffff", () => {
    expect(rgbaToHex({ r: 1, g: 1, b: 1, a: 1 })).toBe("#ffffff");
  });

  it("ignores alpha in hex output", () => {
    expect(rgbaToHex({ r: 1, g: 0, b: 0, a: 0.5 })).toBe("#ff0000");
  });
});

describe("hexToRgba", () => {
  it("parses valid 6-char hex", () => {
    const c = hexToRgba("#ff0000");
    expect(c).not.toBeNull();
    expect(c!.r).toBe(1);
    expect(c!.g).toBe(0);
    expect(c!.b).toBe(0);
    expect(c!.a).toBe(1);
  });

  it("parses uppercase hex", () => {
    const c = hexToRgba("#AABBCC");
    expect(c).not.toBeNull();
    expect(c!.r).toBeCloseTo(0.667, 2);
  });

  it("handles double hash", () => {
    const c = hexToRgba("##ff0000");
    expect(c).not.toBeNull();
    expect(c!.r).toBe(1);
  });

  it("returns null for 3-char hex", () => {
    expect(hexToRgba("#fff")).toBeNull();
  });

  it("returns null for invalid characters", () => {
    expect(hexToRgba("#gggggg")).toBeNull();
  });

  it("returns null for empty string", () => {
    expect(hexToRgba("")).toBeNull();
  });

  it("returns null for non-hex characters", () => {
    expect(hexToRgba("#12345g")).toBeNull();
  });

  it("trims whitespace", () => {
    const c = hexToRgba("  #ff0000  ");
    expect(c).not.toBeNull();
    expect(c!.r).toBe(1);
  });

  it("preserves custom alpha", () => {
    const c = hexToRgba("#ff0000", 0.5);
    expect(c).not.toBeNull();
    expect(c!.a).toBe(0.5);
  });
});

describe("rgbaToHsl / hslToRgba roundtrip", () => {
  it("roundtrips red", () => {
    const red = { r: 1, g: 0, b: 0, a: 1 };
    const hsl = rgbaToHsl(red);
    const back = hslToRgba(hsl.h, hsl.s, hsl.l, 1);
    expect(back.r).toBeCloseTo(1, 2);
    expect(back.g).toBeCloseTo(0, 2);
    expect(back.b).toBeCloseTo(0, 2);
  });

  it("handles achromatic black", () => {
    const hsl = rgbaToHsl({ r: 0, g: 0, b: 0, a: 1 });
    expect(hsl.h).toBe(0);
    expect(hsl.s).toBe(0);
    expect(hsl.l).toBe(0);
  });

  it("handles achromatic white", () => {
    const hsl = rgbaToHsl({ r: 1, g: 1, b: 1, a: 1 });
    expect(hsl.h).toBe(0);
    expect(hsl.s).toBe(0);
    expect(hsl.l).toBe(1);
  });

  it("handles mid-gray", () => {
    const hsl = rgbaToHsl({ r: 0.5, g: 0.5, b: 0.5, a: 1 });
    expect(hsl.s).toBe(0);
    expect(hsl.l).toBeCloseTo(0.5, 2);
  });

  it("roundtrips green", () => {
    const green = { r: 0, g: 1, b: 0, a: 1 };
    const hsl = rgbaToHsl(green);
    const back = hslToRgba(hsl.h, hsl.s, hsl.l, 1);
    expect(back.r).toBeCloseTo(0, 2);
    expect(back.g).toBeCloseTo(1, 2);
    expect(back.b).toBeCloseTo(0, 2);
  });

  it("roundtrips blue", () => {
    const blue = { r: 0, g: 0, b: 1, a: 1 };
    const hsl = rgbaToHsl(blue);
    const back = hslToRgba(hsl.h, hsl.s, hsl.l, 1);
    expect(back.r).toBeCloseTo(0, 2);
    expect(back.g).toBeCloseTo(0, 2);
    expect(back.b).toBeCloseTo(1, 2);
  });
});
