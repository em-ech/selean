import { describe, it, expect } from "vitest";
import { SHAPE_ELEMENTS, TEXT_ELEMENTS } from "./elements";
import { TEMPLATES } from "./templates";

/**
 * The presets are passed straight to the `create_node` tool. These are the
 * kinds and argument names it accepts (crates/selean-llm/src/tools.rs,
 * `tool_create_node`); anything else is rejected or silently ignored.
 */
const CREATE_NODE_KINDS = ["Frame", "Text", "Group", "Image", "Vector"];
const CREATE_NODE_ARGS = [
  "name",
  "kind",
  "x",
  "y",
  "width",
  "height",
  "fill_r",
  "fill_g",
  "fill_b",
  "fill_a",
  "text_content",
  "font_size",
  "corner_radius",
  "asset_ref",
  "path_data",
];

describe("element presets", () => {
  it("shape presets use a kind create_node accepts and a visible fill", () => {
    expect(SHAPE_ELEMENTS.length).toBeGreaterThan(0);
    for (const el of SHAPE_ELEMENTS) {
      expect(CREATE_NODE_KINDS, el.label).toContain(el.kind);
      expect(Object.keys(el.defaults), el.label).toEqual(
        expect.arrayContaining(["width", "height", "fill_r"]),
      );
      for (const key of Object.keys(el.defaults)) {
        expect(CREATE_NODE_ARGS, `${el.label}.${key}`).toContain(key);
      }
    }
  });

  it("text presets carry their text in text_content", () => {
    expect(TEXT_ELEMENTS.length).toBeGreaterThan(0);
    for (const el of TEXT_ELEMENTS) {
      expect(typeof el.defaults.text_content, el.label).toBe("string");
      for (const key of Object.keys(el.defaults)) {
        expect(CREATE_NODE_ARGS, `${el.label}.${key}`).toContain(key);
      }
    }
  });
});

describe("templates", () => {
  it("every template element is a valid create_node call", () => {
    expect(TEMPLATES.length).toBeGreaterThan(0);
    for (const tmpl of TEMPLATES) {
      expect(tmpl.elements.length, tmpl.id).toBeGreaterThan(0);
      for (const el of tmpl.elements) {
        const args = el as Record<string, unknown>;
        const where = `${tmpl.id}/${String(args.name)}`;
        expect(CREATE_NODE_KINDS, where).toContain(args.kind);
        for (const required of ["name", "x", "y", "width", "height"]) {
          expect(args[required], `${where}.${required}`).toBeDefined();
        }
        for (const key of Object.keys(args)) {
          expect(CREATE_NODE_ARGS, `${where}.${key}`).toContain(key);
        }
        if (args.kind === "Text") {
          expect(typeof args.text_content, where).toBe("string");
        }
      }
    }
  });
});
