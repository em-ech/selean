import React from "react";

export interface ShapeElement {
  label: string;
  kind: string;
  icon: React.ReactElement;
  defaults: Record<string, unknown>;
}

export interface TextElement {
  label: string;
  icon: React.ReactElement;
  defaults: Record<string, unknown>;
}

export const SHAPE_ELEMENTS: ShapeElement[] = [
  {
    label: "Rectangle",
    kind: "Rect",
    icon: React.createElement(
      "svg",
      {
        width: 20,
        height: 20,
        viewBox: "0 0 24 24",
        fill: "none",
        stroke: "currentColor",
        strokeWidth: 2,
      },
      React.createElement("rect", { x: 3, y: 3, width: 18, height: 18, rx: 2 }),
    ),
    defaults: { width: 200, height: 150 },
  },
  {
    label: "Square",
    kind: "Rect",
    icon: React.createElement(
      "svg",
      {
        width: 20,
        height: 20,
        viewBox: "0 0 24 24",
        fill: "none",
        stroke: "currentColor",
        strokeWidth: 2,
      },
      React.createElement("rect", { x: 4, y: 4, width: 16, height: 16 }),
    ),
    defaults: { width: 150, height: 150 },
  },
  {
    label: "Frame",
    kind: "Rect",
    icon: React.createElement(
      "svg",
      {
        width: 20,
        height: 20,
        viewBox: "0 0 24 24",
        fill: "none",
        stroke: "currentColor",
        strokeWidth: 2,
      },
      React.createElement("rect", { x: 2, y: 2, width: 20, height: 20, rx: 0 }),
      React.createElement("line", { x1: 2, y1: 8, x2: 22, y2: 8 }),
    ),
    defaults: { width: 400, height: 300 },
  },
];

export const TEXT_ELEMENTS: TextElement[] = [
  {
    label: "Heading",
    icon: React.createElement(
      "svg",
      {
        width: 20,
        height: 20,
        viewBox: "0 0 24 24",
        fill: "none",
        stroke: "currentColor",
        strokeWidth: 2,
      },
      React.createElement("path", { d: "M4 4h16M4 12h16M4 20h10" }),
    ),
    defaults: { text_content: "Heading", font_size: 32, width: 300, height: 50 },
  },
  {
    label: "Subheading",
    icon: React.createElement(
      "svg",
      {
        width: 20,
        height: 20,
        viewBox: "0 0 24 24",
        fill: "none",
        stroke: "currentColor",
        strokeWidth: 2,
      },
      React.createElement("path", { d: "M4 6h16M4 14h12" }),
    ),
    defaults: { text_content: "Subheading", font_size: 20, width: 250, height: 35 },
  },
  {
    label: "Body text",
    icon: React.createElement(
      "svg",
      {
        width: 20,
        height: 20,
        viewBox: "0 0 24 24",
        fill: "none",
        stroke: "currentColor",
        strokeWidth: 2,
      },
      React.createElement("path", { d: "M4 6h16M4 10h16M4 14h16M4 18h10" }),
    ),
    defaults: {
      text_content: "Type your text here",
      font_size: 14,
      width: 250,
      height: 24,
    },
  },
];
