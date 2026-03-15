import { useCallback, useEffect, useRef, useState } from "react";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";
import {
  type RgbaColor,
  rgbaToHex,
  hexToRgba,
  rgbaToHsl,
  hslToRgba,
  rgbaToString,
} from "../utils/color";

export type { RgbaColor };

/** Gradient stop for the gradient editor. */
export interface GradientStop {
  position: number;
  color: RgbaColor;
}

export type FillMode = "solid" | "linear" | "radial" | "none";

export interface ColorPickerValue {
  mode: FillMode;
  color: RgbaColor;
  gradient?: {
    type: "linear" | "radial";
    stops: GradientStop[];
    /** Linear: start/end as fractions 0-1. Radial: center + radius. */
    start?: [number, number];
    end?: [number, number];
    center?: [number, number];
    radius?: number;
  };
}

interface ColorPickerProps {
  value: ColorPickerValue;
  onChange: (value: ColorPickerValue) => void;
  onClose: () => void;
}

const PRESET_COLORS = [
  "#000000",
  "#333333",
  "#666666",
  "#999999",
  "#cccccc",
  "#ffffff",
  "#e74c3c",
  "#e67e22",
  "#f1c40f",
  "#2ecc71",
  "#3498db",
  "#9b59b6",
  "#1abc9c",
  "#d35400",
  "#c0392b",
  "#27ae60",
  "#2980b9",
  "#8e44ad",
  "#f39c12",
  "#16a085",
  "#2c3e50",
  "#7f8c8d",
  "#bdc3c7",
  "#ecf0f1",
];

// Color conversion functions imported from ../utils/color.

/**
 * Canva-style color picker with hex input, hue/saturation spectrum,
 * opacity slider, preset swatches, and gradient mode.
 */
export function ColorPicker({ value, onChange, onClose }: ColorPickerProps) {
  const [hexInput, setHexInput] = useState(rgbaToHex(value.color));
  const [mode, setMode] = useState<FillMode>(value.mode);
  const [activeStopIndex, setActiveStopIndex] = useState(0);
  const spectrumRef = useRef<HTMLDivElement>(null);
  const hueRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const currentColor =
    mode === "none"
      ? { r: 0, g: 0, b: 0, a: 0 }
      : mode === "solid"
        ? value.color
        : (value.gradient?.stops[activeStopIndex]?.color ?? value.color);

  const hsl = rgbaToHsl(currentColor);

  // Close on outside click
  useEffect(() => {
    const handle = (e: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handle);
    return () => document.removeEventListener("mousedown", handle);
  }, [onClose]);

  // Sync hex input when color changes externally
  useEffect(() => {
    setHexInput(rgbaToHex(currentColor));
  }, [currentColor.r, currentColor.g, currentColor.b]);

  const updateColor = useCallback(
    (newColor: RgbaColor) => {
      if (mode === "solid" || mode === "none") {
        onChange({ ...value, mode: "solid", color: newColor });
      } else if (value.gradient) {
        const stops = [...value.gradient.stops];
        stops[activeStopIndex] = {
          ...stops[activeStopIndex],
          color: newColor,
        };
        onChange({
          ...value,
          mode,
          gradient: { ...value.gradient, stops },
        });
      }
    },
    [value, mode, activeStopIndex, onChange],
  );

  const handleHexCommit = useCallback(() => {
    const parsed = hexToRgba(hexInput, currentColor.a);
    if (parsed) {
      updateColor(parsed);
    } else {
      setHexInput(rgbaToHex(currentColor));
    }
  }, [hexInput, currentColor, updateColor]);

  const handleSpectrumPointer = useCallback(
    (e: React.PointerEvent | PointerEvent) => {
      const rect = spectrumRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
      // x = saturation, y = lightness (inverted)
      const s = x;
      const l = 1 - y;
      const newColor = hslToRgba(hsl.h, s, l * 0.5 + 0.25, currentColor.a);
      updateColor(newColor);
    },
    [hsl.h, currentColor.a, updateColor],
  );

  const handleHuePointer = useCallback(
    (e: React.PointerEvent | PointerEvent) => {
      const rect = hueRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      const newColor = hslToRgba(x, hsl.s, hsl.l, currentColor.a);
      updateColor(newColor);
    },
    [hsl.s, hsl.l, currentColor.a, updateColor],
  );

  const startDrag = useCallback(
    (handler: (e: PointerEvent) => void, e: React.PointerEvent) => {
      handler(e.nativeEvent);
      const move = (ev: PointerEvent) => handler(ev);
      const up = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    },
    [],
  );

  const handleModeChange = useCallback(
    (newMode: FillMode) => {
      setMode(newMode);
      if (newMode === "none") {
        onChange({ ...value, mode: "none", color: { r: 0, g: 0, b: 0, a: 0 } });
      } else if (newMode === "solid") {
        onChange({ ...value, mode: "solid" });
      } else {
        // Initialize gradient with current color
        const defaultStops: GradientStop[] = [
          { position: 0, color: value.color },
          { position: 1, color: { r: 1, g: 1, b: 1, a: 1 } },
        ];
        onChange({
          ...value,
          mode: newMode,
          gradient: {
            type: newMode === "linear" ? "linear" : "radial",
            stops: value.gradient?.stops ?? defaultStops,
            start: [0, 0.5],
            end: [1, 0.5],
            center: [0.5, 0.5],
            radius: 0.5,
          },
        });
      }
    },
    [value, onChange],
  );

  return (
    <div ref={popoverRef} style={popoverStyle}>
      {/* Fill mode tabs */}
      <div style={modeTabsStyle}>
        {(["solid", "linear", "radial", "none"] as const).map((m) => (
          <button
            key={m}
            style={{
              ...modeTabStyle,
              ...(mode === m ? modeTabActiveStyle : {}),
            }}
            onClick={() => handleModeChange(m)}
          >
            {m === "none"
              ? "None"
              : m === "solid"
                ? "Solid"
                : m === "linear"
                  ? "Linear"
                  : "Radial"}
          </button>
        ))}
      </div>

      {mode !== "none" && (
        <>
          {/* Gradient stop bar (only for gradient modes) */}
          {(mode === "linear" || mode === "radial") && value.gradient && (
            <div style={gradientBarContainerStyle}>
              <div
                style={{
                  ...gradientBarStyle,
                  background: `linear-gradient(to right, ${value.gradient.stops
                    .map(
                      (s) =>
                        `${rgbaToString(s.color)} ${s.position * 100}%`,
                    )
                    .join(", ")})`,
                }}
              >
                {value.gradient.stops.map((stop, i) => (
                  <button
                    key={i}
                    style={{
                      ...gradientStopHandleStyle,
                      left: `${stop.position * 100}%`,
                      borderColor:
                        i === activeStopIndex
                          ? colors.accent
                          : colors.borderHover,
                    }}
                    onClick={() => setActiveStopIndex(i)}
                  />
                ))}
              </div>
            </div>
          )}

          {/* Color spectrum (saturation x lightness) */}
          <div
            ref={spectrumRef}
            style={{
              ...spectrumStyle,
              background: `linear-gradient(to right, hsl(${hsl.h * 360}, 0%, 50%), hsl(${hsl.h * 360}, 100%, 50%))`,
            }}
            onPointerDown={(e) => startDrag(handleSpectrumPointer, e)}
          >
            <div style={spectrumOverlayWhiteStyle} />
            <div style={spectrumOverlayBlackStyle} />
            <div
              style={{
                ...spectrumCursorStyle,
                left: `${hsl.s * 100}%`,
                top: `${(1 - (hsl.l - 0.25) / 0.5) * 100}%`,
              }}
            />
          </div>

          {/* Hue slider */}
          <div
            ref={hueRef}
            style={hueBarStyle}
            onPointerDown={(e) => startDrag(handleHuePointer, e)}
          >
            <div
              style={{
                ...hueCursorStyle,
                left: `${hsl.h * 100}%`,
              }}
            />
          </div>

          {/* Opacity slider */}
          <div style={opacityRowStyle}>
            <span style={fieldLabelStyle}>Opacity</span>
            <input
              type="range"
              min={0}
              max={100}
              value={Math.round(currentColor.a * 100)}
              onChange={(e) => {
                updateColor({
                  ...currentColor,
                  a: parseInt(e.target.value, 10) / 100,
                });
              }}
              style={opacitySliderStyle}
            />
            <span style={opacityValueStyle}>
              {Math.round(currentColor.a * 100)}%
            </span>
          </div>

          {/* Hex input + current swatch */}
          <div style={hexRowStyle}>
            <div
              style={{
                ...currentSwatchStyle,
                background: rgbaToString(currentColor),
              }}
            />
            <span style={fieldLabelStyle}>HEX</span>
            <input
              type="text"
              value={hexInput}
              onChange={(e) => setHexInput(e.target.value)}
              onBlur={handleHexCommit}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleHexCommit();
              }}
              style={hexInputStyle}
              maxLength={7}
            />
          </div>

          {/* Preset swatches */}
          <div style={presetsStyle}>
            {PRESET_COLORS.map((hex) => (
              <button
                key={hex}
                style={{
                  ...presetSwatchStyle,
                  background: hex,
                }}
                onClick={() => {
                  const c = hexToRgba(hex, currentColor.a);
                  if (c) {
                    updateColor(c);
                    setHexInput(hex);
                  }
                }}
                title={hex}
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

// --- Styles ---

const popoverStyle: React.CSSProperties = {
  position: "absolute",
  top: "100%",
  left: 0,
  marginTop: spacing.sm,
  width: 240,
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.lg,
  boxShadow: shadows.lg,
  padding: spacing.md,
  zIndex: 100,
  display: "flex",
  flexDirection: "column",
  gap: spacing.sm,
};

const modeTabsStyle: React.CSSProperties = {
  display: "flex",
  gap: 0,
  background: colors.surface,
  borderRadius: radii.md,
  padding: 2,
};

const modeTabStyle: React.CSSProperties = {
  flex: 1,
  background: "transparent",
  border: "none",
  padding: `${spacing.xs}px 0`,
  fontSize: fontSizes.xs,
  color: colors.textDim,
  cursor: "pointer",
  borderRadius: radii.md - 2,
  fontWeight: 500,
};

const modeTabActiveStyle: React.CSSProperties = {
  background: colors.bg,
  color: colors.text,
  fontWeight: 600,
  boxShadow: shadows.sm,
};

const gradientBarContainerStyle: React.CSSProperties = {
  padding: `${spacing.xs}px 0`,
};

const gradientBarStyle: React.CSSProperties = {
  position: "relative",
  height: 20,
  borderRadius: radii.sm,
  border: `1px solid ${colors.border}`,
};

const gradientStopHandleStyle: React.CSSProperties = {
  position: "absolute",
  top: -3,
  width: 10,
  height: 26,
  background: colors.white,
  border: "2px solid",
  borderRadius: 3,
  cursor: "pointer",
  transform: "translateX(-50%)",
  padding: 0,
};

const spectrumStyle: React.CSSProperties = {
  position: "relative",
  height: 120,
  borderRadius: radii.sm,
  cursor: "crosshair",
  overflow: "hidden",
};

const spectrumOverlayWhiteStyle: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  background: "linear-gradient(to bottom, rgba(255,255,255,0.8), transparent)",
};

const spectrumOverlayBlackStyle: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  background: "linear-gradient(to top, rgba(0,0,0,0.8), transparent)",
};

const spectrumCursorStyle: React.CSSProperties = {
  position: "absolute",
  width: 12,
  height: 12,
  borderRadius: "50%",
  border: `2px solid ${colors.white}`,
  boxShadow: "0 0 2px rgba(0,0,0,0.5)",
  transform: "translate(-50%, -50%)",
  pointerEvents: "none",
};

const hueBarStyle: React.CSSProperties = {
  position: "relative",
  height: 12,
  borderRadius: radii.full,
  background:
    "linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00)",
  cursor: "pointer",
};

const hueCursorStyle: React.CSSProperties = {
  position: "absolute",
  top: -2,
  width: 8,
  height: 16,
  background: colors.white,
  border: `1px solid ${colors.borderHover}`,
  borderRadius: 3,
  transform: "translateX(-50%)",
  boxShadow: shadows.sm,
  pointerEvents: "none",
};

const opacityRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.sm,
};

const fieldLabelStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textDim,
  flexShrink: 0,
};

const opacitySliderStyle: React.CSSProperties = {
  flex: 1,
  height: 4,
  accentColor: colors.accent,
};

const opacityValueStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textMuted,
  width: 32,
  textAlign: "right",
};

const hexRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.sm,
};

const currentSwatchStyle: React.CSSProperties = {
  width: 24,
  height: 24,
  borderRadius: radii.sm,
  border: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const hexInputStyle: React.CSSProperties = {
  flex: 1,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.sm,
  color: colors.text,
  fontSize: fontSizes.sm,
  padding: `2px ${spacing.xs}px`,
  outline: "none",
  fontFamily: "monospace",
};

const presetsStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(6, 1fr)",
  gap: 4,
};

const presetSwatchStyle: React.CSSProperties = {
  width: "100%",
  aspectRatio: "1",
  borderRadius: radii.sm,
  border: `1px solid ${colors.border}`,
  cursor: "pointer",
  padding: 0,
};
